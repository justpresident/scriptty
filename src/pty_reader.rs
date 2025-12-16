use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Spawns a background thread to read from a PTY
pub fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> Receiver<Vec<u8>> {
    let (tx, rx) = channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx.send(buffer[..n].to_vec()).is_err() {
                        break; // Receiver dropped
                    }
                }
                Err(_) => break,
            }
        }
    });

    rx
}
