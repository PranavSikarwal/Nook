#![cfg(unix)]

use nook_core::sandbox::{Sandbox, SandboxOutput, SandboxSpec};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::net::TcpListener;

const SYSTEM_PATH: &str = "/usr/bin:/bin";

async fn sandbox() -> Option<Sandbox> {
    match Sandbox::detect().await {
        Ok(sandbox) => Some(sandbox),
        Err(error) => {
            assert!(
                std::env::var_os("NOOK_REQUIRE_SANDBOX").is_none(),
                "The sandbox is required but unavailable: {error}"
            );
            eprintln!("Skipping sandbox test: {error}");
            None
        }
    }
}

struct Area {
    _dir: TempDir,
    allowed: PathBuf,
    read_only: PathBuf,
    other: PathBuf,
}

fn area() -> Area {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().canonicalize().unwrap();
    let allowed = base.join("allowed");
    let read_only = base.join("read_only");
    let other = base.join("other");
    for path in [&allowed, &read_only, &other] {
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(allowed.join("ok.txt"), "inside").unwrap();
    std::fs::write(read_only.join("ro.txt"), "readable").unwrap();
    std::fs::write(other.join("secret.txt"), "secret").unwrap();
    Area {
        _dir: dir,
        allowed,
        read_only,
        other,
    }
}

fn spec(area: &Area, argv: &[&str]) -> SandboxSpec {
    SandboxSpec {
        argv: argv.iter().map(|part| part.to_string()).collect(),
        cwd: area.allowed.clone(),
        read_roots: vec![area.read_only.clone()],
        write_roots: vec![area.allowed.clone()],
        network: false,
        env: BTreeMap::from([("PATH".to_string(), SYSTEM_PATH.to_string())]),
        timeout: Duration::from_secs(20),
        max_output_bytes: 65536,
    }
}

async fn run(sandbox: &Sandbox, spec: SandboxSpec) -> SandboxOutput {
    sandbox.run(spec).await.unwrap()
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

#[tokio::test]
async fn reads_a_file_inside_a_write_root() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let file = path_text(&area.allowed.join("ok.txt"));

    let output = run(&sandbox, spec(&area, &["/bin/cat", &file])).await;

    assert_eq!(output.exit_code, Some(0), "{}", output.stderr);
    assert_eq!(output.stdout, "inside");
}

#[tokio::test]
async fn reads_a_file_inside_a_read_root() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let file = path_text(&area.read_only.join("ro.txt"));

    let output = run(&sandbox, spec(&area, &["/bin/cat", &file])).await;

    assert_eq!(output.exit_code, Some(0), "{}", output.stderr);
    assert_eq!(output.stdout, "readable");
}

#[tokio::test]
async fn cannot_read_a_file_outside_the_roots() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let file = path_text(&area.other.join("secret.txt"));

    let output = run(&sandbox, spec(&area, &["/bin/cat", &file])).await;

    assert_ne!(output.exit_code, Some(0));
    assert!(!output.stdout.contains("secret"));
}

#[tokio::test]
async fn cannot_list_the_home_directory() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());

    let output = run(&sandbox, spec(&area, &["/bin/ls", &home])).await;

    assert!(
        output.exit_code != Some(0) || output.stdout.trim().is_empty(),
        "stdout: {}",
        output.stdout
    );
}

#[tokio::test]
async fn writes_inside_a_write_root() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let target = path_text(&area.allowed.join("new.txt"));

    let output = run(
        &sandbox,
        spec(
            &area,
            &["/bin/sh", "-c", &format!("echo written > '{target}'")],
        ),
    )
    .await;

    assert_eq!(output.exit_code, Some(0), "{}", output.stderr);
    assert_eq!(
        std::fs::read_to_string(area.allowed.join("new.txt")).unwrap(),
        "written\n"
    );
}

#[tokio::test]
async fn cannot_write_to_a_read_only_root() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let target = path_text(&area.read_only.join("new.txt"));

    let output = run(
        &sandbox,
        spec(&area, &["/bin/sh", "-c", &format!("echo x > '{target}'")]),
    )
    .await;

    assert_ne!(output.exit_code, Some(0));
    assert!(!area.read_only.join("new.txt").exists());
}

#[tokio::test]
async fn cannot_write_outside_the_roots() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let target = path_text(&area.other.join("new.txt"));

    let output = run(
        &sandbox,
        spec(&area, &["/bin/sh", "-c", &format!("echo x > '{target}'")]),
    )
    .await;

    assert_ne!(output.exit_code, Some(0));
    assert!(!area.other.join("new.txt").exists());
}

#[tokio::test]
async fn cannot_reach_a_local_server_without_network() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let script = format!("exec 3<>/dev/tcp/127.0.0.1/{port}");

    let output = run(&sandbox, spec(&area, &["/bin/bash", "-c", &script])).await;

    assert_ne!(output.exit_code, Some(0), "stderr: {}", output.stderr);
}

#[tokio::test]
async fn reaches_a_local_server_when_network_is_on() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let script = format!("exec 3<>/dev/tcp/127.0.0.1/{port}");
    let mut with_network = spec(&area, &["/bin/bash", "-c", &script]);
    with_network.network = true;

    let output = run(&sandbox, with_network).await;

    assert_eq!(output.exit_code, Some(0), "stderr: {}", output.stderr);
}

