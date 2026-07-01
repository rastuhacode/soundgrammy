//! grammers client construction and lifecycle.

use std::path::Path;
use std::sync::Arc;

use grammers_client::Client;
use grammers_mtsender::SenderPool;
use grammers_session::storages::MemorySession;

use crate::config::Config;
use crate::error::AppResult;
use crate::session;

/// Loads the encrypted session, builds a connected client, and spawns the
/// network runner in the background. Returns the shared session handle and the
/// client.
pub fn build(config: &Config, data_dir: &Path) -> AppResult<(Arc<MemorySession>, Client)> {
    let session = session::load_or_create(data_dir)?;
    let SenderPool { runner, handle, .. } = SenderPool::new(Arc::clone(&session), config.api_id);
    let client = Client::new(handle);

    // The client owns the handle needed to talk to the runner; spawning the
    // runner drives all network I/O for the lifetime of the app.
    tauri::async_runtime::spawn(runner.run());

    Ok((session, client))
}

/// Whether the persisted session corresponds to an authorized account.
pub async fn is_authorized(client: &Client) -> AppResult<bool> {
    Ok(client.is_authorized().await?)
}
