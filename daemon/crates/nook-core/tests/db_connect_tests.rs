use nook_core::db::connect_with_retry;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;

async fn silent_server() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = accepted.clone();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            held.push(stream);
        }
    });
    (port, accepted)
}

#[tokio::test]
async fn a_server_that_never_answers_gets_several_attempts_inside_the_budget() {
    let (port, accepted) = silent_server().await;
    let url = format!("postgresql://nook@127.0.0.1:{port}/nook");
    let start = Instant::now();

    let pool = connect_with_retry(&url, Duration::from_secs(4), Duration::from_millis(500)).await;

    assert!(pool.is_none());
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "{:?}",
        start.elapsed()
    );
    assert!(
        accepted.load(Ordering::SeqCst) >= 3,
        "attempts: {}",
        accepted.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn a_closed_port_fails_after_the_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let url = format!("postgresql://nook@127.0.0.1:{port}/nook");

    let pool = connect_with_retry(&url, Duration::from_secs(2), Duration::from_millis(500)).await;

    assert!(pool.is_none());
}