#[tokio::test]
async fn passes_only_the_given_environment() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    std::env::set_var("NOOK_TEST_SECRET", "leak");
    let mut env_spec = spec(&area, &["/usr/bin/env"]);
    env_spec.env.insert("KEPT".to_string(), "yes".to_string());

    let output = run(&sandbox, env_spec).await;

    let lines: Vec<&str> = output.stdout.lines().collect();
    assert!(lines.contains(&"KEPT=yes"), "{lines:?}");
    assert!(lines.contains(&"PATH=/usr/bin:/bin"), "{lines:?}");
    assert!(!output.stdout.contains("NOOK_TEST_SECRET"), "{lines:?}");
}

#[tokio::test]
async fn runs_in_the_given_working_directory() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();

    let output = run(&sandbox, spec(&area, &["/bin/pwd"])).await;

    assert_eq!(output.stdout.trim(), path_text(&area.allowed));
}

#[tokio::test]
async fn reports_the_exit_code_and_stderr() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();

    let output = run(
        &sandbox,
        spec(&area, &["/bin/sh", "-c", "echo oops >&2; exit 7"]),
    )
    .await;

    assert_eq!(output.exit_code, Some(7));
    assert_eq!(output.stderr.trim(), "oops");
    assert!(!output.timed_out);
}

#[tokio::test]
async fn stops_a_command_at_the_timeout() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let mut slow = spec(&area, &["/bin/sleep", "30"]);
    slow.timeout = Duration::from_millis(400);
    let start = Instant::now();

    let output = run(&sandbox, slow).await;

    assert!(output.timed_out);
    assert_eq!(output.exit_code, None);
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn truncates_output_at_the_limit() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();
    let mut noisy = spec(&area, &["/bin/sh", "-c", "yes | head -c 200000"]);
    noisy.max_output_bytes = 1000;

    let output = run(&sandbox, noisy).await;

    assert_eq!(output.exit_code, Some(0));
    assert!(output.truncated);
    assert_eq!(output.stdout.len(), 1000);
}

#[tokio::test]
async fn an_empty_command_is_refused() {
    let Some(sandbox) = sandbox().await else {
        return;
    };
    let area = area();

    let result = sandbox.run(spec(&area, &[])).await;

    assert!(result.is_err());
}

#[cfg(target_os = "macos")]
mod process_cleanup {
    use super::*;

    async fn wait_until_gone(pid: i32) -> bool {
        for _ in 0..40 {
            if unsafe { libc::kill(pid, 0) } != 0 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    }

    fn background_script(area: &Area) -> String {
        let pid_file = path_text(&area.allowed.join("child.pid"));
        format!("sleep 31 & echo $! > '{pid_file}'; wait")
    }

    fn read_pid(area: &Area) -> i32 {
        std::fs::read_to_string(area.allowed.join("child.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    #[tokio::test]
    async fn a_timeout_kills_the_whole_process_group() {
        let Some(sandbox) = sandbox().await else {
            return;
        };
        let area = area();
        let script = background_script(&area);
        let mut slow = spec(&area, &["/bin/sh", "-c", &script]);
        slow.timeout = Duration::from_millis(800);

        let output = run(&sandbox, slow).await;

        assert!(output.timed_out);
        assert!(wait_until_gone(read_pid(&area)).await);
    }

    #[tokio::test]
    async fn dropping_the_run_kills_the_whole_process_group() {
        let Some(sandbox) = sandbox().await else {
            return;
        };
        let area = area();
        let script = background_script(&area);
        let slow = spec(&area, &["/bin/sh", "-c", &script]);

        let result = tokio::time::timeout(Duration::from_millis(800), sandbox.run(slow)).await;

        assert!(result.is_err());
        assert!(wait_until_gone(read_pid(&area)).await);
    }
}

#[test]
fn the_seatbelt_profile_denies_network_by_default() {
    let area = area();
    let profile = nook_core::sandbox::seatbelt_profile(&spec(&area, &["/bin/true"]));

    assert!(profile.contains("(deny default)"));
    assert!(profile.contains("(deny network*)"));
    assert!(!profile.contains("(allow network*)"));
}

#[test]
fn the_seatbelt_profile_escapes_quotes_in_paths() {
    let area = area();
    let mut quoted = spec(&area, &["/bin/true"]);
    quoted.write_roots = vec![PathBuf::from("/tmp/a\"b")];

    let profile = nook_core::sandbox::seatbelt_profile(&quoted);

    assert!(profile.contains("/tmp/a\\\"b"));
}

#[test]
fn the_bubblewrap_arguments_unshare_the_network_unless_allowed() {
    let area = area();
    let closed = nook_core::sandbox::bubblewrap_args(&spec(&area, &["/bin/true"]), true);
    let mut open_spec = spec(&area, &["/bin/true"]);
    open_spec.network = true;
    let open = nook_core::sandbox::bubblewrap_args(&open_spec, true);

    assert!(closed.contains(&"--unshare-all".to_string()));
    assert!(!closed.contains(&"--share-net".to_string()));
    assert!(open.contains(&"--share-net".to_string()));
    assert!(closed.contains(&"--clearenv".to_string()));
}
