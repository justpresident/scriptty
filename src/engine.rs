//! The [`Engine`] that executes scriptty [`Event`] sequences against a live PTY process.

use crate::event::Event;
use crate::pty::PtySession;
use anyhow::Result;
use rand::Rng;
use std::io::Write;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// Executes a sequence of [`Event`]s against a program running in a PTY.
///
/// Create an engine with [`Engine::spawn`] (output to stdout) or
/// [`Engine::spawn_with_handler`] (custom output sink), then call
/// [`Engine::execute`] with the events produced by the parser.
///
/// The engine maintains a rolling output buffer used by [`Event::Expect`] for
/// pattern matching. All PTY output, simulated keystrokes, and [`Event::ShowToUser`]
/// data are routed through the same output handler so callers receive a unified
/// stream regardless of the event type.
pub struct Engine {
    pty: PtySession,
    output_buffer: Arc<Mutex<String>>,
    _output_task: tokio::task::JoinHandle<()>,
    output_handler: Arc<dyn Fn(&[u8]) + Send + Sync>,
}

impl Engine {
    /// Spawn a new engine that runs `command` in a PTY and writes all output to stdout.
    ///
    /// # Arguments
    ///
    /// * `command` – the program to run (e.g. `"bash"`, `"python3"`)
    /// * `args` – arguments forwarded to the program
    ///
    /// # Errors
    ///
    /// Returns an error if the PTY cannot be opened or the command cannot be
    /// spawned.
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
    /// `handler` is called with raw bytes for every chunk of PTY output, every
    /// character emitted during a [`Event::TypeText`] event, and every
    /// [`Event::ShowToUser`] payload. This makes it straightforward to capture
    /// output, forward it over a network, or suppress it entirely.
    ///
    /// # Arguments
    ///
    /// * `command` – the program to run
    /// * `args` – arguments forwarded to the program
    /// * `handler` – closure called with each output chunk; must be `Send + Sync + 'static`
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
    ///     let events = parse_str(r#"
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
    ///     engine.execute(events).await?;
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

    /// Build an engine from pre-constructed parts.
    ///
    /// Prefer [`Engine::spawn`] or [`Engine::spawn_with_handler`] for normal use.
    fn from_parts<F>(pty: PtySession, output_rx: Receiver<Vec<u8>>, handler: F) -> Self
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let output_buffer = Arc::new(Mutex::new(String::new()));
        let buffer_clone = output_buffer.clone();
        let handler = Arc::new(handler);
        let handler_clone = handler.clone();

        // Background task: drain the PTY output channel, invoke the handler, and
        // maintain the rolling buffer used by Expect pattern matching.
        let output_task = tokio::task::spawn_blocking(move || {
            while let Ok(data) = output_rx.recv() {
                handler_clone(&data);

                if let Ok(mut buffer) = buffer_clone.lock() {
                    let text = String::from_utf8_lossy(&data);
                    buffer.push_str(&text);

                    // Keep the buffer bounded to avoid unbounded memory growth.
                    if buffer.len() > 10_000 {
                        buffer.drain(..5_000);
                    }
                }
            }
        });

        Engine {
            pty,
            output_buffer,
            _output_task: output_task,
            output_handler: handler,
        }
    }

    /// Execute a sequence of events in order.
    ///
    /// After the last event the engine waits briefly for any remaining PTY output
    /// to be flushed through the output handler before returning.
    pub async fn execute(&mut self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.execute_event(event).await?;
        }

        // Allow any buffered PTY output to drain.
        sleep(Duration::from_millis(300)).await;

        Ok(())
    }

    /// Execute a single event.
    async fn execute_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::SendToProgram(data) => {
                self.pty.write(&data)?;
                // Give the program a moment to process the input.
                sleep(Duration::from_millis(50)).await;
            }

            Event::ShowToUser(data) => {
                (self.output_handler)(&data);
            }

            Event::TypeText {
                text,
                min_delay,
                max_delay,
            } => {
                self.simulate_typing(&text, min_delay, max_delay).await?;
            }

            Event::Sleep(duration) => {
                sleep(duration).await;
            }

            Event::Expect { pattern, timeout } => {
                self.wait_for_pattern(&pattern, timeout).await?;
            }
        }

        Ok(())
    }

    /// Send `text` to the PTY one character at a time with random per-character
    /// delays. The PTY's own echo is the sole source of displayed output, so
    /// each character appears exactly once.
    async fn simulate_typing(
        &mut self,
        text: &str,
        min_delay: Duration,
        max_delay: Duration,
    ) -> Result<()> {
        let mut rng = rand::thread_rng();

        for ch in text.chars() {
            // Sending one character at a time; the PTY echoes it back through
            // the output channel, so we do not call output_handler here.
            self.pty.write(ch.to_string().as_bytes())?;

            let delay_ms = rng.gen_range(min_delay.as_millis()..=max_delay.as_millis());
            sleep(Duration::from_millis(delay_ms as u64)).await;
        }

        // Longer pause after the last character before submitting.
        sleep(Duration::from_millis(2 * max_delay.as_millis() as u64)).await;

        // Send the newline to submit the line to the program.
        self.pty.write(b"\n")?;

        // Small pause for the program to process the input.
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Block until `pattern` appears in the PTY output buffer, or until `timeout` elapses.
    async fn wait_for_pattern(&mut self, pattern: &str, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            {
                let mut buffer = self.output_buffer.lock().unwrap();
                if buffer.contains(pattern) {
                    if let Some(idx) = buffer.find(pattern) {
                        let end_idx = idx + pattern.len();
                        buffer.drain(..end_idx);
                    }
                    return Ok(());
                }
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for pattern: '{}'",
                    pattern
                ));
            }

            sleep(Duration::from_millis(10)).await;
        }
    }

    /// Wait for the child process to exit.
    pub fn wait_for_exit(&mut self) -> Result<()> {
        self.pty.wait()
    }
}
