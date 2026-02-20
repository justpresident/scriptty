//! [`Wait`] command — pauses execution for a fixed duration.
//!
//! Script syntax: `wait 500ms` or `wait 1.5s`

use crate::command::{Context, ScripttyCommand};
use crate::parser::parse_duration;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

/// Pauses execution for a fixed duration before running the next command.
pub struct Wait {
    pub duration: Duration,
}

impl Wait {
    pub const NAME: &'static str = "wait";
}

#[async_trait(?Send)]
impl ScripttyCommand for Wait {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn parse(args: &str) -> Result<Self> {
        Ok(Self {
            duration: parse_duration(args)?,
        })
    }

    async fn execute(&self, _ctx: &mut Context) -> Result<()> {
        tokio::time::sleep(self.duration).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScripttyCommand;

    #[test]
    fn test_parse_seconds() {
        assert_eq!(Wait::parse("1s").unwrap().duration, Duration::from_secs(1));
    }

    #[test]
    fn test_parse_millis() {
        assert_eq!(
            Wait::parse("500ms").unwrap().duration,
            Duration::from_millis(500)
        );
    }

    #[test]
    fn test_parse_fractional() {
        assert_eq!(
            Wait::parse("1.5s").unwrap().duration,
            Duration::from_secs_f64(1.5)
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Wait::parse("5minutes").is_err());
    }
}
