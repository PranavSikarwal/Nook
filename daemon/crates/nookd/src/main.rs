use nook_core::config::Config;
use nook_core::db;
use nook_core::keychain;
use nook_core::policy_assets::{find_policy_directory, PolicyAssets};
use nook_core::providers::ProviderRegistry;
use nook_core::server::{AppState, Server};
use nook_core::supervisor::WorkerSupervisor;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {}
        [flag] if flag == "--check-policy" => std::process::exit(check_policy()),
        _ => {
            eprintln!("Usage: nookd [--check-policy]");
            std::process::exit(2);
        }
    }

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

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

    // 1b. Load and validate tool policy before accepting any tool work
    let policy = Arc::new(PolicyAssets::load_default().map_err(|e| {
        error!("Policy assets failed to load: {e}");
        e
    })?);
    info!(tools = policy.catalog.tools.len(), "Loaded tool policy");

    // 2. Read Keychain / env
    let api_key = keychain::get_api_key();

    // 3. Connect to Postgres with 30s retry
    let pool = db::connect_with_retry(
        &config.database_url,
        Duration::from_secs(30),
        db::CONNECT_ATTEMPT_TIMEOUT,
    )
    .await;

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
    let providers = Arc::new(ProviderRegistry::empty());
    let supervisor = match WorkerSupervisor::with_tools(
        &config,
        api_key,
        e2e_worker_command(),
        providers.descriptors().to_vec(),
    )
    .await
    {
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
        approval_timeout: approval_timeout(),
        providers,
        running_tools: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        policy,
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

fn check_policy() -> i32 {
    match PolicyAssets::load_default() {
        Ok(policy) => {
            let dir = find_policy_directory().unwrap_or_default();
            println!(
                "policy ok: {} tools from {}",
                policy.catalog.tools.len(),
                dir.display()
            );
            0
        }
        Err(e) => {
            eprintln!("policy check failed: {e}");
            1
        }
    }
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
fn approval_timeout() -> Duration {
    std::env::var("NOOK_E2E_APPROVAL_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis)
        .unwrap_or(nook_core::tool_approval::APPROVAL_TIMEOUT)
}

#[cfg(not(feature = "e2e"))]
fn approval_timeout() -> Duration {
    nook_core::tool_approval::APPROVAL_TIMEOUT
}

#[cfg(feature = "e2e")]
fn deterministic_worker_enabled() -> bool {
    std::env::var("NOOK_E2E_WORKER_COMMAND").is_ok()
}

#[cfg(not(feature = "e2e"))]
fn deterministic_worker_enabled() -> bool {
    false
}
