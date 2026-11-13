use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::info;

static SIGNAL_COUNT: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
pub fn set_stderr_nonblocking() {
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: std::os::raw::c_int = 0o4000;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: std::os::raw::c_int = 0x0004;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    const O_NONBLOCK: std::os::raw::c_int = 0o4000;

    unsafe {
        extern "C" {
            fn fcntl(fd: std::os::raw::c_int, cmd: std::os::raw::c_int, ...)
                -> std::os::raw::c_int;
        }
        let flags = fcntl(2, 3);
        if flags >= 0 {
            let _ = fcntl(2, 4, flags | O_NONBLOCK);
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct SafeStderr;

impl Write for SafeStderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match std::io::stderr().write(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(buf.len()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(buf.len()),
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        Ok(())
    }
}

pub fn spawn_deadline_thread(socket_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
        std::process::exit(0);
    });
}

pub fn handle_signal(
    signal_name: &'static str,
    shutdown_tx: &broadcast::Sender<()>,
    socket_path: &Path,
) {
    if SIGNAL_COUNT.fetch_add(1, Ordering::SeqCst) > 0 {
        std::process::exit(0);
    }

    spawn_deadline_thread(socket_path.to_path_buf());

    let _ = shutdown_tx.send(());

    tokio::task::spawn_blocking(move || {
        info!("Received {signal_name}, shutting down daemon");
    });
}

pub fn exit(code: i32) -> ! {
    std::process::exit(code)
}
