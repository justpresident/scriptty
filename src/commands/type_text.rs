//! [`TypeText`] command — simulates human typing character by character.
//!
//! Script syntax: `type "text here"`

use crate::command::{Context, ScripttyCommand};
use crate::parser::parse_quoted_string;
use anyhow::Result;
use async_trait::async_trait;
use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

/// Simulates human typing by sending `text` to the PTY one character at a time
/// with random per-character delays, then submits the line with a newline.
///
/// The PTY's own echo produces the visible output, so each character appears
/// exactly once regardless of the delay.
pub struct TypeText {
    pub text: String,
    pub min_delay: Duration,
    pub max_delay: Duration,
}

impl TypeText {
    pub const NAME: &'static str = "type";

    /// Create a `TypeText` command with default timing (50–150 ms per character).
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_timing(text, Duration::from_millis(50), Duration::from_millis(150))
    }

    /// Create a `TypeText` command with custom per-character timing.
    pub fn with_timing(text: impl Into<String>, min_delay: Duration, max_delay: Duration) -> Self {
        Self {
            text: text.into(),
            min_delay,
            max_delay,
        }
    }
}

#[async_trait(?Send)]
impl ScripttyCommand for TypeText {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn parse(args: &str) -> Result<Self> {
        Ok(Self::new(parse_quoted_string(args)?))
    }

    async fn execute(&self, ctx: &mut Context) -> Result<()> {
        for ch in self.text.chars() {
            ctx.write_to_pty(ch.to_string().as_bytes())?;
            // Drop rng before the await so it does not cross the yield point.
            let delay_ms = {
                let mut rng = rand::thread_rng();
                rng.gen_range(self.min_delay.as_millis()..=self.max_delay.as_millis())
            };
            sleep(Duration::from_millis(delay_ms as u64)).await;
        }

        // Longer pause after the last character before submitting.
        sleep(Duration::from_millis(2 * self.max_delay.as_millis() as u64)).await;
        ctx.write_to_pty(b"\n")?;
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScripttyCommand;

    #[test]
    fn test_parse() {
        let cmd = TypeText::parse(r#""hello world""#).unwrap();
        assert_eq!(cmd.text, "hello world");
    }

    #[test]
    fn test_parse_escaped_quotes() {
        let cmd = TypeText::parse(r#""hello \"world\"""#).unwrap();
        assert_eq!(cmd.text, r#"hello "world""#);
    }

    #[test]
    fn test_parse_newline_escape() {
        let cmd = TypeText::parse(r#""line1\nline2""#).unwrap();
        assert_eq!(cmd.text, "line1\nline2");
    }

    #[test]
    fn test_default_timing() {
        let cmd = TypeText::new("hello");
        assert_eq!(cmd.min_delay, Duration::from_millis(50));
        assert_eq!(cmd.max_delay, Duration::from_millis(150));
    }

    #[test]
    fn test_custom_timing() {
        let cmd = TypeText::with_timing("hi", Duration::from_millis(10), Duration::from_millis(20));
        assert_eq!(cmd.min_delay, Duration::from_millis(10));
        assert_eq!(cmd.max_delay, Duration::from_millis(20));
    }
}
