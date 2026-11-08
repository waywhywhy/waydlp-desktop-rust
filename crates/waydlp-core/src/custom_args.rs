//! Parsing of user-supplied extra yt-dlp arguments.
//!
//! Arguments are split with POSIX shell-word rules but are **never** passed
//! through a shell: each word becomes one argv entry. Options that make
//! yt-dlp execute arbitrary commands are rejected, because a settings file
//! imported from elsewhere could otherwise run code on the user's machine.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CustomArgsError {
    #[error("Could not parse the extra arguments: unbalanced quotes.")]
    Unbalanced,
    #[error(
        "The option \"{0}\" runs external commands and is not allowed in extra arguments. \
         Use waydlp's post-download actions instead."
    )]
    Forbidden(String),
    #[error("The option \"{0}\" is managed by waydlp and cannot be overridden here.")]
    Managed(String),
}

/// Options that execute commands or otherwise escape the sandbox we expect.
const FORBIDDEN: &[&str] = &[
    "--exec",
    "--exec-before-download",
    "--netrc-cmd",
    "--config-locations",
    "--config-location",
    "--plugin-dirs",
    "--batch-file",
    "-a",
    "--load-info-json",
];

/// Options waydlp needs to control so progress parsing keeps working.
const MANAGED: &[&str] = &[
    "--progress-template",
    "--print",
    "-O",
    "--quiet",
    "-q",
    "--no-progress",
    "--simulate",
    "-s",
    "--dump-json",
    "-j",
    "--dump-single-json",
    "-J",
    "--update",
    "-U",
    "--update-to",
];

pub fn parse_custom_args(input: &str) -> Result<Vec<String>, CustomArgsError> {
    let words = shell_words::split(input).map_err(|_| CustomArgsError::Unbalanced)?;
    for w in &words {
        let name = w.split('=').next().unwrap_or(w);
        if FORBIDDEN.contains(&name) {
            return Err(CustomArgsError::Forbidden(name.to_string()));
        }
        if MANAGED.contains(&name) {
            return Err(CustomArgsError::Managed(name.to_string()));
        }
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_without_shell() {
        let a = parse_custom_args(r#"--sponsorblock-remove all --match-filter "duration > 60""#).unwrap();
        assert_eq!(a, vec!["--sponsorblock-remove", "all", "--match-filter", "duration > 60"]);
        // Shell metacharacters are just characters.
        let a = parse_custom_args("--referer 'a;rm -rf ~'").unwrap();
        assert_eq!(a, vec!["--referer", "a;rm -rf ~"]);
    }

    #[test]
    fn rejects_exec() {
        assert_eq!(parse_custom_args("--exec 'echo hi'"), Err(CustomArgsError::Forbidden("--exec".into())));
        assert_eq!(parse_custom_args("--exec=calc"), Err(CustomArgsError::Forbidden("--exec".into())));
        assert!(parse_custom_args("--netrc-cmd x").is_err());
    }

    #[test]
    fn rejects_managed() {
        assert!(matches!(parse_custom_args("-q"), Err(CustomArgsError::Managed(_))));
        assert!(matches!(parse_custom_args("-U"), Err(CustomArgsError::Managed(_))));
    }

    #[test]
    fn unbalanced() {
        assert_eq!(parse_custom_args("--x 'abc"), Err(CustomArgsError::Unbalanced));
    }

    #[test]
    fn empty_is_ok() {
        assert!(parse_custom_args("   ").unwrap().is_empty());
    }
}
