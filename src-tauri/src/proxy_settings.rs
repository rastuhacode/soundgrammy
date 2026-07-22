//! Persistent MTProto proxy settings (tg-ws-proxy / Telegram Desktop compatible).

use ferogram::parse_proxy_link;
use serde::{Deserialize, Serialize};

use crate::db::Db;
use crate::error::{AppError, AppResult};

pub const SETTING_PROXY_ENABLED: &str = "proxy_enabled";
pub const SETTING_PROXY_SERVER: &str = "proxy_server";
pub const SETTING_PROXY_PORT: &str = "proxy_port";
pub const SETTING_PROXY_SECRET: &str = "proxy_secret";

/// User-editable MTProto proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub enabled: bool,
    pub server: String,
    pub port: u16,
    pub secret: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            server: String::new(),
            port: 1443,
            secret: String::new(),
        }
    }
}

impl ProxySettings {
    /// Canonical `tg://proxy?…` link for display / paste round-trips.
    pub fn to_proxy_link(&self) -> String {
        format!(
            "tg://proxy?server={}&port={}&secret={}",
            self.server.trim(),
            self.port,
            self.secret.trim()
        )
    }

    /// Whether fields are complete enough to attempt an MTProto proxy connection.
    pub fn is_configured(&self) -> bool {
        !self.server.trim().is_empty() && self.port > 0 && !self.secret.trim().is_empty()
    }

    /// Validate by parsing through ferogram (rejects secrets that would panic `.proxy_link`).
    pub fn validate(&self) -> AppResult<()> {
        if !self.is_configured() {
            if self.enabled {
                return Err(AppError::msg(
                    "proxy is enabled but server, port, and secret are required",
                ));
            }
            return Ok(());
        }
        let link = self.to_proxy_link();
        if parse_proxy_link(&link).is_none() {
            // Also accept https://t.me/proxy form for the same fields.
            let alt = format!(
                "https://t.me/proxy?server={}&port={}&secret={}",
                self.server.trim(),
                self.port,
                self.secret.trim()
            );
            if parse_proxy_link(&alt).is_none() {
                return Err(AppError::msg("invalid MTProto proxy secret or fields"));
            }
        }
        Ok(())
    }

    /// Settings to pass into ferogram when connecting (enabled + valid only).
    pub fn for_connect(&self) -> Option<&Self> {
        if self.enabled && self.is_configured() {
            Some(self)
        } else {
            None
        }
    }
}

/// Parse a `tg://proxy?…` or `https://t.me/proxy?…` link into editable fields.
/// Preserves the original secret string from the query (not re-encoded bytes).
pub fn from_proxy_link(url: &str) -> AppResult<ProxySettings> {
    let trimmed = url.trim();
    let cfg = parse_proxy_link(trimmed)
        .ok_or_else(|| AppError::msg("invalid MTProto proxy link"))?;

    let secret = extract_secret_param(trimmed).unwrap_or_else(|| {
        // Fallback: hex-encode parsed bytes if query parsing failed oddly.
        cfg.secret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    });

    Ok(ProxySettings {
        enabled: true,
        server: cfg.host,
        port: cfg.port,
        secret,
    })
}

fn extract_secret_param(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://t.me/proxy?")
        .or_else(|| url.strip_prefix("tg://proxy?"))?;
    for kv in rest.split('&') {
        let mut it = kv.splitn(2, '=');
        let key = it.next()?;
        let value = it.next()?;
        if key == "secret" {
            return Some(value.to_string());
        }
    }
    None
}

pub fn load(db: &Db) -> AppResult<ProxySettings> {
    let enabled = matches!(
        db.get_setting(SETTING_PROXY_ENABLED)?
            .as_deref()
            .map(str::trim),
        Some("1") | Some("true") | Some("TRUE") | Some("yes")
    );
    let server = db
        .get_setting(SETTING_PROXY_SERVER)?
        .unwrap_or_default();
    let port = db
        .get_setting(SETTING_PROXY_PORT)?
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(1443);
    let secret = db
        .get_setting(SETTING_PROXY_SECRET)?
        .unwrap_or_default();

    Ok(ProxySettings {
        enabled,
        server,
        port,
        secret,
    })
}

pub fn save(db: &Db, settings: &ProxySettings) -> AppResult<()> {
    settings.validate()?;
    db.set_setting(
        SETTING_PROXY_ENABLED,
        if settings.enabled { "true" } else { "false" },
    )?;
    db.set_setting(SETTING_PROXY_SERVER, settings.server.trim())?;
    db.set_setting(SETTING_PROXY_PORT, &settings.port.to_string())?;
    db.set_setting(SETTING_PROXY_SECRET, settings.secret.trim())?;
    Ok(())
}

/// Payload returned to the UI (settings + live connection hints).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettingsView {
    #[serde(flatten)]
    pub settings: ProxySettings,
    /// Whether the live ferogram client was built with the proxy.
    pub active: bool,
    /// Last connect/apply error (e.g. proxy unreachable; fell back to direct).
    pub apply_error: Option<String>,
    /// Derived pasteable link when configured.
    pub link: Option<String>,
    /// Whether a live Telegram client is connected.
    pub telegram_online: bool,
}

