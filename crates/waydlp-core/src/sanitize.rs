//! Filename sanitisation and output-template validation.
//!
//! yt-dlp sanitises the fields it substitutes into templates itself. What we
//! must guard against is the *template* and the *directory* the user (or an
//! imported settings file) supplies: absolute paths and `..` components in a
//! template could write outside the chosen download folder.

use thiserror::Error;

const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Maximum length (in bytes) of a single path component we generate.
pub const MAX_COMPONENT_BYTES: usize = 200;

/// Make an arbitrary string safe to use as a single file or folder name on
/// both Windows and Linux.
pub fn sanitize_filename(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    // Trailing dots and spaces are not allowed on Windows.
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    let trimmed = out.trim_start().to_string();
    out = trimmed;

    if out.is_empty() || out == "." || out == ".." {
        out = "download".into();
    }

    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        out = format!("_{out}");
    }

    truncate_utf8(&out, MAX_COMPONENT_BYTES)
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].trim_end_matches(['.', ' ']).to_string()
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TemplateError {
    #[error("The filename template is empty.")]
    Empty,
    #[error("The filename template must be relative to the download folder.")]
    Absolute,
    #[error("The filename template may not contain \"..\" folders.")]
    ParentTraversal,
    #[error("The filename template contains a control character.")]
    ControlChar,
    #[error("The filename template must include a file name, not just a folder.")]
    NoFileName,
}

/// Validate a yt-dlp output template such as `%(title)s [%(id)s].%(ext)s`.
pub fn validate_output_template(template: &str) -> Result<(), TemplateError> {
    let t = template.trim();
    if t.is_empty() {
        return Err(TemplateError::Empty);
    }
    if t.chars().any(|c| c.is_control()) {
        return Err(TemplateError::ControlChar);
    }
    if t.starts_with('/')
        || t.starts_with('\\')
        || t.starts_with('~')
        || (t.len() >= 2 && t.as_bytes()[1] == b':' && t.as_bytes()[0].is_ascii_alphabetic())
    {
        return Err(TemplateError::Absolute);
    }
    // Only check literal path components; fields like %(title)s are sanitised
    // by yt-dlp and cannot introduce separators.
    for comp in t.split(['/', '\\']) {
        if comp.trim() == ".." {
            return Err(TemplateError::ParentTraversal);
        }
    }
    if t.ends_with('/') || t.ends_with('\\') {
        return Err(TemplateError::NoFileName);
    }
    Ok(())
}

/// The default template used when the user hasn't configured one.
pub const DEFAULT_TEMPLATE: &str = "%(title).180B [%(id)s].%(ext)s";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_reserved_characters() {
        assert_eq!(sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(sanitize_filename("tab\there"), "tab_here");
    }

    #[test]
    fn trims_and_handles_empty() {
        assert_eq!(sanitize_filename("  name. . "), "name");
        assert_eq!(sanitize_filename(""), "download");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("..."), "download");
    }

    #[test]
    fn windows_reserved_names() {
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("nul.txt"), "_nul.txt");
        assert_eq!(sanitize_filename("console"), "console");
    }

    #[test]
    fn truncates_on_char_boundary() {
        let long = "é".repeat(300);
        let s = sanitize_filename(&long);
        assert!(s.len() <= MAX_COMPONENT_BYTES);
        assert!(s.chars().all(|c| c == 'é'));
    }

    #[test]
    fn template_validation() {
        assert!(validate_output_template(DEFAULT_TEMPLATE).is_ok());
        assert!(validate_output_template("%(uploader)s/%(title)s.%(ext)s").is_ok());
        assert_eq!(validate_output_template(""), Err(TemplateError::Empty));
        assert_eq!(validate_output_template("/etc/x.%(ext)s"), Err(TemplateError::Absolute));
        assert_eq!(validate_output_template("C:\\x.%(ext)s"), Err(TemplateError::Absolute));
        assert_eq!(validate_output_template("~/x.%(ext)s"), Err(TemplateError::Absolute));
        assert_eq!(validate_output_template("../%(title)s.%(ext)s"), Err(TemplateError::ParentTraversal));
        assert_eq!(validate_output_template("a/../../b.%(ext)s"), Err(TemplateError::ParentTraversal));
        assert_eq!(validate_output_template("folder/"), Err(TemplateError::NoFileName));
        assert_eq!(validate_output_template("a\nb"), Err(TemplateError::ControlChar));
    }
}
