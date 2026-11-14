// Warning: the next plan task uses these building blocks.
#![allow(dead_code)]

fn check_title(value: &str) -> Result<&str, ()> {
    let title = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .unwrap_or(value);
    if title.is_empty() || title.chars().count() > 200 || title.contains(['\r', '\n', '\0']) {
        return Err(());
    }
    Ok(title)
}

fn check_body(value: &str) -> Result<&str, ()> {
    check_plain_text(value, 0, 20_000)
}

fn check_text(value: &str) -> Result<&str, ()> {
    check_plain_text(value, 1, 20_000)
}

fn check_plain_text(value: &str, min: usize, max: usize) -> Result<&str, ()> {
    let count = value.chars().count();
    if value.contains('\0') || count < min || count > max {
        return Err(());
    }
    Ok(value)
}

fn check_note_id(value: &str) -> Result<&str, ()> {
    if !(14..=300).contains(&value.chars().count())
        || !value.starts_with("x-coredata://")
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":/._%-".contains(c))
    {
        return Err(());
    }
    Ok(value)
}

fn create_html(title: &str, body: &str) -> Result<String, ()> {
    Ok(format!(
        "<div>{}</div>{}",
        escape_html(title),
        html_lines(body)
    ))
}

fn append_html(text: &str) -> Result<String, ()> {
    Ok(html_lines(check_text(text)?))
}

fn html_lines(value: &str) -> String {
    value
        .split('\n')
        .map(|line| format!("<div>{}</div>", escape_html(line)))
        .collect()
}

fn escape_html(value: &str) -> String {
    value.chars().fold(String::new(), |mut escaped, character| {
        escaped.push_str(match character {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            _ => {
                escaped.push(character);
                return escaped;
            }
        });
        escaped
    })
}

#[cfg(unix)]
#[derive(Clone)]
pub(crate) struct NotesRunner {
    program: std::path::PathBuf,
    lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    timeout: std::time::Duration,
}

#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NotesRunResult {
    Success(String),
    PermissionDenied,
    NotFound,
    WrongTitle,
    Failed,
    TimedOut,
    SpawnFailed,
}

