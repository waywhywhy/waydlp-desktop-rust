//! Friendly display names for source websites.
//!
//! This is purely cosmetic. Which sites are *supported* is decided by the
//! installed yt-dlp version, never by this table.

use url::Url;

const KNOWN: &[(&str, &str)] = &[
    ("youtube.com", "YouTube"),
    ("youtu.be", "YouTube"),
    ("music.youtube.com", "YouTube Music"),
    ("tiktok.com", "TikTok"),
    ("instagram.com", "Instagram"),
    ("facebook.com", "Facebook"),
    ("fb.watch", "Facebook"),
    ("x.com", "X"),
    ("twitter.com", "X"),
    ("reddit.com", "Reddit"),
    ("redd.it", "Reddit"),
    ("twitch.tv", "Twitch"),
    ("soundcloud.com", "SoundCloud"),
    ("vimeo.com", "Vimeo"),
    ("dailymotion.com", "Dailymotion"),
    ("pinterest.com", "Pinterest"),
    ("bandcamp.com", "Bandcamp"),
    ("bilibili.com", "Bilibili"),
    ("archive.org", "Internet Archive"),
    ("rumble.com", "Rumble"),
    ("streamable.com", "Streamable"),
    ("mixcloud.com", "Mixcloud"),
];

/// Return a human-friendly site name for a URL, e.g. `YouTube` or `example.org`.
pub fn site_name(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    // Exact match first (music.youtube.com before youtube.com).
    for (domain, name) in KNOWN {
        if host == *domain {
            return (*name).to_string();
        }
    }
    for (domain, name) in KNOWN {
        if host.ends_with(&format!(".{domain}")) {
            return (*name).to_string();
        }
    }
    strip_common_prefixes(&host).to_string()
}

/// Same as [`site_name`] but accepts a raw string; falls back to the input.
pub fn site_name_str(url: &str) -> String {
    Url::parse(url).map(|u| site_name(&u)).unwrap_or_else(|_| url.to_string())
}

fn strip_common_prefixes(host: &str) -> &str {
    for p in ["www.", "m.", "mobile."] {
        if let Some(rest) = host.strip_prefix(p) {
            return rest;
        }
    }
    host
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sites() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert_eq!(site_name(&u("https://www.youtube.com/watch?v=1")), "YouTube");
        assert_eq!(site_name(&u("https://music.youtube.com/watch?v=1")), "YouTube Music");
        assert_eq!(site_name(&u("https://m.facebook.com/x")), "Facebook");
        assert_eq!(site_name(&u("https://artist.bandcamp.com/album/x")), "Bandcamp");
        assert_eq!(site_name(&u("https://www.example.org/v")), "example.org");
    }
}
