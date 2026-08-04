//! ferogram client construction and lifecycle.

use std::path::Path;
use std::sync::Arc;

use ferogram::{parse_proxy_link, Client, ShutdownToken};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::proxy_settings::ProxySettings;
use crate::session::EncryptedSessionBackend;

/// Loads the encrypted session backend, connects a ferogram client, and returns
/// the client plus a shutdown token for clean teardown.
///
/// When `proxy` is `Some`, routes all MTProto traffic through that MTProto proxy
/// (tg-ws-proxy / Telegram Desktop compatible).
pub async fn build(
    config: &Config,
    data_dir: &Path,
    proxy: Option<&ProxySettings>,
) -> AppResult<(Client, ShutdownToken)> {
    let backend = EncryptedSessionBackend::arc(data_dir);
    let mut builder = Client::builder()
        .api_id(config.api_id)
        .api_hash(config.api_hash.clone())
        .session_backend(backend as Arc<dyn ferogram::SessionBackend>)
        .device_model("SoundGrammy")
        .app_version(env!("CARGO_PKG_VERSION"))
        .catch_up(false);

    if let Some(proxy) = proxy {
        let link = proxy.to_proxy_link();
        let cfg = parse_proxy_link(&link)
            .or_else(|| {
                let alt = format!(
                    "https://t.me/proxy?server={}&port={}&secret={}",
                    proxy.server.trim(),
                    proxy.port,
                    proxy.secret.trim()
                );
                parse_proxy_link(&alt)
            })
            .ok_or_else(|| AppError::msg("invalid MTProto proxy configuration"))?;
        builder = builder.mtproxy(cfg);
    }

    let connect = builder.connect();
    let (client, shutdown) = match tokio::time::timeout(
        std::time::Duration::from_secs(20),
        connect,
    )
    .await
    {
        Ok(Ok(pair)) => pair,
        Ok(Err(e)) => {
            let err_text = e.to_string();
            return Err(AppError::msg(map_connect_error(&err_text, proxy.is_some())));
        }
        Err(_elapsed) => {
            return Err(AppError::msg(
                "telegram connect timed out after 20s. If using a proxy, confirm tg-ws-proxy is running and the secret matches.",
            ));
        }
    };

    Ok((client, shutdown))
}

fn map_connect_error(err_text: &str, used_proxy: bool) -> String {
    if used_proxy && err_text.contains("transport code") {
        return format!(
            "MTProxy framing error while reading Telegram data (transport code). \
             Confirm tg-ws-proxy is running and the secret matches the tray “Copy link”. \
             Detail: {err_text}"
        );
    }
    format!("telegram connect failed: {err_text}")
}

/// Whether the persisted session corresponds to an authorized account.
pub async fn is_authorized(client: &Client) -> AppResult<bool> {
    Ok(client.is_authorized().await?)
}

#[cfg(test)]
mod tests {
    use super::map_connect_error;

    #[test]
    fn map_connect_error_special_cases_proxy_transport_code() {
        let msg = map_connect_error(
            "deserialize error: Io(Custom { kind: Other, error: \"transport code -149480985\" })",
            true,
        );
        assert!(msg.contains("MTProxy framing error"));
        assert!(msg.contains("transport code"));
    }

    #[test]
    fn map_connect_error_passthrough_without_proxy() {
        let raw = "deserialize error: Io(Custom { kind: Other, error: \"transport code -1\" })";
        let msg = map_connect_error(raw, false);
        assert_eq!(msg, format!("telegram connect failed: {raw}"));
    }

    #[test]
    fn map_connect_error_passthrough_other_proxy_errors() {
        let raw = "connection timed out";
        let msg = map_connect_error(raw, true);
        assert_eq!(msg, format!("telegram connect failed: {raw}"));
    }
}
