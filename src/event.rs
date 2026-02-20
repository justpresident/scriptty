use std::time::Duration;

/// An action that the [`crate::Engine`] can perform against a running PTY process.
///
/// Events are produced by the parser and consumed sequentially by
/// [`crate::Engine::execute`]. You can also build them programmatically using
/// the constructor methods below.
#[derive(Debug, Clone)]
pub enum Event {
    /// Send raw bytes to the program's stdin immediately, without any visible output.
    ///
    /// A newline (`\n`) is appended automatically by [`Event::send`].
    SendToProgram(Vec<u8>),

    /// Write bytes directly to the output handler without sending them to the program.
    ///
    /// Useful for inserting annotations or commentary into the output stream.
    ShowToUser(Vec<u8>),

    /// Simulate a human typing `text` character by character, then send the line.
    ///
    /// A random per-character delay between `min_delay` and `max_delay` is used.
    /// After the last character the engine pauses for `2 × max_delay` before
    /// sending the complete text (plus newline) to the program.
    TypeText {
        text: String,
        min_delay: Duration,
        max_delay: Duration,
    },

    /// Pause execution for the given duration.
    Sleep(Duration),

    /// Block until `pattern` appears in the program's output, or until `timeout` elapses.
    ///
    /// When the pattern is found the output buffer is consumed up to and including it,
    /// so a subsequent `Expect` will not match the same occurrence again.
    Expect { pattern: String, timeout: Duration },
}

impl Event {
    /// Create a [`Event::SendToProgram`] event from a string.
    ///
    /// A newline is appended so the program receives a complete line.
    pub fn send(text: impl Into<String>) -> Self {
        let mut bytes = text.into().into_bytes();
        bytes.push(b'\n');
        Event::SendToProgram(bytes)
    }

    /// Create a [`Event::ShowToUser`] event from a string.
    ///
    /// A newline is appended so each `show` value appears on its own line.
    pub fn show(text: impl Into<String>) -> Self {
        let mut text = text.into();
        text.push('\n');
        Event::ShowToUser(text.into_bytes())
    }

    /// Create a [`Event::TypeText`] event with default timing (50–150 ms per character).
    pub fn type_text(text: impl Into<String>) -> Self {
        Self::type_text_with_timing(text, Duration::from_millis(50), Duration::from_millis(150))
    }

    /// Create a [`Event::TypeText`] event with custom per-character timing.
    pub fn type_text_with_timing(
        text: impl Into<String>,
        min_delay: Duration,
        max_delay: Duration,
    ) -> Self {
        Event::TypeText {
            text: text.into(),
            min_delay,
            max_delay,
        }
    }

    /// Create a [`Event::Sleep`] event.
    pub fn sleep(duration: Duration) -> Self {
        Event::Sleep(duration)
    }

    /// Create an [`Event::Expect`] event with the default 5-second timeout.
    pub fn expect(pattern: impl Into<String>) -> Self {
        Event::Expect {
            pattern: pattern.into(),
            timeout: Duration::from_secs(5),
        }
    }

    /// Create an [`Event::Expect`] event with a custom timeout.
    pub fn expect_with_timeout(pattern: impl Into<String>, timeout: Duration) -> Self {
        Event::Expect {
            pattern: pattern.into(),
            timeout,
        }
    }
}