#[cfg(unix)]
impl NotesRunner {
    pub(crate) fn new(
        program: std::path::PathBuf,
        lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    ) -> Self {
        Self {
            program,
            lock,
            timeout: std::time::Duration::from_secs(60),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub(crate) async fn run(
        &self,
        script: &str,
        arguments: &[&str],
        output_limit: usize,
    ) -> NotesRunResult {
        use crate::sandbox::{join_reader, read_capped, GroupKill};
        use std::process::Stdio;
        use tokio::process::Command;
        let _guard = self.lock.lock().await;
        let mut command = Command::new(&self.program);
        command
            .arg("-e")
            .arg(script)
            .args(arguments.iter().map(|value| format!(":{value}")));
        command
            .current_dir("/")
            .env_clear()
            .env("LANG", "en_US.UTF-8")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let Ok(mut child) = command.spawn() else {
            return NotesRunResult::SpawnFailed;
        };
        let _group = GroupKill(child.id().map(|pid| pid as i32));
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let stdout_task = tokio::spawn(read_capped(stdout, output_limit));
        let stderr_task = tokio::spawn(read_capped(stderr, 4096));
        let status = match tokio::time::timeout(self.timeout, child.wait()).await {
            Ok(Ok(status)) => status,
            _ => {
                drop(_group);
                let _ = child.wait().await;
                return NotesRunResult::TimedOut;
            }
        };
        let (stdout, _) = join_reader(stdout_task).await;
        let (stderr, _) = join_reader(stderr_task).await;
        let stderr = String::from_utf8_lossy(&stderr);
        let code = status.code();
        let number = ["(-1743)", "(-1728)", "(9001)", "(9002)"]
            .into_iter()
            .find(|n| stderr.contains(n));
        tracing::debug!(exit_status = ?code, error_number = ?number, "Notes script completed");
        if status.success() {
            return NotesRunResult::Success(String::from_utf8_lossy(&stdout).into_owned());
        }
        match number {
            Some("(-1743)") => NotesRunResult::PermissionDenied,
            Some("(-1728)") | Some("(9001)") => NotesRunResult::NotFound,
            Some("(9002)") => NotesRunResult::WrongTitle,
            _ => NotesRunResult::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::sync::Arc;
    #[cfg(unix)]
    use std::time::Duration;

    #[test]
    fn title_removes_one_trailing_line_break() {
        assert_eq!(check_title("Title\n").unwrap(), "Title");
        assert_eq!(check_title("Title\r\n").unwrap(), "Title");
    }

    #[test]
    fn title_refuses_empty_or_multiline_values() {
        for value in ["\n", "Title\n\n", "Ti\ntle"] {
            assert!(check_title(value).is_err());
        }
    }

    #[test]
    fn a_nul_in_a_title_is_refused() {
        assert!(check_title("a\0b").is_err());
    }

    #[test]
    fn option_like_title_is_accepted() {
        assert_eq!(check_title("-e"), Ok("-e"));
    }

    #[test]
    fn a_note_id_over_300_characters_is_refused() {
        let over_300 = format!("x-coredata://{}", "a".repeat(288));
        assert!(check_note_id(&over_300).is_err());
    }

    #[test]
    fn plain_text_checks_lengths_and_nul() {
        assert!(check_body("").is_ok());
        assert!(check_text("text").is_ok());
        assert!(check_body(&"x".repeat(20_001)).is_err());
        assert!(check_text("x\0y").is_err());
    }

    #[test]
    fn note_id_requires_prefix_and_allowed_characters() {
        assert!(check_note_id("x-coredata://ABC-123/._%:").is_ok());
        assert!(check_note_id("-e").is_err());
        assert!(check_note_id("ABC-x-coredata://123").is_err());
    }

    #[test]
    fn html_escapes_content_and_puts_title_first() {
        assert_eq!(
            create_html("T&<>", "a&<>").unwrap(),
            "<div>T&amp;&lt;&gt;</div><div>a&amp;&lt;&gt;</div>"
        );
        assert_eq!(append_html("a\nb").unwrap(), "<div>a</div><div>b</div>");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_prefixes_data_and_captures_output() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf '%s\\n' \"$#\" \"$1\" \"$2\" \"$3\" \"$PWD\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("script", &["-e"], 4096).await;
        assert_eq!(
            result,
            NotesRunResult::Success("3\n-e\nscript\n:-e\n/\n".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_known_error_numbers_to_fixed_results() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'failure (9002)' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("script", &[], 4096).await,
            NotesRunResult::WrongTitle
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_one_value_call_has_exactly_three_arguments() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf '%s\n' \"$#\" \"$1\" \"$2\" \"$3\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("the_script", &["val"], 4096).await;
        assert_eq!(
            result,
            NotesRunResult::Success("3\n-e\nthe_script\n:val\n".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_option_like_value_arrives_with_a_colon_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\nprintf '%s\n' \"$3\"\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("the_script", &["-e"], 4096).await;
        assert_eq!(result, NotesRunResult::Success(":-e\n".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_working_folder_is_root() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\npwd -P\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("the_script", &[], 4096).await;
        assert_eq!(result, NotesRunResult::Success("/\n".to_string()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_timeout_kills_the_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("child.pid");
        let program = dir.path().join("osascript");
        let script = format!(
            "#!/bin/sh\nsleep 300 &\necho $! > \"{}\"\nsleep 300\n",
            pid_file.display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())))
            .with_timeout(Duration::from_millis(200));
        let result = runner.run("the_script", &[], 4096).await;
        assert_eq!(result, NotesRunResult::TimedOut);
        let pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let start = std::time::Instant::now();
        let mut gone = false;
        while start.elapsed() < Duration::from_secs(2) {
            if unsafe { libc::kill(pid, 0) } != 0 {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(gone);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancel_kills_the_process_group() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("child.pid");
        let program = dir.path().join("osascript");
        let script = format!(
            "#!/bin/sh\nsleep 300 &\necho $! > \"{}\"\nsleep 300\n",
            pid_file.display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            runner.run("the_script", &[], 4096),
        )
        .await;
        assert!(result.is_err());
        let start = std::time::Instant::now();
        while !pid_file.exists() && start.elapsed() < Duration::from_secs(1) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let start = std::time::Instant::now();
        let mut gone = false;
        while start.elapsed() < Duration::from_secs(2) {
            if unsafe { libc::kill(pid, 0) } != 0 {
                gone = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(gone);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_exceeding_limit_is_drained_and_capped() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(
            &program,
            "#!/bin/sh\nhead -c 1048576 /dev/zero | tr '\\0' 'a'\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("the_script", &[], 4096).await;
        let NotesRunResult::Success(text) = result else {
            panic!("expected success");
        };
        assert_eq!(text.len(), 4096);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_returns_success_for_zero_exit_status() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'done'\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::Success("done\n".to_string())
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_permission_denied_error() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'error (-1743)' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::PermissionDenied
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_not_found_error_1728() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'error (-1728)' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::NotFound
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_not_found_error_9001() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'error (9001)' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::NotFound
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_wrong_title_error() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'error (9002)' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::WrongTitle
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_maps_unrecognized_error_to_failed() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("osascript");
        std::fs::write(&program, "#!/bin/sh\necho 'other error' >&2\nexit 1\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::Failed
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn runner_returns_spawn_failed_for_missing_program() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing_osascript");
        let runner = NotesRunner::new(missing, Arc::new(tokio::sync::Mutex::new(())));
        assert_eq!(
            runner.run("the_script", &[], 4096).await,
            NotesRunResult::SpawnFailed
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn parallel_calls_never_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let log_file = dir.path().join("calls.log");
        let program = dir.path().join("osascript");
        let script = format!(
            "#!/bin/sh\necho start >> \"{}\"\nsleep 0.05\necho end >> \"{}\"\n",
            log_file.display(),
            log_file.display(),
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let (first, second) = tokio::join!(
            runner.run("script1", &[], 4096),
            runner.run("script2", &[], 4096),
        );
        assert!(matches!(first, NotesRunResult::Success(_)));
        assert!(matches!(second, NotesRunResult::Success(_)));
        let content = std::fs::read_to_string(&log_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines, vec!["start", "end", "start", "end"]);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn child_environment_is_exactly_lang() {
        let dir = tempfile::tempdir().unwrap();
        let env_file = dir.path().join("environ");
        let program = dir.path().join("osascript");
        let script = format!(
            "#!/bin/sh\ncat /proc/$$/environ > \"{}\"\n",
            env_file.display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runner = NotesRunner::new(program, Arc::new(tokio::sync::Mutex::new(())));
        let result = runner.run("the_script", &[], 4096).await;
        assert!(matches!(result, NotesRunResult::Success(_)));
        let bytes = std::fs::read(&env_file).unwrap();
        assert_eq!(bytes, b"LANG=en_US.UTF-8\0");
    }
}
