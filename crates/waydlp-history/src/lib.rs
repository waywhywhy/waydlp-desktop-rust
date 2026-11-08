//! Download history stored in SQLite.
//!
//! The database only ever stores what actually happened. Removing an entry
//! never touches the downloaded file; deleting files is a separate, explicit
//! action performed by the UI after confirmation.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("History database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("History database I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("The history database was created by a newer waydlp (version {0}).")]
    TooNew(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatus {
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

impl EntryStatus {
    fn as_str(self) -> &'static str {
        match self {
            EntryStatus::InProgress => "in_progress",
            EntryStatus::Completed => "completed",
            EntryStatus::Failed => "failed",
            EntryStatus::Cancelled => "cancelled",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "completed" => EntryStatus::Completed,
            "failed" => EntryStatus::Failed,
            "cancelled" => EntryStatus::Cancelled,
            _ => EntryStatus::InProgress,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EntryStatus::InProgress => "Interrupted",
            EntryStatus::Completed => "Completed",
            EntryStatus::Failed => "Failed",
            EntryStatus::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub site: String,
    pub created_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Human summary such as `MP4 · 1080p`.
    pub format: String,
    /// Serialized `DownloadOptions` (JSON) so a download can be retried exactly.
    pub options_json: String,
    pub is_playlist: bool,
    pub files: Vec<PathBuf>,
    pub status: EntryStatus,
    pub total_size: Option<u64>,
    pub thumbnail: Option<String>,
    pub error: Option<String>,
}

impl HistoryEntry {
    pub fn primary_file(&self) -> Option<&Path> {
        self.files.first().map(PathBuf::as_path)
    }
}

/// Data for a new row.
#[derive(Debug, Clone, Default)]
pub struct NewEntry {
    pub title: String,
    pub url: String,
    pub site: String,
    pub format: String,
    pub options_json: String,
    pub is_playlist: bool,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryFilter {
    pub search: String,
    pub status: Option<EntryStatus>,
    pub site: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryStats {
    pub total: u64,
    pub completed: u64,
    pub failed: u64,
    pub total_bytes: u64,
}

pub struct History {
    conn: Connection,
}

impl History {
    pub fn open(path: &Path) -> Result<Self, HistoryError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, HistoryError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, HistoryError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(HistoryError::TooNew(version));
        }
        if version < 1 {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS downloads (
                    id           INTEGER PRIMARY KEY AUTOINCREMENT,
                    title        TEXT NOT NULL,
                    url          TEXT NOT NULL,
                    site         TEXT NOT NULL,
                    created_at   TEXT NOT NULL,
                    finished_at  TEXT,
                    format       TEXT NOT NULL,
                    options_json TEXT NOT NULL,
                    is_playlist  INTEGER NOT NULL DEFAULT 0,
                    files_json   TEXT NOT NULL DEFAULT '[]',
                    status       TEXT NOT NULL,
                    total_size   INTEGER,
                    thumbnail    TEXT,
                    error        TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_downloads_created ON downloads(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status);
                "#,
            )?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        // Rows left "in progress" by a crash or forced quit are interrupted.
        Ok(Self { conn })
    }

    pub fn insert(&self, e: &NewEntry) -> Result<i64, HistoryError> {
        self.conn.execute(
            "INSERT INTO downloads (title, url, site, created_at, format, options_json, is_playlist, status, thumbnail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                e.title,
                e.url,
                e.site,
                Utc::now().to_rfc3339(),
                e.format,
                e.options_json,
                e.is_playlist as i64,
                EntryStatus::InProgress.as_str(),
                e.thumbnail,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_title(&self, id: i64, title: &str) -> Result<(), HistoryError> {
        self.conn.execute("UPDATE downloads SET title = ?1 WHERE id = ?2", params![title, id])?;
        Ok(())
    }

    /// Record the final state of a download.
    pub fn finish(
        &self,
        id: i64,
        status: EntryStatus,
        files: &[PathBuf],
        error: Option<&str>,
    ) -> Result<(), HistoryError> {
        let files_json = serde_json_files(files);
        let size: Option<u64> = if files.is_empty() {
            None
        } else {
            Some(files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum())
        };
        self.conn.execute(
            "UPDATE downloads SET status = ?1, files_json = ?2, error = ?3, total_size = ?4, finished_at = ?5 WHERE id = ?6",
            params![
                status.as_str(),
                files_json,
                error,
                size.map(|s| s as i64),
                Utc::now().to_rfc3339(),
                id
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: i64) -> Result<Option<HistoryEntry>, HistoryError> {
        Ok(self
            .conn
            .query_row(&format!("SELECT {COLS} FROM downloads WHERE id = ?1"), params![id], row_to_entry)
            .optional()?)
    }

    pub fn list(&self, f: &HistoryFilter) -> Result<Vec<HistoryEntry>, HistoryError> {
        let mut sql = format!("SELECT {COLS} FROM downloads WHERE 1=1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let search = f.search.trim();
        if !search.is_empty() {
            let pattern = format!("%{}%", escape_like(search));
            sql.push_str(" AND (title LIKE ? ESCAPE '\\' OR url LIKE ? ESCAPE '\\' OR site LIKE ? ESCAPE '\\')");
            args.push(Box::new(pattern.clone()));
            args.push(Box::new(pattern.clone()));
            args.push(Box::new(pattern));
        }
        if let Some(s) = f.status {
            sql.push_str(" AND status = ?");
            args.push(Box::new(s.as_str()));
        }
        if let Some(site) = &f.site {
            sql.push_str(" AND site = ?");
            args.push(Box::new(site.clone()));
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC");
        if let Some(l) = f.limit {
            sql.push_str(&format!(" LIMIT {}", l.min(100_000)));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())), row_to_entry)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn sites(&self) -> Result<Vec<String>, HistoryError> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT site FROM downloads ORDER BY site")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Remove the history row only; files on disk are untouched.
    pub fn remove(&self, id: i64) -> Result<(), HistoryError> {
        self.conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear(&self) -> Result<usize, HistoryError> {
        Ok(self.conn.execute("DELETE FROM downloads WHERE status != 'in_progress'", [])?)
    }

    /// Delete entries older than `days` days (0 = keep forever).
    pub fn prune_older_than(&self, days: u32) -> Result<usize, HistoryError> {
        if days == 0 {
            return Ok(0);
        }
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        Ok(self.conn.execute(
            "DELETE FROM downloads WHERE created_at < ?1 AND status != 'in_progress'",
            params![cutoff.to_rfc3339()],
        )?)
    }

    /// Mark rows left "in progress" by a previous session as interrupted
    /// (cancelled) so the UI never shows phantom active downloads.
    pub fn mark_interrupted(&self) -> Result<usize, HistoryError> {
        Ok(self.conn.execute(
            "UPDATE downloads SET status = 'cancelled', error = 'Interrupted: waydlp was closed before the download finished.' WHERE status = 'in_progress'",
            [],
        )?)
    }

    pub fn stats(&self) -> Result<HistoryStats, HistoryError> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(status = 'completed'), 0),
                    COALESCE(SUM(status = 'failed'), 0),
                    COALESCE(SUM(CASE WHEN status = 'completed' THEN total_size ELSE 0 END), 0)
             FROM downloads",
            [],
            |r| {
                Ok(HistoryStats {
                    total: r.get::<_, i64>(0)? as u64,
                    completed: r.get::<_, i64>(1)? as u64,
                    failed: r.get::<_, i64>(2)? as u64,
                    total_bytes: r.get::<_, i64>(3)?.max(0) as u64,
                })
            },
        )?)
    }
}

const COLS: &str = "id, title, url, site, created_at, finished_at, format, options_json, is_playlist, files_json, status, total_size, thumbnail, error";

fn row_to_entry(r: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let parse_dt =
        |s: Option<String>| s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.with_timezone(&Utc));
    let files_json: String = r.get(9)?;
    Ok(HistoryEntry {
        id: r.get(0)?,
        title: r.get(1)?,
        url: r.get(2)?,
        site: r.get(3)?,
        created_at: parse_dt(Some(r.get(4)?)).unwrap_or_else(Utc::now),
        finished_at: parse_dt(r.get(5)?),
        format: r.get(6)?,
        options_json: r.get(7)?,
        is_playlist: r.get::<_, i64>(8)? != 0,
        files: parse_files(&files_json),
        status: EntryStatus::parse(&r.get::<_, String>(10)?),
        total_size: r.get::<_, Option<i64>>(11)?.map(|v| v.max(0) as u64),
        thumbnail: r.get(12)?,
        error: r.get(13)?,
    })
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

// A tiny JSON array-of-strings codec to avoid pulling serde_json in here.
fn serde_json_files(files: &[PathBuf]) -> String {
    let mut out = String::from("[");
    for (i, f) in files.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        for c in f.to_string_lossy().chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}

fn parse_files(s: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut chars = s.trim().chars().peekable();
    if chars.next() != Some('[') {
        return out;
    }
    loop {
        match chars.next() {
            Some('"') => {
                let mut cur = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => cur.push('\n'),
                            Some('u') => {
                                let hex: String = chars.by_ref().take(4).collect();
                                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                    cur.push(ch);
                                }
                            }
                            Some(other) => cur.push(other),
                            None => break,
                        },
                        c => cur.push(c),
                    }
                }
                out.push(PathBuf::from(cur));
            }
            Some(']') | None => break,
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, site: &str) -> NewEntry {
        NewEntry {
            title: title.into(),
            url: format!("https://{site}/{title}"),
            site: site.into(),
            format: "MP4 · 1080p".into(),
            options_json: "{}".into(),
            is_playlist: false,
            thumbnail: None,
        }
    }

    #[test]
    fn insert_finish_get() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a 'quoted' é.mp4");
        std::fs::write(&f, vec![0u8; 1234]).unwrap();
        let h = History::open(&dir.path().join("h.sqlite")).unwrap();
        let id = h.insert(&entry("Cats", "YouTube")).unwrap();
        assert_eq!(h.get(id).unwrap().unwrap().status, EntryStatus::InProgress);
        h.finish(id, EntryStatus::Completed, std::slice::from_ref(&f), None).unwrap();
        let e = h.get(id).unwrap().unwrap();
        assert_eq!(e.status, EntryStatus::Completed);
        assert_eq!(e.files, vec![f]);
        assert_eq!(e.total_size, Some(1234));
        assert!(e.finished_at.is_some());
    }

    #[test]
    fn search_and_filter() {
        let h = History::open_in_memory().unwrap();
        let a = h.insert(&entry("Funny cats", "YouTube")).unwrap();
        let b = h.insert(&entry("Lofi beats", "SoundCloud")).unwrap();
        h.insert(&entry("100% real_cats", "Vimeo")).unwrap();
        h.finish(a, EntryStatus::Completed, &[], None).unwrap();
        h.finish(b, EntryStatus::Failed, &[], Some("boom")).unwrap();

        let r = h.list(&HistoryFilter { search: "cats".into(), ..Default::default() }).unwrap();
        assert_eq!(r.len(), 2);
        // LIKE wildcards in the search are literal.
        let r = h.list(&HistoryFilter { search: "100%".into(), ..Default::default() }).unwrap();
        assert_eq!(r.len(), 1);
        let r = h.list(&HistoryFilter { search: "l_cats".into(), ..Default::default() }).unwrap();
        assert_eq!(r.len(), 1);
        let r = h.list(&HistoryFilter { status: Some(EntryStatus::Failed), ..Default::default() }).unwrap();
        assert_eq!(r[0].error.as_deref(), Some("boom"));
        let r = h.list(&HistoryFilter { site: Some("Vimeo".into()), ..Default::default() }).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(h.sites().unwrap(), vec!["SoundCloud", "Vimeo", "YouTube"]);
        let s = h.stats().unwrap();
        assert_eq!((s.total, s.completed, s.failed), (3, 1, 1));
    }

    #[test]
    fn remove_never_touches_files() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("keep.mp3");
        std::fs::write(&f, b"x").unwrap();
        let h = History::open_in_memory().unwrap();
        let id = h.insert(&entry("Song", "YouTube")).unwrap();
        h.finish(id, EntryStatus::Completed, std::slice::from_ref(&f), None).unwrap();
        h.remove(id).unwrap();
        assert!(h.get(id).unwrap().is_none());
        assert!(f.exists());
    }

    #[test]
    fn interrupted_and_clear() {
        let h = History::open_in_memory().unwrap();
        let a = h.insert(&entry("A", "x")).unwrap();
        let b = h.insert(&entry("B", "x")).unwrap();
        h.finish(b, EntryStatus::Completed, &[], None).unwrap();
        assert_eq!(h.mark_interrupted().unwrap(), 1);
        assert_eq!(h.get(a).unwrap().unwrap().status, EntryStatus::Cancelled);
        assert_eq!(h.clear().unwrap(), 2);
        assert!(h.list(&HistoryFilter::default()).unwrap().is_empty());
    }

    #[test]
    fn files_codec_roundtrip() {
        let files = vec![PathBuf::from("C:\\a\\b \"c\".mp4"), PathBuf::from("/x/y\nz")];
        assert_eq!(parse_files(&serde_json_files(&files)), files);
        assert!(parse_files("garbage").is_empty());
        assert!(parse_files("[]").is_empty());
    }

    #[test]
    fn reopen_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("h.sqlite");
        {
            let h = History::open(&p).unwrap();
            h.insert(&entry("A", "x")).unwrap();
        }
        let h = History::open(&p).unwrap();
        assert_eq!(h.list(&HistoryFilter::default()).unwrap().len(), 1);
    }
}
