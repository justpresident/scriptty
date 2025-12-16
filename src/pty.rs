use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, Child, MasterPty, PtySize};
use std::io::{Read, Write};

/// Manages a program running inside a PTY
pub struct PtySession {
    #[allow(dead_code)]
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Spawn a new program in a PTY, returning the session and reader separately
    pub fn spawn(command: &str, args: &[String]) -> Result<(Self, Box<dyn Read + Send>)> {
        let pty_system = portable_pty::native_pty_system();

        // Create PTY with reasonable defaults
        let pty_size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(pty_size)
            .context("Failed to open PTY")?;

        // Build the command
        let mut cmd = CommandBuilder::new(command);
        for arg in args {
            cmd.arg(arg);
        }

        // Spawn the child process
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        // Get reader and writer from the master PTY
        let writer = pair
            .master
            .take_writer()
            .context("Failed to get PTY writer")?;

        let reader = pair
            .master
            .try_clone_reader()
            .context("Failed to get PTY reader")?;

        let session = PtySession {
            master: pair.master,
            child,
            writer,
        };

        Ok((session, reader))
    }

    /// Write data to the program's stdin
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Check if the child process is still running
    #[allow(dead_code)]
    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Wait for the child process to exit
    pub fn wait(&mut self) -> Result<()> {
        self.child.wait()?;
        Ok(())
    }

    /// Resize the PTY
    #[allow(dead_code)]
    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        self.master.resize(size)?;
        Ok(())
    }
}
