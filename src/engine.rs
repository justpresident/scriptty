//! The [`Engine`] that executes [`ScripttyCommand`] sequences against a live PTY process.

use crate::command::{Context, ScripttyCommand};
use crate::pty::PtySession;
use anyhow::Result;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// Executes a sequence of [`ScripttyCommand`]s against a program running in a PTY.
///
/// Create an engine with [`Engine::spawn`] (output to stdout) or
/// [`Engine::spawn_with_handler`] (custom output sink), then call
/// [`Engine::execute`] with the commands produced by the parser.
pub struct Engine {
    ctx: Context,
    _output_task: tokio::task::JoinHandle<()>,
}

impl Engine {
    /// Spawn a new engine that runs `command` in a PTY and writes all output to stdout.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY cannot be opened or the command cannot be spawned.
    pub fn spawn<S: AsRef<str>>(command: &str, args: &[S]) -> Result<Self> {
        Self::spawn_with_handler(command, args, |data| {
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            stdout.write_all(data).ok();
            stdout.flush().ok();
        })
    }

    /// Spawn a new engine that runs `command` in a PTY and passes all output to `handler`.
    ///
    /// `handler` is called with raw bytes for every chunk of PTY output and every
    /// [`crate::commands::Show`] payload. This makes it straightforward to capture
    /// output, forward it over a network, or suppress it entirely.
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY cannot be opened or the command cannot be spawned.
    ///
    /// # Example
    ///
    /// Drive a `gdb` session against an imaginary binary, streaming every output
    /// line to a Kafka topic for observability while still showing live output to
    /// the developer. The script waits for the debugger prompt before each
    /// command, demonstrating real back-and-forth interaction with the process.
    ///
    /// ```no_run
    /// use scriptty::{Engine, parse_str};
    /// use std::io::Write;
    ///
    /// // Stand-in for a real Kafka producer (e.g. `rdkafka::producer::FutureProducer`).
    /// fn kafka_send(topic: &str, msg: &str) { /* ... */ }
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut engine = Engine::spawn_with_handler(
    ///         "gdb",
    ///         &["./my_hello_world"],
    ///         |data| {
    ///             let stdout = std::io::stdout();
    ///             let mut out = stdout.lock();
    ///             out.write_all(data).ok();
    ///             out.flush().ok();
    ///             for line in String::from_utf8_lossy(data).lines() {
    ///                 if !line.trim().is_empty() {
    ///                     kafka_send("debugger-trace", line.trim());
    ///                 }
    ///             }
    ///         },
    ///     )?;
    ///
    ///     let commands = parse_str(r#"
    /// expect "(gdb) "
    /// type "break hello_world"
    /// expect "Breakpoint 1"
    /// type "run"
    /// expect "Breakpoint 1, hello_world"
    /// type "backtrace"
    /// expect "(gdb) "
    /// send "quit"
    /// "#)?;
    ///
    ///     engine.execute(commands).await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn spawn_with_handler<S, F>(command: &str, args: &[S], handler: F) -> Result<Self>
    where
        S: AsRef<str>,
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let string_args: Vec<String> = args.iter().map(|s| s.as_ref().to_string()).collect();
        let (pty, reader) = PtySession::spawn(command, &string_args)?;
        let output_rx = crate::pty_reader::spawn_reader(reader);
        Ok(Self::from_parts(pty, output_rx, handler))
    }

    fn from_parts<F>(pty: PtySession, output_rx: Receiver<Vec<u8>>, handler: F) -> Self
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let output_buffer = Arc::new(Mutex::new(String::new()));
        let buffer_clone = output_buffer.clone();
        let handler = Arc::new(handler);
        let handler_clone = handler.clone();

        let output_task = tokio::task::spawn_blocking(move || {
            while let Ok(data) = output_rx.recv() {
                handler_clone(&data);
                if let Ok(mut buffer) = buffer_clone.lock() {
                    buffer.push_str(&String::from_utf8_lossy(&data));
                    if buffer.len() > 10_000 {
                        buffer.drain(..5_000);
                    }
                }
            }
        });

        Engine {
            ctx: Context {
                pty,
                output_buffer,
                output_handler: handler,
            },
            _output_task: output_task,
        }
    }

    /// Execute a sequence of commands in order.
    ///
    /// After the last command the engine waits briefly for any remaining PTY
    /// output to be flushed through the output handler before returning.
    pub async fn execute(&mut self, commands: Vec<Box<dyn ScripttyCommand>>) -> Result<()> {
        for cmd in commands {
            cmd.execute(&mut self.ctx).await?;
        }
        sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    /// Wait for the child process to exit.
    pub fn wait_for_exit(&mut self) -> Result<()> {
        self.ctx.pty.wait()
    }
}
