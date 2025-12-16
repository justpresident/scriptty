use std::time::Duration;

/// Core event types that drive the demo engine
#[derive(Debug, Clone)]
pub enum Event {
    /// Send input directly to the program (instant, not visible)
    SendToProgram(Vec<u8>),

    /// Display output to the viewer (what they see)
    ShowToUser(Vec<u8>),

    /// Simulate typing text with realistic timing
    TypeText {
        text: String,
        min_delay: Duration,
        max_delay: Duration,
    },

    /// Pause execution
    Sleep(Duration),
}

impl Event {
    /// Create a SendToProgram event from a string
    pub fn send(text: impl Into<String>) -> Self {
        let mut bytes = text.into().into_bytes();
        bytes.push(b'\n');
        Event::SendToProgram(bytes)
    }

    /// Create a ShowToUser event from a string
    pub fn show(text: impl Into<String>) -> Self {
        Event::ShowToUser(text.into().into_bytes())
    }

    /// Create a TypeText event with default timing (50-150ms per char)
    pub fn type_text(text: impl Into<String>) -> Self {
        Event::TypeText {
            text: text.into(),
            min_delay: Duration::from_millis(50),
            max_delay: Duration::from_millis(150),
        }
    }

    /// Create a TypeText event with custom timing
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

    /// Create a Sleep event
    pub fn sleep(duration: Duration) -> Self {
        Event::Sleep(duration)
    }
}
