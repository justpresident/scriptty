//! # Scriptty
//!
//! A PTY scripting engine for automating interactive terminal sessions.
//!
//! Scriptty lets you write simple scripts that drive any interactive terminal
//! program with precise control over input timing and output presentation.
//! It is useful for testing CLI tools, building reproducible terminal
//! walkthroughs, and constructing terminal automation pipelines.
//!
//! ## Quick start
//!
//! ```no_run
//! use scriptty::{Engine, parse_str};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let script = r#"
//! expect "$ "
//! type "echo hello"
//! key Enter
//! expect "hello"
//! send "exit"
//! key Enter
//! "#;
//!
//!     let commands = parse_str(script)?;
//!     let mut engine = Engine::spawn("bash", &[] as &[&str])?;
//!     engine.execute(commands).await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Parsing scripts
//!
//! Use [`parse_str`] to parse a script from an in-memory string, or
//! [`parse_file`] to read one from a file path. Both return a
//! `Vec<Box<dyn `[`ScripttyCommand`]`>>` ready to pass to [`Engine::execute`].
//!
//! ## Script syntax
//!
//! | Command | Description |
//! |---------|-------------|
//! | `type "text"` | Simulate typing with per-character delays |
//! | `send "text"` | Send text to the program immediately (no typing simulation) |
//! | `key Enter` | Send a key press (supports `Ctrl+`, `Alt+`, `Shift+` modifiers) |
//! | `show "text"` | Write text directly to the output handler |
//! | `expect "pattern"` | Wait until `pattern` appears in the program output |
//! | `expect "pattern" 5s` | Wait up to 5 seconds for the pattern |
//! | `wait 500ms` | Pause for a duration (`ms` or `s` units, floats allowed) |
//! | `# comment` | Full-line or inline comment |
//!
//! ## Custom output handling
//!
//! By default [`Engine::spawn`] writes all output to stdout. Use
//! [`Engine::spawn_with_handler`] to redirect output to any sink:
//!
//! ```no_run
//! use scriptty::{Engine, parse_str};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let commands = parse_str(r#"type "hello""#)?;
//!
//!     let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
//!     let sink = captured.clone();
//!
//!     let mut engine = Engine::spawn_with_handler("bash", &[] as &[&str], move |data| {
//!         sink.lock().unwrap().extend_from_slice(data);
//!     })?;
//!
//!     engine.execute(commands).await?;
//!     println!("{}", String::from_utf8_lossy(&captured.lock().unwrap()));
//!     Ok(())
//! }
//! ```
//!
//! ## Implementing a custom command
//!
//! Implement [`ScripttyCommand`] to add new commands to the engine:
//!
//! ```no_run
//! use scriptty::command::{Context, ScripttyCommand};
//! use async_trait::async_trait;
//! use anyhow::Result;
//!
//! pub struct Beep;
//!
//! impl Beep {
//!     pub const NAME: &'static str = "beep";
//! }
//!
//! #[async_trait(?Send)]
//! impl ScripttyCommand for Beep {
//!     fn name(&self) -> &'static str { Self::NAME }
//!
//!     fn parse(_args: &str) -> Result<Self> {
//!         Ok(Self)
//!     }
//!
//!     async fn execute(&self, ctx: &mut Context) -> Result<()> {
//!         ctx.emit(b"\x07"); // BEL character
//!         Ok(())
//!     }
//! }
//! ```

pub mod command;
pub mod commands;
pub mod engine;
pub mod parser;
pub(crate) mod pty;
pub(crate) mod pty_reader;

pub use command::{Context, ScripttyCommand};
pub use commands::{Expect, KeyPress, SendInput, Show, TypeText, Wait};
pub use engine::Engine;
pub use parser::{parse_file, parse_str};
