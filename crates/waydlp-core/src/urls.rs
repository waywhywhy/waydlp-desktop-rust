//! URL validation, safe normalisation and batch extraction.

use thiserror::Error;
use url::Url;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UrlError {
    #[error("Please enter a URL.")]
    Empty,
    #[error("This doesn't look like a web address.")]
    Invalid,
    #[error("Only http:// and https:// links are supported (got \"{0}\").")]
    UnsupportedScheme(String),
    #[error("The link has no host name.")]
    MissingHost,
    #[error("Local network and file addresses are not allowed.")]
    LocalAddress,
}

/// Validate and safely normalise a user-supplied URL.
///
/// Normalisation is intentionally conservative: we trim whitespace and
/// surrounding quotes/angle brackets, add `https://` when the scheme is
/// missing, and let the `url` crate canonicalise the host. We never strip
/// query parameters, because many sites encode the media id there.
pub fn validate_url(input: &str) -> Result<Url, UrlError> {
    let trimmed = trim_link_punctuation(input.trim()).trim();
    if trimmed.is_empty() {
        return Err(UrlError::Empty);
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(UrlError::Invalid);
    }

    let candidate = if has_scheme(trimmed) {
        trimmed.to_string()
    } else if looks_like_host(trimmed) {
        format!("https://{trimmed}")
    } else {
        return Err(UrlError::Invalid);
    };

    let url = Url::parse(&candidate).map_err(|_| UrlError::Invalid)?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(UrlError::UnsupportedScheme(other.to_string())),
    }
    let host = url.host_str().ok_or(UrlError::MissingHost)?;
    if host.is_empty() {
        return Err(UrlError::MissingHost);
    }
    if is_local_host(host) {
        return Err(UrlError::LocalAddress);
    }
    Ok(url)
}

/// Remove punctuation that surrounding text leaves attached to a link, e.g.
/// `(see https://x.com/v/1).` → `https://x.com/v/1`. Closing brackets are
/// only removed when unbalanced, so `https://en.wikipedia.org/wiki/A_(b)` survives.
fn trim_link_punctuation(s: &str) -> &str {
    let mut t = s.trim_start_matches(['"', '\'', '<', '(', '[']);
    while let Some(c) = t.chars().last() {
        let unbalanced = |open: char| t.matches(c).count() > t.matches(open).count();
        let strip = match c {
            '.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '>' => true,
            ')' => unbalanced('('),
            ']' => unbalanced('['),
            _ => false,
        };
        if !strip {
            break;
        }
        t = &t[..t.len() - c.len_utf8()];
    }
    t
}

fn has_scheme(s: &str) -> bool {
    match s.find("://") {
        Some(idx) => s[..idx].chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.'),
        None => {
            // Schemes without "//" such as javascript: or file: are invalid for us,
            // but we still detect them so the error message is precise.
            let lower = s.to_ascii_lowercase();
            ["javascript:", "data:", "file:", "mailto:", "about:"].iter().any(|p| lower.starts_with(p))
        }
    }
}

fn looks_like_host(s: &str) -> bool {
    let host_part = s.split(['/', '?', '#']).next().unwrap_or("");
    let host_only = host_part.rsplit('@').next().unwrap_or(host_part);
    let host_only = host_only.split(':').next().unwrap_or(host_only);
    host_only.contains('.')
        && !host_only.starts_with('.')
        && !host_only.ends_with('.')
        && host_only.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}

fn is_local_host(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") {
        return true;
    }
    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() || v4.is_broadcast()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80
            }
        };
    }
    false
}

/// Extract every valid URL from free-form text (one per line, or separated by
/// whitespace/commas). Duplicates are removed while preserving order.
/// Returns `(valid_urls, rejected_tokens)`.
pub fn extract_urls(text: &str) -> (Vec<Url>, Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for token in
        text.split(|c: char| c.is_whitespace() || c == ',' || c == ';').map(str::trim).filter(|t| !t.is_empty())
    {
        match validate_url(token) {
            Ok(u) => {
                if seen.insert(u.as_str().to_string()) {
                    ok.push(u);
                }
            }
            Err(_) => bad.push(token.to_string()),
        }
    }
    (ok, bad)
}

