use crate::providers::ProviderRegistry;
use crate::server::AppState;
use std::time::Duration;

/// Stops the Worker, running tool tasks, and providers in that order.
/// Returns false when the steps do not finish within `timeout`.
pub async fn stop_everything(state: &AppState, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async {
        let supervisor = state.supervisor.lock().await.clone();
        if let Some(supervisor) = supervisor {
            supervisor.shutdown().await;
        }

        crate::tool_approval::abort_all_running_tools(state).await;

        let old = state.providers.current();
        state.providers.replace(ProviderRegistry::empty());
        while std::sync::Arc::strong_count(&old) > 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        drop(old);
    })
    .await
    .is_ok()
}
