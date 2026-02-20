//! The [`ScripttyCommand`] trait and the [`Context`] type commands receive when executed.

use crate::pty::PtySession;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type OutputHandler = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Execution context passed to [`ScripttyCommand::execute`].
///
/// Provides access to the PTY stdin, the output handler, and the rolling output
/// buffer used by pattern-matching commands.
pub struct Context {
    pub(crate) pty: PtySession,
    pub(crate) output_buffer: Arc<Mutex<String>>,
    pub(crate) output_handler: OutputHandler,
}

impl Context {
    /// Write raw bytes to the program's stdin.
    pub fn write_to_pty(&mut self, data: &[u8]) -> Result<()> {
        self.pty.write(data)
    }

    /// Pass bytes through the output handler (e.g. to stdout or a custom sink).
    pub fn emit(&self, data: &[u8]) {
        (self.output_handler)(data);
    }

    /// Block until `pattern` appears in the rolling output buffer, or until
    /// `timeout` elapses.
    ///
    /// Once found, the buffer is consumed up to and including the pattern so
    /// subsequent calls do not match the same occurrence.
    pub async fn wait_for_pattern(&self, pattern: &str, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let mut buffer = self.output_buffer.lock().unwrap();
                if let Some(idx) = buffer.find(pattern) {
                    buffer.drain(..idx + pattern.len());
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for pattern: '{}'",
                    pattern
                ));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// A single scriptty script command.
///
/// Implement this trait to add a new command to the engine. Then:
///
/// 1. Define `pub const NAME: &'static str` on your struct — the script
///    keyword (e.g. `"type"`, `"expect"`) used by the parser.
/// 2. Re-export the struct from `src/commands/mod.rs`.
/// 3. Add one entry to the `REGISTRY` in [`crate::parser`]:
///    `(MyCmd::NAME, MyCmd::parse_boxed)`.
#[async_trait(?Send)]
pub trait ScripttyCommand: 'static {
    /// The command name, accessible at runtime through a trait object.
    ///
    /// Implementations should return their `NAME` constant:
    /// `fn name(&self) -> &'static str { Self::NAME }`.
    fn name(&self) -> &'static str;

    /// Parse this command from the argument string (everything after the
    /// command keyword on the script line).
    fn parse(args: &str) -> Result<Self>
    where
        Self: Sized;

    /// Parse and box this command. Used as the function-pointer type stored in
    /// the command registry; the default implementation calls [`parse`](Self::parse)
    /// and boxes the result.
    fn parse_boxed(args: &str) -> Result<Box<dyn ScripttyCommand>>
    where
        Self: Sized,
    {
        Ok(Box::new(Self::parse(args)?))
    }

    /// Execute the command using the provided engine context.
    async fn execute(&self, ctx: &mut Context) -> Result<()>;
}
