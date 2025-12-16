mod engine;
mod event;
mod parser;
mod pty;
mod pty_reader;

use anyhow::{Context, Result};
use clap::Parser;
use engine::Engine;
use pty::PtySession;
use std::fs;
use std::io::Write;

#[derive(Parser, Debug)]
#[command(
    name = "scriptty",
    about = "Terminal proxy demo engine - create realistic, reproducible terminal demos",
    version
)]
struct Args {
    /// Path to the script file
    #[arg(short, long)]
    script: String,

    /// Command to run in the PTY
    #[arg(short, long)]
    command: String,

    /// Arguments to pass to the command
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Read and parse the script
    let script_content = fs::read_to_string(&args.script)
        .with_context(|| format!("Failed to read script file: {}", args.script))?;

    let events = parser::parse_script(&script_content)
        .context("Failed to parse script")?;

    // Spawn the PTY with the target program
    let (pty, reader) = PtySession::spawn(&args.command, &args.args)
        .context("Failed to spawn PTY")?;

    // Spawn background reader thread
    let output_rx = pty_reader::spawn_reader(reader);

    // Give the program time to start up and drain initial output
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Drain initial output (like bash prompt)
    while let Ok(data) = output_rx.recv_timeout(std::time::Duration::from_millis(50)) {
        std::io::stdout().write_all(&data)?;
        std::io::stdout().flush()?;
    }

    // Create and run the engine
    let mut engine = Engine::new(pty, output_rx);

    engine
        .execute(events)
        .await
        .context("Failed to execute events")?;

    Ok(())
}
