#![allow(dead_code)] // Warning: the next plan task uses these building blocks.
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

#[cfg(unix)]
#[derive(Clone)]
pub(crate) struct NotesRunner {
    program: std::path::PathBuf,
    lock: std::sync::Arc<tokio::sync::Mutex<()>>,
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
        Self { program, lock }
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
        let status =
            match tokio::time::timeout(std::time::Duration::from_secs(60), child.wait()).await {
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

#[cfg(all(test, unix))]
mod runner_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

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
}
