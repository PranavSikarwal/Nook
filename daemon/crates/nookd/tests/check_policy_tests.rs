use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn repo_policy_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../policy")
        .canonicalize()
        .unwrap()
}

fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nook-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_check_policy(policy_dir: &Path, app_dir: &Path) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nookd"))
        .arg("--check-policy")
        .env("NOOK_POLICY_DIR", policy_dir)
        .env("NOOK_APP_DIR", app_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("nookd should start");
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            child.kill().ok();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    child.wait_with_output().unwrap()
}

#[test]
fn check_policy_succeeds_with_the_repository_policy() {
    let dir = repo_policy_dir();
    let app_dir = scratch_dir("check-policy-ok-app");

    let output = run_check_policy(&dir, &app_dir);
    std::fs::remove_dir_all(&app_dir).ok();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with("policy ok: "), "stdout: {stdout}");
    assert!(stdout.contains(" tools from "), "stdout: {stdout}");
    assert!(stdout.contains(dir.to_str().unwrap()), "stdout: {stdout}");
    assert_eq!(stdout.lines().count(), 1);
}

#[test]
fn check_policy_fails_with_an_empty_policy_directory() {
    let empty = scratch_dir("check-policy-empty");
    let app_dir = scratch_dir("check-policy-empty-app");

    let output = run_check_policy(&empty, &app_dir);
    std::fs::remove_dir_all(&empty).ok();
    std::fs::remove_dir_all(&app_dir).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn check_policy_finds_the_policy_next_to_the_daemon_directory() {
    let root = scratch_dir("check-policy-layout");
    let cwd = scratch_dir("check-policy-layout-cwd");
    let daemon_dir = root.join("daemon");
    std::fs::create_dir_all(&daemon_dir).unwrap();
    let binary = daemon_dir.join(Path::new(env!("CARGO_BIN_EXE_nookd")).file_name().unwrap());
    std::fs::copy(env!("CARGO_BIN_EXE_nookd"), &binary).unwrap();
    copy_dir(&repo_policy_dir(), &root.join("policy"));

    let output = Command::new(&binary)
        .arg("--check-policy")
        .env_remove("NOOK_POLICY_DIR")
        .current_dir(&cwd)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&cwd).ok();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with("policy ok: "), "stdout: {stdout}");
}

#[test]
fn nookd_rejects_arguments_other_than_check_policy() {
    for args in [
        vec!["--bogus"],
        vec!["--bogus", "--check-policy"],
        vec!["--check-policy", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_nookd"))
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Usage: nookd"),
            "{args:?}"
        );
    }
}
