use anyhow::{Context, Result};
use clap::Parser;
use scriptty::{Engine, parse_file};
use std::io::Write;

#[derive(Parser, Debug)]
#[command(
    name = "scriptty",
    about = "Run a scriptty script against an interactive terminal program",
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

    let events = parse_file(&args.script)
        .with_context(|| format!("Failed to parse script file: {}", args.script))?;

    let mut engine = Engine::spawn(&args.command, &args.args).context("Failed to spawn engine")?;

    // Give the program time to start up before executing events.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    clear_screen()?;

    engine
        .execute(events)
        .await
        .context("Failed to execute script")?;

    Ok(())
}

fn clear_screen() -> Result<()> {
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush()?;
    Ok(())
}
