//! Script parser for the scriptty scripting language.
//!
//! The top-level entry points are [`parse_str`] and [`parse_file`].

use crate::command::ScripttyCommand;
use crate::commands::{Expect, KeyPress, SendInput, Show, TypeText, Wait};
use anyhow::{Context as _, Result, anyhow};
use std::path::Path;
use std::time::Duration;

/// Parse a scriptty script from a string slice and return the resulting commands.
///
/// Lines that are empty or start with `#` are ignored. Inline comments (` # …`)
/// are stripped while preserving `#` characters inside quoted strings.
///
/// # Errors
///
/// Returns an error if any line contains an unknown command, a malformed
/// argument, or an unclosed quoted string.
///
/// # Example
///
/// ```
/// use scriptty::parse_str;
///
/// let commands = parse_str("wait 500ms\ntype \"hello world\"\n").unwrap();
/// assert_eq!(commands.len(), 2);
/// ```
pub fn parse_str(content: &str) -> Result<Vec<Box<dyn ScripttyCommand>>> {
    let mut commands = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = strip_inline_comment(line);
        let cmd = parse_line(line)
            .with_context(|| format!("Failed to parse line {}: {}", line_num + 1, line))?;
        commands.push(cmd);
    }
    Ok(commands)
}

/// Parse a scriptty script from a file and return the resulting commands.
///
/// Reads the entire file into memory and delegates to [`parse_str`].
///
/// # Errors
///
/// Returns an error if the file cannot be read or if the script is malformed.
///
/// # Example
///
/// ```no_run
/// use scriptty::parse_file;
///
/// let commands = parse_file("my_script.script").unwrap();
/// ```
pub fn parse_file(path: impl AsRef<Path>) -> Result<Vec<Box<dyn ScripttyCommand>>> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read script file: {}", path.display()))?;
    parse_str(&content)
}

type ParseFn = fn(&str) -> Result<Box<dyn ScripttyCommand>>;

static REGISTRY: &[(&str, ParseFn)] = &[
    (TypeText::NAME, TypeText::parse_boxed),
    (SendInput::NAME, SendInput::parse_boxed),
    (Show::NAME, Show::parse_boxed),
    (Wait::NAME, Wait::parse_boxed),
    (Expect::NAME, Expect::parse_boxed),
    (KeyPress::NAME, KeyPress::parse_boxed),
];

/// Dispatch a single non-empty, non-comment line to the matching command's parser.
///
/// To add a new command, add one entry to [`REGISTRY`] using the command's
/// `NAME` constant and `parse_boxed` function pointer.
fn parse_line(line: &str) -> Result<Box<dyn ScripttyCommand>> {
    let (name, args) = line.split_once(' ').unwrap_or((line, ""));
    REGISTRY
        .iter()
        .find(|(cmd_name, _)| *cmd_name == name)
        .map(|(_, parse)| parse(args))
        .unwrap_or_else(|| Err(anyhow!("Unknown command: {}", line)))
}

/// Strip inline comments from a line, preserving `#` inside quoted strings.
fn strip_inline_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if ch == '#' && !in_quotes {
            return line[..i].trim();
        }
    }
    line
}

/// Parse a duration string: `1s`, `500ms`, `1.5s`.
pub(crate) fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if let Some(ms_str) = s.strip_suffix("ms") {
        let ms: u64 = ms_str
            .trim()
            .parse()
            .context("Invalid milliseconds value")?;
        Ok(Duration::from_millis(ms))
    } else if let Some(s_str) = s.strip_suffix('s') {
        let secs: f64 = s_str.trim().parse().context("Invalid seconds value")?;
        Ok(Duration::from_secs_f64(secs))
    } else {
        Err(anyhow!("Duration must end with 's' or 'ms', got: {}", s))
    }
}

/// Parse a double-quoted string, processing `\n`, `\t`, `\"`, and `\\`.
pub(crate) fn parse_quoted_string(s: &str) -> Result<String> {
    let s = s.trim();
    if !s.starts_with('"') {
        return Err(anyhow!("Expected string to start with '\"'"));
    }
    if !s.ends_with('"') {
        return Err(anyhow!("Expected string to end with '\"'"));
    }
    Ok(s[1..s.len() - 1]
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("1s").unwrap(), Duration::from_secs(1));
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(
            parse_duration("1.5s").unwrap(),
            Duration::from_secs_f64(1.5)
        );
    }

    #[test]
    fn test_parse_quoted_string() {
        assert_eq!(parse_quoted_string("\"hello\"").unwrap(), "hello");
        assert_eq!(
            parse_quoted_string("\"hello world\"").unwrap(),
            "hello world"
        );
        assert_eq!(
            parse_quoted_string("\"hello\\nworld\"").unwrap(),
            "hello\nworld"
        );
    }

    #[test]
    fn test_parse_str() {
        let cmds = parse_str("wait 1s\ntype \"hello\"\nwait 500ms\nsend \"cmd\"\n").unwrap();
        assert_eq!(cmds.len(), 4);
    }

    #[test]
    fn test_parse_all_commands() {
        let cmds = parse_str(
            "wait 500ms\ntype \"cmd\"\nsend \"instant\"\nexpect \"out\"\nshow \"note\"\nkey Enter\n",
        )
        .unwrap();
        assert_eq!(cmds.len(), 6);
        assert_eq!(cmds[0].name(), "wait");
        assert_eq!(cmds[1].name(), "type");
        assert_eq!(cmds[2].name(), "send");
        assert_eq!(cmds[3].name(), "expect");
        assert_eq!(cmds[4].name(), "show");
        assert_eq!(cmds[5].name(), "key");
    }

    #[test]
    fn test_parse_key_command() {
        let cmds = parse_str("key Enter\nkey Ctrl+C\n").unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].name(), "key");
        assert_eq!(cmds[1].name(), "key");
    }

    #[test]
    fn test_parse_comments_only() {
        assert_eq!(parse_str("# c1\n# c2\n").unwrap().len(), 0);
    }

    #[test]
    fn test_parse_empty_lines() {
        let cmds = parse_str("\n\nwait 1s\n\ntype \"test\"\n\n").unwrap();
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn test_parse_invalid_command() {
        let err = parse_str("unknown_command \"test\"")
            .err()
            .unwrap()
            .to_string();
        assert!(
            err.contains("Unknown command") || err.contains("unknown_command"),
            "got: {err}"
        );
    }

    #[test]
    fn test_parse_invalid_duration() {
        assert!(parse_str("wait 5minutes").is_err());
    }

    #[test]
    fn test_parse_unclosed_quote() {
        assert!(parse_str("type \"unclosed").is_err());
    }

    #[test]
    fn test_strip_inline_comments() {
        assert_eq!(strip_inline_comment("wait 1s # comment"), "wait 1s");
        assert_eq!(
            strip_inline_comment("type \"test\" # inline"),
            "type \"test\""
        );
        assert_eq!(
            strip_inline_comment("type \"#hashtag\""),
            "type \"#hashtag\""
        );
        assert_eq!(
            strip_inline_comment("type \"test#1\" # comment"),
            "type \"test#1\""
        );
    }

    #[test]
    fn test_parse_with_inline_comments() {
        let cmds = parse_str("wait 1s # delay\ntype \"hi\" # greet\nexpect \"ok\" 2s\n").unwrap();
        assert_eq!(cmds.len(), 3);
    }
}
