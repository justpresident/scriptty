//! [`Expect`] command — blocks until a pattern appears in the PTY output.
//!
//! Script syntax:
//! - `expect "$ "` — 5-second default timeout
//! - `expect "Password:" 10s` — custom timeout

use crate::command::{Context, ScripttyCommand};
use crate::parser::{parse_duration, parse_quoted_string};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::time::Duration;

/// Blocks until `pattern` appears in the PTY output, or until `timeout` elapses.
///
/// When the pattern is found the output buffer is consumed up to and including
/// it, so a subsequent `Expect` will not match the same occurrence again.
pub struct Expect {
    pub pattern: String,
    pub timeout: Duration,
}

impl Expect {
    pub const NAME: &'static str = "expect";

    /// Create an `Expect` command with the default 5-second timeout.
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            timeout: Duration::from_secs(5),
        }
    }

    /// Create an `Expect` command with a custom timeout.
    pub fn with_timeout(pattern: impl Into<String>, timeout: Duration) -> Self {
        Self {
            pattern: pattern.into(),
            timeout,
        }
    }
}

#[async_trait(?Send)]
impl ScripttyCommand for Expect {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn parse(args: &str) -> Result<Self> {
        let args = args.trim();
        if !args.starts_with('"') {
            return Err(anyhow!("Expected quoted string after 'expect'"));
        }

        // Locate the closing quote, respecting backslash escapes.
        let mut escaped = false;
        let mut end_idx = None;
        for (i, ch) in args.chars().enumerate().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                end_idx = Some(i);
                break;
            }
        }

        let end_idx = end_idx.ok_or_else(|| anyhow!("Unclosed quote in expect command"))?;
        let pattern = parse_quoted_string(&args[..=end_idx])?;
        let remainder = args[end_idx + 1..].trim();

        if remainder.is_empty() {
            Ok(Self::new(pattern))
        } else {
            Ok(Self::with_timeout(pattern, parse_duration(remainder)?))
        }
    }

    async fn execute(&self, ctx: &mut Context) -> Result<()> {
        ctx.wait_for_pattern(&self.pattern, self.timeout).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScripttyCommand;

    #[test]
    fn test_parse_default_timeout() {
        let cmd = Expect::parse(r#""$ ""#).unwrap();
        assert_eq!(cmd.pattern, "$ ");
        assert_eq!(cmd.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_parse_custom_timeout() {
        let cmd = Expect::parse(r#""hello world" 2s"#).unwrap();
        assert_eq!(cmd.pattern, "hello world");
        assert_eq!(cmd.timeout, Duration::from_secs(2));
    }

    #[test]
    fn test_parse_ms_timeout() {
        let cmd = Expect::parse(r#""Ready" 500ms"#).unwrap();
        assert_eq!(cmd.timeout, Duration::from_millis(500));
    }

    #[test]
    fn test_parse_unclosed_quote() {
        assert!(Expect::parse(r#""unclosed"#).is_err());
    }

    #[test]
    fn test_parse_missing_quote() {
        assert!(Expect::parse("no_quotes").is_err());
    }
}
