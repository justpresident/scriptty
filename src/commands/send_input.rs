//! [`SendInput`] command — sends bytes to the program's stdin instantly.
//!
//! Script syntax: `send "text here"`

use crate::command::{Context, ScripttyCommand};
use crate::parser::parse_quoted_string;
use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;

/// Sends bytes to the program's stdin immediately without any visible output.
///
/// A newline is appended so the program receives a complete line.
pub struct SendInput {
    pub data: Vec<u8>,
}

impl SendInput {
    pub const NAME: &'static str = "send";

    /// Create a `SendInput` command. A newline is appended automatically.
    pub fn new(text: impl Into<String>) -> Self {
        let mut bytes = text.into().into_bytes();
        bytes.push(b'\n');
        Self { data: bytes }
    }
}

#[async_trait(?Send)]
impl ScripttyCommand for SendInput {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn parse(args: &str) -> Result<Self> {
        Ok(Self::new(parse_quoted_string(args)?))
    }

    async fn execute(&self, ctx: &mut Context) -> Result<()> {
        ctx.write_to_pty(&self.data)?;
        // Give the program a moment to process the input.
        sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScripttyCommand;

    #[test]
    fn test_parse() {
        let cmd = SendInput::parse(r#""hello""#).unwrap();
        assert_eq!(cmd.data, b"hello\n");
    }

    #[test]
    fn test_newline_appended() {
        let cmd = SendInput::new("cmd");
        assert_eq!(cmd.data.last(), Some(&b'\n'));
    }
}
