use std::fs;
use std::path::PathBuf;
use nook_core::protocol::{
    ClientMessage, DaemonMessage, DaemonWorkerEvent, DaemonWorkerRequest,
};

fn contracts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("contracts")
        .join("examples")
}

#[test]
fn test_panel_daemon_examples() {
    let path = contracts_dir().join("panel-daemon.ndjson");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 21, "Expected 21 examples in panel-daemon.ndjson");

    // Lines 0..8 are ClientRequests
    for (i, line) in lines[..8].iter().enumerate() {
        let msg: ClientMessage = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Line {i} failed to parse as ClientMessage: {e}\n{line}"));
        let round_trip = serde_json::to_string(&msg).unwrap();
        let _: ClientMessage = serde_json::from_str(&round_trip).unwrap();
    }

    // Lines 8..21 are DaemonReplies
    for (i, line) in lines[8..].iter().enumerate() {
        let msg: DaemonMessage = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Line {} failed to parse as DaemonMessage: {e}\n{line}", i + 8));
        let round_trip = serde_json::to_string(&msg).unwrap();
        let _: DaemonMessage = serde_json::from_str(&round_trip).unwrap();
    }
}

#[test]
fn test_daemon_worker_examples() {
    let path = contracts_dir().join("daemon-worker.ndjson");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));

    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 14, "Expected 14 examples in daemon-worker.ndjson");

    // Lines 0..5 are DaemonWorkerRequest
    for (i, line) in lines[..5].iter().enumerate() {
        let msg: DaemonWorkerRequest = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Line {i} failed to parse as DaemonWorkerRequest: {e}\n{line}"));
        let round_trip = serde_json::to_string(&msg).unwrap();
        let _: DaemonWorkerRequest = serde_json::from_str(&round_trip).unwrap();
    }

    // Lines 5..14 are DaemonWorkerEvent
    for (i, line) in lines[5..].iter().enumerate() {
        let msg: DaemonWorkerEvent = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Line {} failed to parse as DaemonWorkerEvent: {e}\n{line}", i + 5));
        let round_trip = serde_json::to_string(&msg).unwrap();
        let _: DaemonWorkerEvent = serde_json::from_str(&round_trip).unwrap();
    }
}
