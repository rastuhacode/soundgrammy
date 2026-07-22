//! ferogram client construction and lifecycle.

use std::path::Path;
use std::sync::Arc;

use ferogram::{Client, ShutdownToken};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::session::EncryptedSessionBackend;

/// Loads the encrypted session backend, connects a ferogram client, and returns
/// the client plus a shutdown token for clean teardown.
pub async fn build(config: &Config, data_dir: &Path) -> AppResult<(Client, ShutdownToken)> {
    let backend = EncryptedSessionBackend::arc(data_dir);
    let (client, shutdown) = Client::builder()
        .api_id(config.api_id)
        .api_hash(config.api_hash.clone())
        .session_backend(backend as Arc<dyn ferogram::SessionBackend>)
        .device_model("SoundGrammy")
        .app_version(env!("CARGO_PKG_VERSION"))
        .catch_up(false)
        .connect()
        .await
        .map_err(|e| AppError::msg(format!("telegram connect failed: {e}")))?;

    Ok((client, shutdown))
}

/// Whether the persisted session corresponds to an authorized account.
pub async fn is_authorized(client: &Client) -> AppResult<bool> {
    Ok(client.is_authorized().await?)
}
