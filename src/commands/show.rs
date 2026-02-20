//! [`Show`] command — writes text directly to the output handler.
//!
//! Script syntax: `show "This is a note"`

use crate::command::{Context, ScripttyCommand};
use crate::parser::parse_quoted_string;
use anyhow::Result;
use async_trait::async_trait;

/// Writes text directly to the output handler without sending anything to the program.
///
/// Useful for inserting annotations or commentary into the output stream.
pub struct Show {
    pub data: Vec<u8>,
}

impl Show {
    pub const NAME: &'static str = "show";

    /// Create a `Show` command from a string. A newline is appended automatically.
    pub fn new(text: impl Into<String>) -> Self {
        let mut t = text.into();
        t.push('\n');
        Self {
            data: t.into_bytes(),
        }
    }
}

#[async_trait(?Send)]
impl ScripttyCommand for Show {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn parse(args: &str) -> Result<Self> {
        Ok(Self::new(parse_quoted_string(args)?))
    }

    async fn execute(&self, ctx: &mut Context) -> Result<()> {
        ctx.emit(&self.data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::ScripttyCommand;

    #[test]
    fn test_parse() {
        let cmd = Show::parse(r#""hello world""#).unwrap();
        let text = String::from_utf8_lossy(&cmd.data);
        assert!(text.contains("hello world"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn test_parse_unclosed_quote() {
        assert!(Show::parse(r#""unclosed"#).is_err());
    }
}
