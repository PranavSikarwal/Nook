#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn repo_policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../policy")
        .canonicalize()
        .unwrap()
}

fn fake_worker_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../nook-core/tests/fake_worker.py")
        .canonicalize()
        .unwrap()
}

fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nook-sig-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

struct TestDaemon {
    child: Child,
    app_dir: PathBuf,
    socket_path: PathBuf,
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.app_dir);
    }
}

fn spawn_daemon(label: &str, pipe_stderr: bool, rust_log: Option<&str>) -> TestDaemon {
    let app_dir = scratch_dir(label);
    let socket_path = app_dir.join("daemon.sock");
    let config_path = app_dir.join("config.toml");
    let fake_worker = fake_worker_path();
    let worker_cmd = format!("python3 {}", fake_worker.display());

    let config_content = format!(
        "model = \"test-model\"\nbase_url = \"https://example.com/v1\"\nworker_command = \"{worker_cmd}\"\n"
    );
    std::fs::write(&config_path, config_content).unwrap();

    let db_url = std::env::var("NOOK_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://azureuser:nook@127.0.0.1:5432/nook".to_string());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nookd"));
    cmd.env("NOOK_APP_DIR", &app_dir)
        .env("NOOK_POLICY_DIR", repo_policy_dir())
        .env("NOOK_WORKER_COMMAND", &worker_cmd)
        .env("NOOK_DATABASE_URL", &db_url)
        .stdin(Stdio::null())
        .stdout(Stdio::null());

    if let Some(log) = rust_log {
        cmd.env("RUST_LOG", log);
    }

    if pipe_stderr {
        cmd.stderr(Stdio::piped());
    } else {
        cmd.stderr(Stdio::null());
    }

    let child = cmd.spawn().expect("nookd should start");

    let mut daemon = TestDaemon {
        child,
        app_dir,
        socket_path,
    };

    let start = Instant::now();
    let startup_timeout = Duration::from_secs(15);
    loop {
        if let Ok(_stream) = UnixStream::connect(&daemon.socket_path) {
            break;
        }
        if let Ok(Some(status)) = daemon.child.try_wait() {
            panic!("nookd exited unexpectedly during startup with status: {status:?}");
        }
        if start.elapsed() > startup_timeout {
            panic!("nookd did not create socket or accept connections within {startup_timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    daemon
}

fn send_signal(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .expect("kill command failed to execute");
    assert!(status.success(), "kill command returned failure status");
}

fn assert_exits_within_5s_and_socket_removed(mut daemon: TestDaemon) {
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let mut exited = false;

    while start.elapsed() < timeout {
        if let Ok(Some(status)) = daemon.child.try_wait() {
            assert!(
                status.success(),
                "nookd exited with error code: {:?}",
                status.code()
            );
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        exited,
        "nookd did not exit within 5 seconds of signal (hung)"
    );

    assert!(
        !daemon.socket_path.exists(),
        "socket file {} was not removed after shutdown",
        daemon.socket_path.display()
    );
}

#[test]
fn the_daemon_exits_on_sigterm_with_no_client() {
    let daemon = spawn_daemon("no-client", false, None);
    send_signal(daemon.child.id(), "-TERM");
    assert_exits_within_5s_and_socket_removed(daemon);
}

#[test]
fn the_daemon_exits_on_sigterm_with_an_idle_client_connected() {
    let daemon = spawn_daemon("idle-client", false, None);
    let _client = UnixStream::connect(&daemon.socket_path).expect("client should connect");
    send_signal(daemon.child.id(), "-TERM");
    assert_exits_within_5s_and_socket_removed(daemon);
}

#[test]
fn the_daemon_exits_on_sigterm_when_stderr_has_no_reader() {
    let mut daemon = spawn_daemon("no-reader", true, None);
    drop(daemon.child.stderr.take());
    send_signal(daemon.child.id(), "-TERM");
    assert_exits_within_5s_and_socket_removed(daemon);
}

#[test]
fn the_daemon_exits_on_sigterm_when_the_stderr_pipe_is_full() {
    let daemon = spawn_daemon("pipe-full", true, Some("trace"));
    for _ in 0..1500 {
        if let Ok(mut stream) = UnixStream::connect(&daemon.socket_path) {
            use std::io::Write;
            let _ = stream.set_write_timeout(Some(Duration::from_millis(50)));
            let _ = stream.write_all(b"{\"invalid\": true}\n");
        }
    }
    send_signal(daemon.child.id(), "-TERM");
    assert_exits_within_5s_and_socket_removed(daemon);
}

#[test]
fn the_daemon_exits_on_sigint() {
    let daemon = spawn_daemon("sigint", false, None);
    send_signal(daemon.child.id(), "-INT");
    assert_exits_within_5s_and_socket_removed(daemon);
}
