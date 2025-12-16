use crate::event::Event;
use crate::pty::PtySession;
use anyhow::Result;
use rand::Rng;
use std::io::{self, Write};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// The demo engine that executes events and manages presentation
pub struct Engine {
    pty: PtySession,
    output_buffer: Arc<Mutex<String>>,
    _output_task: tokio::task::JoinHandle<()>,
}

impl Engine {
    /// Create a new engine with a PTY session and output receiver
    pub fn new(pty: PtySession, output_rx: Receiver<Vec<u8>>) -> Self {
        let output_buffer = Arc::new(Mutex::new(String::new()));
        let buffer_clone = output_buffer.clone();

        // Spawn background task to continuously drain output and display it
        let output_task = tokio::task::spawn_blocking(move || {
            while let Ok(data) = output_rx.recv() {
                // Immediately write to stdout
                if let Err(e) = io::stdout().write_all(&data) {
                    eprintln!("Error writing to stdout: {}", e);
                    break;
                }
                if let Err(e) = io::stdout().flush() {
                    eprintln!("Error flushing stdout: {}", e);
                    break;
                }

                // Add to buffer for expect pattern matching
                if let Ok(mut buffer) = buffer_clone.lock() {
                    let text = String::from_utf8_lossy(&data);
                    buffer.push_str(&text);

                    // Prevent buffer from growing too large
                    if buffer.len() > 10000 {
                        buffer.drain(..5000);
                    }
                }
            }
        });

        Engine {
            pty,
            output_buffer,
            _output_task: output_task,
        }
    }

    /// Execute a sequence of events
    pub async fn execute(&mut self, events: Vec<Event>) -> Result<()> {
        for event in events {
            self.execute_event(event).await?;
        }

        // After all events, wait a bit for final output to be displayed
        sleep(Duration::from_millis(300)).await;

        Ok(())
    }

    /// Execute a single event
    async fn execute_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::SendToProgram(data) => {
                self.pty.write(&data)?;
                // Give the program a moment to process
                sleep(Duration::from_millis(50)).await;
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
        }

        // After typing is done, send the complete text to the program
        let mut input = text.to_string();
        input.push('\n');
        self.pty.write(input.as_bytes())?;

        // Show the newline to the user
        println!();

        // Small delay for program to process
        sleep(Duration::from_millis(100)).await;

        Ok(())
    }

    /// Wait for the PTY process to exit
    #[allow(dead_code)]
    pub fn wait_for_exit(&mut self) -> Result<()> {
        self.pty.wait()
    }

    /// Wait for a specific pattern to appear in the output
    async fn wait_for_pattern(&mut self, pattern: &str, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            // Check the buffer (background task is continuously filling it)
            {
                let mut buffer = self.output_buffer.lock().unwrap();
                if buffer.contains(pattern) {
                    // Clear the buffer up to and including the pattern
                    if let Some(idx) = buffer.find(pattern) {
                        let end_idx = idx + pattern.len();
                        buffer.drain(..end_idx);
                    }
                    return Ok(());
                }
            } // Release lock

            // Check if we've exceeded the timeout
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for pattern: '{}'",
                    pattern
                ));
            }

            // Small sleep before checking again
            sleep(Duration::from_millis(10)).await;
        }
    }
}
