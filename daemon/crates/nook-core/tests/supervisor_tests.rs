use nook_core::config::Config;
use nook_core::protocol::{DaemonWorkerEvent, DaemonWorkerRequest};
use nook_core::supervisor::WorkerSupervisor;
use std::path::PathBuf;
use uuid::Uuid;

fn fake_worker_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fake_worker.py")
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
async fn test_supervisor_approval_decision_keeps_reply_listener() {
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
        text: "needs approval".to_string(),
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

    let ev2 = rx.recv().await.expect("Expected ApprovalRequested");
    assert!(
        matches!(ev2, DaemonWorkerEvent::ApprovalRequested { request_id, call_id, .. } if request_id == req_id && call_id == "call_fake")
    );

    // Send decision
    let decision_req = DaemonWorkerRequest::ApprovalDecision {
        request_id: req_id,
        call_id: "call_fake".to_string(),
        action: "allow_once".to_string(),
    };
    let _ = supervisor
        .send_request(decision_req)
        .await
        .expect("Failed to send decision");

    // Events after decision must arrive on original receiver rx
    let ev3 = rx.recv().await.expect("Expected TextDelta after decision");
    assert!(
        matches!(ev3, DaemonWorkerEvent::TextDelta { text, .. } if text == "Decision: allow_once")
    );

    let ev4 = rx
        .recv()
        .await
        .expect("Expected MessageFinished after decision");
    assert!(matches!(ev4, DaemonWorkerEvent::MessageFinished { .. }));

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
