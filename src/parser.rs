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

        let event = parse_line(line)
            .with_context(|| format!("Failed to parse line {}: {}", line_num + 1, line))?;

        events.push(event);
    }

    Ok(events)
}

/// Parse a single line into an event
fn parse_line(line: &str) -> Result<Event> {
    if let Some(rest) = line.strip_prefix("wait ") {
        parse_wait(rest)
    } else if let Some(rest) = line.strip_prefix("type ") {
        parse_type(rest)
    } else if let Some(rest) = line.strip_prefix("send ") {
        parse_send(rest)
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
}
