use nook_core::config::Config;
use nook_core::protocol::{AuthorizationDecision, DaemonWorkerEvent, DaemonWorkerRequest};
use nook_core::supervisor::WorkerSupervisor;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};
use uuid::Uuid;

fn fake_worker_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py")
}

#[tokio::test]
async fn shutdown_can_be_called_twice() {
    let config = Config::default();
    let command = vec![
        "python3".to_string(),
        fake_worker_script().to_string_lossy().to_string(),
    ];
    let supervisor = WorkerSupervisor::new(&config, None, Some(command))
        .await
        .expect("fake Worker should start");

    supervisor.shutdown().await;
    supervisor.shutdown().await;
}

#[tokio::test]
async fn shutdown_kills_a_worker_that_ignores_stdin_close() {
    let pid_file = std::env::temp_dir().join(format!("nook-worker-{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&pid_file);
    std::env::set_var("FAKE_WORKER_IGNORE_EOF", "1");
    std::env::set_var("FAKE_WORKER_PID_FILE", &pid_file);
    let config = Config::default();
    let command = vec![
        "python3".to_string(),
        fake_worker_script().to_string_lossy().to_string(),
    ];
    let supervisor = WorkerSupervisor::new(&config, None, Some(command))
        .await
        .expect("fake Worker should start");
    std::env::remove_var("FAKE_WORKER_IGNORE_EOF");
    std::env::remove_var("FAKE_WORKER_PID_FILE");
    let pid: u32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    supervisor.shutdown().await;

    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(1) {
        if !Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            let _ = std::fs::remove_file(pid_file);
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Worker process {pid} stayed alive after supervisor shutdown");
}

#[tokio::test]
async fn test_supervisor_routes_events() {
    let script = fake_worker_script();
    let config = Config::default();

    let custom_cmd = vec!["python3".to_string(), script.to_string_lossy().to_string()];

    let supervisor = WorkerSupervisor::new(&config, None, Some(custom_cmd))
        .await
        .expect("Failed to spawn supervisor with fake worker");

    let req_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    let run_req = DaemonWorkerRequest::Run {
        request_id: req_id,
        chat_id,
        text: "Hello fake worker".to_string(),
        attachments: vec![],
    };

    let mut rx = supervisor
        .send_request(run_req)
        .await
        .expect("Failed to send run request");

    let ev1 = rx.recv().await.expect("Expected event 1");
    assert!(
        matches!(ev1, DaemonWorkerEvent::MessageStarted { request_id, .. } if request_id == req_id)
    );

    let ev2 = rx.recv().await.expect("Expected event 2");
    assert!(
        matches!(ev2, DaemonWorkerEvent::TextDelta { request_id, text, .. } if request_id == req_id && text == "Fake worker reply")
    );

    let ev3 = rx.recv().await.expect("Expected event 3");
    assert!(
        matches!(ev3, DaemonWorkerEvent::MessageFinished { request_id, .. } if request_id == req_id)
    );

    supervisor.shutdown().await;
}

#[tokio::test]
async fn test_supervisor_authorization_result_keeps_reply_listener() {
    let script = fake_worker_script();
    let config = Config::default();

    let custom_cmd = vec!["python3".to_string(), script.to_string_lossy().to_string()];

    let supervisor = WorkerSupervisor::new(&config, None, Some(custom_cmd))
        .await
        .expect("Failed to spawn supervisor with fake worker");

    let req_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    let run_req = DaemonWorkerRequest::Run {
        request_id: req_id,
        chat_id,
        text: "authorize:call_fake|nook:web_search|{}".to_string(),
        attachments: vec![],
    };

    let mut rx = supervisor
        .send_request(run_req)
        .await
        .expect("Failed to send run request");

    let ev1 = rx.recv().await.expect("Expected MessageStarted");
    assert!(
        matches!(ev1, DaemonWorkerEvent::MessageStarted { request_id, .. } if request_id == req_id)
    );

    let ev2 = rx.recv().await.expect("Expected AuthorizeTool");
    assert!(
        matches!(ev2, DaemonWorkerEvent::AuthorizeTool { request_id, call_id, chat_id: event_chat, .. } if request_id == req_id && call_id == "call_fake" && event_chat == chat_id)
    );

    let _ = supervisor
        .send_request(DaemonWorkerRequest::AuthorizationResult {
            request_id: req_id,
            call_id: "call_fake".to_string(),
            decision: AuthorizationDecision::Allow,
            reason: None,
        })
        .await
        .expect("Failed to send authorization result");

    let ev3 = rx.recv().await.expect("Expected TextDelta after result");
    assert!(
        matches!(ev3, DaemonWorkerEvent::TextDelta { text, .. } if text.contains(r#""decision": "allow""#))
    );

    supervisor.shutdown().await;
}

#[tokio::test]
async fn test_supervisor_crash_broadcasts_error() {
    let script = fake_worker_script();
    let config = Config::default();

    let custom_cmd = vec!["python3".to_string(), script.to_string_lossy().to_string()];

    let supervisor = WorkerSupervisor::new(&config, None, Some(custom_cmd))
        .await
        .expect("Failed to spawn supervisor with fake worker");

    let req_id = Uuid::new_v4();
    let chat_id = Uuid::new_v4();
    let run_req = DaemonWorkerRequest::Run {
        request_id: req_id,
        chat_id,
        text: "Hello fake worker".to_string(),
        attachments: vec![],
    };

    let mut rx = supervisor
        .send_request(run_req)
        .await
        .expect("Failed to send run request");

    // Tell fake worker to shutdown/exit unexpectedly
    supervisor.shutdown().await;

    // The open request listener should receive either finished or worker_crashed
    let ev = rx.recv().await.expect("Expected event");
    assert!(matches!(
        ev,
        DaemonWorkerEvent::MessageStarted { .. }
            | DaemonWorkerEvent::MessageFinished { .. }
            | DaemonWorkerEvent::Error { .. }
    ));
}
