use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::info;

static SIGNAL_COUNT: AtomicU32 = AtomicU32::new(0);
static SENDER: OnceLock<SyncSender<Vec<u8>>> = OnceLock::new();

fn get_or_init_sender() -> &'static SyncSender<Vec<u8>> {
    SENDER.get_or_init(|| {
        let (tx, rx) = sync_channel::<Vec<u8>>(1024);
        std::thread::Builder::new()
            .name("safe-stderr-writer".to_string())
            .spawn(move || {
                let mut stderr = std::io::stderr();
                while let Ok(msg) = rx.recv() {
                    let _ = stderr.write_all(&msg);
                    let _ = stderr.flush();
                }
            })
            .expect("failed to spawn safe-stderr-writer thread");
        tx
    })
}

#[derive(Clone, Copy, Default)]
pub struct SafeStderr;

impl Write for SafeStderr {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let tx = get_or_init_sender();
        let _ = tx.try_send(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn spawn_deadline_thread(socket_path: PathBuf, #[cfg(unix)] socket_identity: (u64, u64)) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        #[cfg(unix)]
        nook_core::server::remove_socket_if_owned(&socket_path, socket_identity);
        std::process::exit(1);
    });
}

pub fn handle_signal(
    signal_name: &'static str,
    shutdown_tx: &broadcast::Sender<()>,
    socket_path: &Path,
    #[cfg(unix)] socket_identity: (u64, u64),
) {
    if SIGNAL_COUNT.fetch_add(1, Ordering::SeqCst) > 0 {
        #[cfg(unix)]
        nook_core::server::remove_socket_if_owned(socket_path, socket_identity);
        std::process::exit(1);
    }

    spawn_deadline_thread(
        socket_path.to_path_buf(),
        #[cfg(unix)]
        socket_identity,
    );

    info!("Received {signal_name}, shutting down daemon");
    let _ = shutdown_tx.send(());
}

/// Warning: `exit` calls `std::process::exit`, which ends the process immediately without running destructors.
pub fn exit(code: i32) -> ! {
    std::process::exit(code)
}
