use std::path::PathBuf;
use std::process::{Command, Stdio};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("nook-secret-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("failed to create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nookctl_secret_set_reads_standard_input_and_takes_no_value_argument() {
    let temp = TempDir::new();
    let bin = env!("CARGO_BIN_EXE_nookctl");

    // (a) `secret set a b` is refused by the argument parser (non-zero exit, because a value argument does not exist)
    let output_a = Command::new(bin)
        .env("NOOK_APP_DIR", temp.path())
        .env("HOME", temp.path())
        .args(["secret", "set", "a", "b"])
        .output()
        .expect("failed to run nookctl");

    assert!(
        !output_a.status.success(),
        "`secret set a b` should fail because extra arguments are refused"
    );

    // (b) `secret set a` with empty standard input exits non-zero, prints the fixed empty-value text, and does NOT print "Is nookd running?"
    let mut child_b = Command::new(bin)
        .env("NOOK_APP_DIR", temp.path())
        .env("HOME", temp.path())
        .args(["secret", "set", "a"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn nookctl");

    // Close stdin with no bytes written
    drop(child_b.stdin.take());

    let output_b = child_b
        .wait_with_output()
        .expect("failed to wait for nookctl");

    assert!(
        !output_b.status.success(),
        "`secret set a` with empty stdin must exit non-zero"
    );

    let stdout_b = String::from_utf8_lossy(&output_b.stdout);
    let stderr_b = String::from_utf8_lossy(&output_b.stderr);
    let combined_b = format!("{stdout_b}\n{stderr_b}");

    assert!(
        combined_b.contains("The value is empty."),
        "expected output to contain 'The value is empty.', got:\n{combined_b}"
    );
    assert!(
        !combined_b.contains("Is nookd running?"),
        "expected output to NOT contain 'Is nookd running?', got:\n{combined_b}"
    );
}
