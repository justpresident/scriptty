use crate::event::Event;
use crate::pty::PtySession;
use anyhow::Result;
use rand::Rng;
use std::io::{self, Write};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tokio::time::sleep;

/// The demo engine that executes events and manages presentation
pub struct Engine {
    pty: PtySession,
    output_rx: Receiver<Vec<u8>>,
    output_buffer: String,
}

impl Engine {
    /// Create a new engine with a PTY session and output receiver
    pub fn new(pty: PtySession, output_rx: Receiver<Vec<u8>>) -> Self {
        Engine {
            pty,
            output_rx,
            output_buffer: String::new(),
        }
    }

    /// Execute a sequence of events
    pub async fn execute(&mut self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.execute_event(event).await?;
        }

        // After all events, wait a bit for final output
        sleep(Duration::from_millis(500)).await;
        self.flush_pty_output().await?;

        Ok(())
    }

    /// Execute a single event
    async fn execute_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::SendToProgram(data) => {
                self.pty.write(&data)?;
                // Give the program a moment to process
                sleep(Duration::from_millis(10)).await;
                self.flush_pty_output().await?;
            }

            Event::ShowToUser(data) => {
                io::stdout().write_all(&data)?;
                io::stdout().flush()?;
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
                // Check for any output while sleeping
                self.flush_pty_output().await?;
            }

            Event::Expect { pattern, timeout } => {
                self.wait_for_pattern(&pattern, timeout).await?;
            }
        }

        Ok(())
    }

    /// Simulate realistic typing with character-by-character delays
    async fn simulate_typing(
        &mut self,
        text: &str,
        min_delay: Duration,
        max_delay: Duration,
    ) -> Result<()> {
        let mut rng = rand::thread_rng();

        for ch in text.chars() {
            // Display the character to the user
            print!("{}", ch);
            io::stdout().flush()?;

            // Random delay between min and max
            let delay_ms = rng.gen_range(min_delay.as_millis()..=max_delay.as_millis());
            sleep(Duration::from_millis(delay_ms as u64)).await;

            // Check for program output while typing
            self.try_flush_pty_output()?;
        }

        // After typing is done, send the complete text to the program
        let mut input = text.to_string();
        input.push('\n');
        self.pty.write(input.as_bytes())?;

        // Show the newline to the user
        println!();

        // Small delay for program to process
        sleep(Duration::from_millis(50)).await;

        // Get the program's response
        self.flush_pty_output().await?;

        Ok(())
    }

    /// Read and display all available output from the PTY
    async fn flush_pty_output(&mut self) -> Result<()> {
        // Give the program time to produce output
        sleep(Duration::from_millis(100)).await;

        // Drain the channel with timeout
        loop {
            match self.output_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(data) => {
                    io::stdout().write_all(&data)?;
                    io::stdout().flush()?;

                    // Add to buffer for expect commands
                    let text = String::from_utf8_lossy(&data);
                    self.output_buffer.push_str(&text);

                    // Prevent buffer from growing too large
                    if self.output_buffer.len() > 10000 {
                        self.output_buffer.drain(..5000);
                    }
                }
                Err(_) => break, // Timeout or disconnected
            }
        }

        Ok(())
    }

    /// Try to read output from PTY without blocking
    fn try_flush_pty_output(&mut self) -> Result<()> {
        while let Ok(data) = self.output_rx.try_recv() {
            io::stdout().write_all(&data)?;
            io::stdout().flush()?;

            // Add to buffer for expect commands
            let text = String::from_utf8_lossy(&data);
            self.output_buffer.push_str(&text);

            // Prevent buffer from growing too large
            if self.output_buffer.len() > 10000 {
                self.output_buffer.drain(..5000);
            }
        }

        Ok(())
    }

    /// Wait for the PTY process to exit
    pub fn wait_for_exit(&mut self) -> Result<()> {
        self.pty.wait()
    }

    /// Wait for a specific pattern to appear in the output
    async fn wait_for_pattern(&mut self, pattern: &str, timeout: Duration) -> Result<()> {
        // First check if the pattern is already in the buffer
        if self.output_buffer.contains(pattern) {
            // Clear the buffer up to and including the pattern
            if let Some(idx) = self.output_buffer.find(pattern) {
                let end_idx = idx + pattern.len();
                self.output_buffer.drain(..end_idx);
            }
            return Ok(());
        }

        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            // Check if we've exceeded the timeout
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for pattern: '{}'",
                    pattern
                ));
            }

            // Calculate remaining timeout
            let remaining = deadline - tokio::time::Instant::now();
            let check_timeout = remaining.min(Duration::from_millis(50));

            // Try to read more output
            match self.output_rx.recv_timeout(check_timeout.into()) {
                Ok(data) => {
                    // Display the data
                    io::stdout().write_all(&data)?;
                    io::stdout().flush()?;

                    // Add to buffer (convert from bytes to string, ignoring invalid UTF-8)
                    let text = String::from_utf8_lossy(&data);
                    self.output_buffer.push_str(&text);

                    // Check if pattern is found
                    if self.output_buffer.contains(pattern) {
                        // Clear the buffer up to and including the pattern
                        if let Some(idx) = self.output_buffer.find(pattern) {
                            let end_idx = idx + pattern.len();
                            self.output_buffer.drain(..end_idx);
                        }
                        return Ok(());
                    }

                    // Prevent buffer from growing too large
                    if self.output_buffer.len() > 10000 {
                        self.output_buffer.drain(..5000);
                    }
                }
                Err(_) => {
                    // Timeout or channel closed, continue checking
                    continue;
                }
            }
        }
    }
}
