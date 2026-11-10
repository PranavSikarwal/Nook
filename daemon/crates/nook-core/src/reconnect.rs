use crate::db;
use crate::permissions;
use crate::server::AppState;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

pub const RECONNECT_INTERVAL: Duration = Duration::from_secs(10);

/// Retries the database connection until it works, then prepares the database
/// and loads the stored grants. Call it when the Daemon started without a
/// database. It returns once the pool is set.
pub async fn reconnect_database(
    state: Arc<AppState>,
    database_url: String,
    interval: Duration,
    attempt_timeout: Duration,
) {
    let mut attempt = 0u32;
    while state.pool.get().is_none() {
        tokio::time::sleep(interval).await;
        attempt += 1;
        let pool = match db::create_pool_with_timeout(&database_url, attempt_timeout).await {
            Ok(pool) => pool,
            Err(error) => {
                warn!("Database reconnect attempt {attempt} failed: {error}");
                continue;
            }
        };
        if let Err(error) = db::run_migrations(&pool).await {
            warn!("Database migrations failed after reconnecting: {error}");
            pool.close().await;
            continue;
        }
        if let Ok(repaired) = db::repair_open_replies(&pool).await {
            if repaired > 0 {
                info!(repaired_count = repaired, "Repaired open replies");
            }
        }
        state.pool.set(pool);
        permissions::restore(&state).await;
        info!("Database connected after {attempt} reconnect attempts");
    }
}
