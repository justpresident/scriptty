use crate::event::Event;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;

/// Parse a script file into a sequence of events
pub fn parse_script(content: &str) -> Result<Vec<Event>> {
    let mut events = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Remove inline comments (but preserve # inside quoted strings)
        let line = strip_inline_comment(line);

        let event = parse_line(line)
            .with_context(|| format!("Failed to parse line {}: {}", line_num + 1, line))?;

        events.push(event);
    }

    Ok(events)
}

/// Strip inline comments from a line, preserving # inside quoted strings
fn strip_inline_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;

    for (i, ch) in line.chars().enumerate() {
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

/// Parse a single line into an event
fn parse_line(line: &str) -> Result<Event> {
    if let Some(rest) = line.strip_prefix("wait ") {
        parse_wait(rest)
    } else if let Some(rest) = line.strip_prefix("type ") {
        parse_type(rest)
    } else if let Some(rest) = line.strip_prefix("send ") {
        parse_send(rest)
    } else if let Some(rest) = line.strip_prefix("expect ") {
        parse_expect(rest)
    } else {
        Err(anyhow!("Unknown command: {}", line))
    }
}

/// Parse a wait command: "wait 1s", "wait 500ms"
fn parse_wait(rest: &str) -> Result<Event> {
    let duration = parse_duration(rest)?;
    Ok(Event::sleep(duration))
}

/// Parse a type command: type "text here"
fn parse_type(rest: &str) -> Result<Event> {
    let text = parse_quoted_string(rest)?;
    Ok(Event::type_text(text))
}

/// Parse a send command: send "text here"
fn parse_send(rest: &str) -> Result<Event> {
    let text = parse_quoted_string(rest)?;
    Ok(Event::send(text))
}

/// Parse an expect command: expect "pattern" [timeout]
/// Examples: expect "$ ", expect "Password:" 10s
fn parse_expect(rest: &str) -> Result<Event> {
    let rest = rest.trim();

    // Find the end of the quoted string
    if !rest.starts_with('"') {
        return Err(anyhow!("Expected quoted string after 'expect'"));
    }

    // Find the closing quote
    let mut escaped = false;
    let mut end_quote_idx = None;
    for (i, ch) in rest.chars().enumerate().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            end_quote_idx = Some(i);
            break;
        }
    }

    let end_idx = end_quote_idx.ok_or_else(|| anyhow!("Unclosed quote in expect command"))?;

    let pattern = parse_quoted_string(&rest[..=end_idx])?;
    let remainder = rest[end_idx + 1..].trim();

    if remainder.is_empty() {
        // No timeout specified, use default
        Ok(Event::expect(pattern))
    } else {
        // Parse timeout
        let timeout = parse_duration(remainder)?;
        Ok(Event::expect_with_timeout(pattern, timeout))
    }
}

/// Parse duration from strings like "1s", "500ms", "1.5s"
fn parse_duration(s: &str) -> Result<Duration> {
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
        Err(anyhow!(
            "Duration must end with 's' or 'ms', got: {}",
            s
        ))
    }
}

/// Parse a quoted string: "hello world" -> hello world
fn parse_quoted_string(s: &str) -> Result<String> {
    let s = s.trim();

    if !s.starts_with('"') {
        return Err(anyhow!("Expected string to start with '\"'"));
    }

    if !s.ends_with('"') {
        return Err(anyhow!("Expected string to end with '\"'"));
    }

    let content = &s[1..s.len() - 1];

    // Handle basic escape sequences
    let content = content
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");

    Ok(content)
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
        assert_eq!(parse_quoted_string("\"hello\\nworld\"").unwrap(), "hello\nworld");
    }

    #[test]
    fn test_parse_script() {
        let script = r#"
# This is a comment
wait 1s
type "hello world"
wait 500ms
send "command"
"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_parse_expect() {
        let script = r#"
expect "$ "
expect "Password:" 10s
expect "Ready" 500ms
"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_parse_empty_lines() {
        let script = r#"


wait 1s


type "test"


"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_parse_comments_only() {
        let script = r#"
# Comment 1
# Comment 2
# Comment 3
"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_parse_escaped_strings() {
        let script = r#"type "hello \"world\"""#;
        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            Event::TypeText { text, .. } => {
                assert_eq!(text, "hello \"world\"");
            }
            _ => panic!("Expected TypeText event"),
        }
    }

    #[test]
    fn test_parse_newlines_in_strings() {
        let script = r#"type "line1\nline2""#;
        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            Event::TypeText { text, .. } => {
                assert_eq!(text, "line1\nline2");
            }
            _ => panic!("Expected TypeText event"),
        }
    }

    #[test]
    fn test_parse_invalid_duration() {
        let script = r#"wait 5minutes"#;
        let result = parse_script(script);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_command() {
        let script = r#"unknown_command "test""#;
        let result = parse_script(script);
        assert!(result.is_err());
        // The error message includes context, so check for a broader match
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unknown command") || err_msg.contains("unknown_command"),
            "Expected error about unknown command, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_parse_unclosed_quote() {
        let script = r#"type "unclosed"#;
        let result = parse_script(script);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fractional_seconds() {
        let script = r#"wait 1.5s"#;
        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            Event::Sleep(duration) => {
                assert_eq!(*duration, Duration::from_secs_f64(1.5));
            }
            _ => panic!("Expected Sleep event"),
        }
    }

    #[test]
    fn test_parse_expect_with_spaces() {
        let script = r#"expect "hello world" 2s"#;
        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            Event::Expect { pattern, timeout } => {
                assert_eq!(pattern, "hello world");
                assert_eq!(*timeout, Duration::from_secs(2));
            }
            _ => panic!("Expected Expect event"),
        }
    }

    #[test]
    fn test_parse_mixed_script() {
        let script = r#"
# Start of script
wait 500ms

# Type something
type "hello"

# Wait for response
expect "prompt"

# Send command
send "exit"
"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_strip_inline_comments() {
        assert_eq!(strip_inline_comment("wait 1s # comment"), "wait 1s");
        assert_eq!(strip_inline_comment("type \"test\" # inline"), "type \"test\"");
        assert_eq!(strip_inline_comment("type \"#hashtag\""), "type \"#hashtag\"");
        assert_eq!(
            strip_inline_comment("type \"test#1\" # comment"),
            "type \"test#1\""
        );
        assert_eq!(strip_inline_comment("# full comment"), "");
    }

    #[test]
    fn test_parse_with_inline_comments() {
        let script = r#"
wait 1s # Wait a bit
type "hello" # Type greeting
expect "world" 2s # Wait for response
"#;

        let events = parse_script(script).unwrap();
        assert_eq!(events.len(), 3);
    }
}
