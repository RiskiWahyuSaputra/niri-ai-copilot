//! IPC socket untuk integrasi shortcut Niri.
//!
//! Aplikasi membuka Unix socket di `$XDG_RUNTIME_DIR/niri-ai-copilot.sock`.
//! Perintah dikirim sebagai satu baris teks, mis.:
//!
//! ```text
//! toggle
//! start
//! pause
//! resume
//! stop
//! expand
//! collapse
//! alt-answer
//! status
//! quit
//! ```
//!
//! Niri `binds { Mod+Shift+C { spawn-sh "niri-ai-copilot toggle"; } }`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::config::paths;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcCommand {
    Toggle,
    Show,
    Hide,
    Start,
    Pause,
    Resume,
    Stop,
    Expand,
    Collapse,
    AltAnswer,
    Status,
    Quit,
}

impl IpcCommand {
    pub fn as_str(&self) -> &'static str {
        match self {
            IpcCommand::Toggle => "toggle",
            IpcCommand::Show => "show",
            IpcCommand::Hide => "hide",
            IpcCommand::Start => "start",
            IpcCommand::Pause => "pause",
            IpcCommand::Resume => "resume",
            IpcCommand::Stop => "stop",
            IpcCommand::Expand => "expand",
            IpcCommand::Collapse => "collapse",
            IpcCommand::AltAnswer => "alt-answer",
            IpcCommand::Status => "status",
            IpcCommand::Quit => "quit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "toggle" => Some(IpcCommand::Toggle),
            "show" => Some(IpcCommand::Show),
            "hide" => Some(IpcCommand::Hide),
            "start" => Some(IpcCommand::Start),
            "pause" => Some(IpcCommand::Pause),
            "resume" => Some(IpcCommand::Resume),
            "stop" => Some(IpcCommand::Stop),
            "expand" => Some(IpcCommand::Expand),
            "collapse" => Some(IpcCommand::Collapse),
            "alt-answer" | "alt" | "alternative" => Some(IpcCommand::AltAnswer),
            "status" => Some(IpcCommand::Status),
            "quit" | "exit" => Some(IpcCommand::Quit),
            _ => None,
        }
    }
}

/// Snapshot status yang dibagikan ke server IPC (diperbarui oleh app thread).
pub type SharedStatus = Arc<Mutex<String>>;

pub struct IpcServer {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    socket_path: PathBuf,
}

impl IpcServer {
    /// Jalankan server; `tx` menerima perintah untuk diproses di UI thread.
    pub fn start(tx: Sender<IpcCommand>, status: SharedStatus) -> Result<Self> {
        let socket_path = paths::socket_path();
        prepare_socket_path(&socket_path)?;

        let listener = UnixListener::bind(&socket_path).map_err(|e| {
            AppError::Other(format!(
                "tidak dapat membuka socket IPC {}: {e}",
                socket_path.display()
            ))
        })?;
        restrict_socket_permissions(&socket_path)?;
        listener
            .set_nonblocking(true)
            .map_err(AppError::Io)?;

        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let path_for_thread = socket_path.clone();

        let handle = std::thread::Builder::new()
            .name("ipc-server".into())
            .spawn(move || {
                while !stop_thread.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = handle_client(stream, &tx, &status);
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        Err(_) => break,
                    }
                }
                let _ = std::fs::remove_file(&path_for_thread);
            })
            .map_err(AppError::Io)?;

        Ok(Self {
            stop,
            handle: Some(handle),
            socket_path,
        })
    }

    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Bangunkan accept() yang nonblocking dengan connect dummy.
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(&self.socket_path);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn handle_client(
    stream: UnixStream,
    tx: &Sender<IpcCommand>,
    status: &SharedStatus,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut writer = &stream;
    match IpcCommand::parse(&line) {
        Some(IpcCommand::Status) => {
            let snapshot = status
                .lock()
                .map(|s| s.clone())
                .unwrap_or_else(|_| "status tidak tersedia".into());
            writeln!(writer, "{snapshot}")?;
        }
        Some(cmd) => {
            let _ = tx.send(cmd);
            writeln!(writer, "ok: {}", cmd.as_str())?;
        }
        None => {
            writeln!(writer, "error: perintah tidak dikenal '{}'", line.trim())?;
        }
    }
    Ok(())
}

fn prepare_socket_path(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => {
                return Err(AppError::Other(
                    "instans niri-ai-copilot lain sedang berjalan".into(),
                ));
            }
            Err(_) => {
                // Socket basi.
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_socket_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_socket_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// Kirim perintah ke instans yang berjalan (dipakai CLI `niri-ai-copilot <cmd>`).
pub fn send_command(cmd: &str) -> Result<String> {
    let path = paths::socket_path();
    let mut stream = UnixStream::connect(&path).map_err(|_| {
        AppError::Unavailable(format!(
            "aplikasi sepertinya tidak berjalan (socket {} tidak dapat dihubungi)",
            path.display()
        ))
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(format!("{cmd}\n").as_bytes())?;
    let mut reader = BufReader::new(&stream);
    let mut reply = String::new();
    reader.read_line(&mut reply)?;
    Ok(reply.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        assert_eq!(IpcCommand::parse(" Toggle \n"), Some(IpcCommand::Toggle));
        assert_eq!(IpcCommand::parse("alt-answer"), Some(IpcCommand::AltAnswer));
        assert_eq!(IpcCommand::parse("nonsense"), None);
    }

    #[test]
    fn roundtrip_over_socket() {
        let dir = std::env::temp_dir().join(format!("niri-ai-copilot-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Override path socket lewat env XDG_RUNTIME_DIR (paths::socket_path).
        std::env::set_var("XDG_RUNTIME_DIR", &dir);

        let (tx, rx) = std::sync::mpsc::channel();
        let status: SharedStatus = Arc::new(Mutex::new("idle, tidak ada sesi".into()));
        let server = IpcServer::start(tx, status).expect("server start");

        let reply = send_command("toggle").expect("send");
        assert!(reply.contains("ok"));
        let received = rx.recv_timeout(Duration::from_secs(2)).expect("perintah diterima");
        assert_eq!(received, IpcCommand::Toggle);

        let status_reply = send_command("status").expect("status");
        assert!(status_reply.contains("idle"));

        server.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
