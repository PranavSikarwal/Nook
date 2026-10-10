use nook_core::config::Config;
use nook_core::db;
use nook_core::keychain;
use nook_core::server::{AppState, Server};
use nook_core::supervisor::WorkerSupervisor;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    // 0. Load .env if present
    Config::load_env_file();

    // 1. Read config
    let config_path = Config::default_config_path();
    let config = Config::from_file(&config_path)?;
    info!(
        model = %config.model,
        base_url = %config.base_url,
        "Loaded configuration"
    );

    // 2. Read Keychain / env
    let api_key = keychain::get_api_key();

    // 3. Connect to Postgres with 30s retry
    let pool = connect_db_with_retry(&config.database_url, Duration::from_secs(30)).await;

    // 4. Run migrations & 5. Repair open replies
    if let Some(ref p) = pool {
        if let Err(e) = db::run_migrations(p).await {
            error!("Failed to apply database migrations: {e}");
        } else {
            info!("Database migrations applied");
        }

        if let Ok(repaired) = db::repair_open_replies(p).await {
            if repaired > 0 {
                info!(
                    repaired_count = repaired,
                    "Repaired open replies from previous run"
                );
            }
        }
    } else {
        warn!("Starting daemon without database: requests will receive database_unavailable");
    }

    // 6. Start Worker supervisor
    let supervisor = match WorkerSupervisor::new(&config, api_key, e2e_worker_command()).await {
        Ok(sup) => {
            info!("Worker supervisor initialized");
            Some(sup)
        }
        Err(e) => {
            error!("Worker supervisor initialization failed: {e}");
            None
        }
    };

    let state = Arc::new(AppState {
        config: RwLock::new(config),
        config_path,
        pool,
        supervisor: Mutex::new(supervisor),
        preserve_worker_on_settings_update: deterministic_worker_enabled(),
        open_requests: Mutex::new(HashMap::new()),
        pending_approvals: Mutex::new(HashMap::new()),
        cancelled_requests: Mutex::new(std::collections::HashSet::new()),
    });

    // 7. Bind socket with permissions 0600
    let socket_path = Config::default_socket_path();
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let server = Server::bind(&socket_path, shutdown_rx, state).await?;

    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C (SIGINT), shutting down daemon");
                }
                _ = async {
                    if let Some(ref mut sig) = sigterm {
                        sig.recv().await
                    } else {
                        std::future::pending().await
                    }
                } => {
                    info!("Received SIGTERM, shutting down daemon");
                }
            }
        }
        #[cfg(not(unix))]
        {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                info!("Received Ctrl+C, shutting down daemon");
            }
        }
        let _ = shutdown_tx_clone.send(());
    });

    server.run().await?;
    Ok(())
}

#[cfg(feature = "e2e")]
fn e2e_worker_command() -> Option<Vec<String>> {
    let command = std::env::var("NOOK_E2E_WORKER_COMMAND").ok()?;
    Some(command.split_whitespace().map(str::to_owned).collect())
}

#[cfg(not(feature = "e2e"))]
fn e2e_worker_command() -> Option<Vec<String>> {
    None
}

#[cfg(feature = "e2e")]
fn deterministic_worker_enabled() -> bool {
    std::env::var("NOOK_E2E_WORKER_COMMAND").is_ok()
}

#[cfg(not(feature = "e2e"))]
fn deterministic_worker_enabled() -> bool {
    false
}

async fn connect_db_with_retry(db_url: &str, timeout_dur: Duration) -> Option<sqlx::PgPool> {
    let start = std::time::Instant::now();
    loop {
        match db::create_pool(db_url).await {
            Ok(pool) => return Some(pool),
            Err(e) => {
                if start.elapsed() >= timeout_dur {
                    warn!("Database connection failed after 30s timeout: {e}");
                    return None;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