/// Heuristic used for UX hints only (the real answer comes from yt-dlp):
/// does the URL look like it points at a playlist/channel/album?
pub fn looks_like_playlist(url: &Url) -> bool {
    let path = url.path().to_ascii_lowercase();
    let has_list_param = url.query_pairs().any(|(k, _)| k == "list");
    has_list_param
        || path.contains("/playlist")
        || path.contains("/sets/")
        || path.contains("/album/")
        || path.starts_with("/@") && !path.contains("/video/")
        || path.contains("/channel/")
        || path.contains("/c/")
        || path.contains("/user/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https() {
        let u = validate_url("https://www.youtube.com/watch?v=abc").unwrap();
        assert_eq!(u.host_str(), Some("www.youtube.com"));
        assert_eq!(u.query(), Some("v=abc"));
    }

    #[test]
    fn adds_missing_scheme() {
        let u = validate_url("  youtu.be/xyz  ").unwrap();
        assert_eq!(u.as_str(), "https://youtu.be/xyz");
    }

    #[test]
    fn strips_quotes_and_brackets() {
        let u = validate_url("\"https://vimeo.com/1\"").unwrap();
        assert_eq!(u.as_str(), "https://vimeo.com/1");
        let u = validate_url("<https://vimeo.com/2>").unwrap();
        assert_eq!(u.as_str(), "https://vimeo.com/2");
    }

    #[test]
    fn lowercases_host() {
        let u = validate_url("HTTPS://WWW.Example.COM/Path").unwrap();
        assert_eq!(u.as_str(), "https://www.example.com/Path");
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(validate_url(""), Err(UrlError::Empty));
        assert_eq!(validate_url("   "), Err(UrlError::Empty));
        assert_eq!(validate_url("hello world"), Err(UrlError::Invalid));
        assert_eq!(validate_url("notaurl"), Err(UrlError::Invalid));
        assert!(matches!(validate_url("ftp://example.com/file"), Err(UrlError::UnsupportedScheme(_))));
        assert!(matches!(
            validate_url("javascript:alert(1)"),
            Err(UrlError::UnsupportedScheme(_)) | Err(UrlError::Invalid)
        ));
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn rejects_local_addresses() {
        assert_eq!(validate_url("http://localhost:8080/x"), Err(UrlError::LocalAddress));
        assert_eq!(validate_url("http://127.0.0.1/x"), Err(UrlError::LocalAddress));
        assert_eq!(validate_url("http://192.168.1.10/x"), Err(UrlError::LocalAddress));
        assert_eq!(validate_url("http://10.0.0.1/x"), Err(UrlError::LocalAddress));
        assert_eq!(validate_url("http://[::1]/x"), Err(UrlError::LocalAddress));
        assert_eq!(validate_url("http://printer.local/"), Err(UrlError::LocalAddress));
    }

    #[test]
    fn extracts_batch_and_dedupes() {
        let text = "https://a.com/1\nhttps://b.com/2, https://a.com/1\n\ngarbage\n  c.org/3 ";
        let (ok, bad) = extract_urls(text);
        let s: Vec<_> = ok.iter().map(|u| u.as_str()).collect();
        assert_eq!(s, vec!["https://a.com/1", "https://b.com/2", "https://c.org/3"]);
        assert_eq!(bad, vec!["garbage"]);
    }

    #[test]
    fn strips_sentence_punctuation() {
        assert_eq!(validate_url("(https://youtu.be/jNQXAC9IVRw).").unwrap().as_str(), "https://youtu.be/jNQXAC9IVRw");
        assert_eq!(validate_url("https://vimeo.com/1,").unwrap().as_str(), "https://vimeo.com/1");
        assert_eq!(
            validate_url("https://en.wikipedia.org/wiki/Foo_(bar)").unwrap().as_str(),
            "https://en.wikipedia.org/wiki/Foo_(bar)"
        );
        let (ok, bad) = extract_urls("Watch (https://a.com/1). Also https://b.com/2!");
        let s: Vec<_> = ok.iter().map(|u| u.as_str()).collect();
        assert_eq!(s, vec!["https://a.com/1", "https://b.com/2"]);
        assert_eq!(bad, vec!["Watch", "Also"]);
    }

    #[test]
    fn playlist_heuristic() {
        let p = validate_url("https://www.youtube.com/playlist?list=PL123").unwrap();
        assert!(looks_like_playlist(&p));
        let v = validate_url("https://www.youtube.com/watch?v=abc").unwrap();
        assert!(!looks_like_playlist(&v));
        let sc = validate_url("https://soundcloud.com/artist/sets/album").unwrap();
        assert!(looks_like_playlist(&sc));
    }
}