impl ProxySettingsView {
    pub fn from_parts(
        settings: ProxySettings,
        active: bool,
        apply_error: Option<String>,
        telegram_online: bool,
    ) -> Self {
        let link = if settings.is_configured() {
            Some(settings.to_proxy_link())
        } else {
            None
        };
        Self {
            settings,
            active,
            apply_error,
            link,
            telegram_online,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_db() -> Db {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "soundgrammy-proxy-{}-{nanos}.db",
            std::process::id()
        ));
        Db::open(&path).expect("open test db")
    }

    fn sample_dd() -> ProxySettings {
        ProxySettings {
            enabled: true,
            server: "127.0.0.1".into(),
            port: 1443,
            // dd + 16-byte key (32 hex) — ferogram PaddedIntermediate
            secret: "ddebb7baf7cdd71571bf6ea7a9daf32d29".into(),
        }
    }

    #[test]
    fn from_proxy_link_parses_tg_scheme() {
        let link =
            "tg://proxy?server=127.0.0.1&port=1443&secret=ddebb7baf7cdd71571bf6ea7a9daf32d29";
        let parsed = from_proxy_link(link).expect("parse");
        assert!(parsed.enabled);
        assert_eq!(parsed.server, "127.0.0.1");
        assert_eq!(parsed.port, 1443);
        assert_eq!(parsed.secret, "ddebb7baf7cdd71571bf6ea7a9daf32d29");
    }

    #[test]
    fn from_proxy_link_parses_https_tme() {
        let link =
            "https://t.me/proxy?server=127.0.0.1&port=1443&secret=ddebb7baf7cdd71571bf6ea7a9daf32d29";
        let parsed = from_proxy_link(link).expect("parse");
        assert_eq!(parsed.server, "127.0.0.1");
        assert_eq!(parsed.port, 1443);
        assert_eq!(parsed.secret, "ddebb7baf7cdd71571bf6ea7a9daf32d29");
    }

    #[test]
    fn from_proxy_link_rejects_garbage() {
        assert!(from_proxy_link("not-a-proxy-link").is_err());
        assert!(from_proxy_link("tg://proxy?server=only").is_err());
    }

    #[test]
    fn to_proxy_link_round_trips_with_parse() {
        let settings = sample_dd();
        let link = settings.to_proxy_link();
        let parsed = from_proxy_link(&link).expect("parse");
        assert_eq!(parsed.server, settings.server);
        assert_eq!(parsed.port, settings.port);
        assert_eq!(parsed.secret, settings.secret);
    }

    #[test]
    fn validate_allows_disabled_incomplete() {
        let settings = ProxySettings {
            enabled: false,
            server: String::new(),
            port: 1443,
            secret: String::new(),
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn validate_rejects_enabled_incomplete() {
        let settings = ProxySettings {
            enabled: true,
            server: "127.0.0.1".into(),
            port: 1443,
            secret: String::new(),
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn validate_accepts_dd_secret() {
        assert!(sample_dd().validate().is_ok());
    }

    #[test]
    fn for_connect_requires_enabled_and_configured() {
        let mut settings = sample_dd();
        assert!(settings.for_connect().is_some());

        settings.enabled = false;
        assert!(settings.for_connect().is_none());

        settings.enabled = true;
        settings.secret.clear();
        assert!(settings.for_connect().is_none());
    }

    #[test]
    fn save_and_load_round_trip() {
        let db = test_db();
        let settings = sample_dd();
        save(&db, &settings).expect("save");
        let loaded = load(&db).expect("load");
        assert!(loaded.enabled);
        assert_eq!(loaded.server, "127.0.0.1");
        assert_eq!(loaded.port, 1443);
        assert_eq!(loaded.secret, settings.secret);
    }

    #[test]
    fn load_treats_legacy_one_as_enabled() {
        let db = test_db();
        db.set_setting(SETTING_PROXY_ENABLED, "1").unwrap();
        db.set_setting(SETTING_PROXY_SERVER, "127.0.0.1").unwrap();
        db.set_setting(SETTING_PROXY_PORT, "1443").unwrap();
        db.set_setting(SETTING_PROXY_SECRET, "ddebb7baf7cdd71571bf6ea7a9daf32d29")
            .unwrap();
        let loaded = load(&db).expect("load");
        assert!(loaded.enabled);
    }

    #[test]
    fn load_defaults_when_empty() {
        let db = test_db();
        let loaded = load(&db).expect("load");
        assert!(!loaded.enabled);
        assert!(loaded.server.is_empty());
        assert_eq!(loaded.port, 1443);
        assert!(loaded.secret.is_empty());
    }

    #[test]
    fn view_includes_link_when_configured() {
        let view = ProxySettingsView::from_parts(sample_dd(), true, None, true);
        assert!(view.active);
        assert!(view.telegram_online);
        assert!(view.link.as_ref().is_some_and(|l| l.contains("secret=dd")));
    }

    #[test]
    fn view_omits_link_when_incomplete() {
        let view = ProxySettingsView::from_parts(ProxySettings::default(), false, None, false);
        assert!(view.link.is_none());
    }
}
