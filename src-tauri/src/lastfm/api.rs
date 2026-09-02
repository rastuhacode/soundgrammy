use std::collections::BTreeMap;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::config::LastFmCredentials;
use crate::lastfm::models::{LastFmScrobble, LastFmScrobbleResult, LastFmSession};

const ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastFmError {
    Transport,
    Api(i64),
    Malformed,
}

impl LastFmError {
    pub fn safe_message(&self) -> &'static str {
        match self {
            Self::Transport => "Last.fm could not be reached.",
            Self::Api(9) => "The Last.fm session is no longer valid.",
            Self::Api(14) => "Authorization is still waiting for approval.",
            Self::Api(15) => "The Last.fm authorization request expired.",
            Self::Api(29) => "Last.fm rate limited requests.",
            Self::Api(11 | 16) => "Last.fm is temporarily unavailable.",
            Self::Api(_) => "Last.fm rejected the request.",
            Self::Malformed => "Last.fm returned an unreadable response.",
        }
    }
}

#[async_trait]
pub trait LastFmTransport: Send + Sync {
    async fn post(&self, params: &BTreeMap<String, String>) -> Result<Value, LastFmError>;
}

pub struct HttpTransport {
    client: reqwest::Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, LastFmError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .https_only(true)
            .build()
            .map_err(|_| LastFmError::Transport)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl LastFmTransport for HttpTransport {
    async fn post(&self, params: &BTreeMap<String, String>) -> Result<Value, LastFmError> {
        let response = self
            .client
            .post(ENDPOINT)
            .form(params)
            .send()
            .await
            .map_err(|_| LastFmError::Transport)?;
        response
            .json::<Value>()
            .await
            .map_err(|_| LastFmError::Malformed)
    }
}

pub struct LastFmClient {
    credentials: LastFmCredentials,
    transport: Box<dyn LastFmTransport>,
}

impl LastFmClient {
    pub fn production(credentials: LastFmCredentials) -> Result<Self, LastFmError> {
        Ok(Self {
            credentials,
            transport: Box::new(HttpTransport::new()?),
        })
    }

    #[cfg(test)]
    pub fn with_transport(
        credentials: LastFmCredentials,
        transport: Box<dyn LastFmTransport>,
    ) -> Self {
        Self {
            credentials,
            transport,
        }
    }

    async fn call(
        &self,
        method: &str,
        mut params: BTreeMap<String, String>,
    ) -> Result<Value, LastFmError> {
        params.insert("api_key".into(), self.credentials.api_key.clone());
        params.insert("method".into(), method.into());
        let signature = sign_params(&params, &self.credentials.api_secret);
        params.insert("api_sig".into(), signature);
        params.insert("format".into(), "json".into());
        let value = self.transport.post(&params).await?;
        if let Some(code) = value.get("error").and_then(|code| {
            code.as_i64()
                .or_else(|| code.as_str().and_then(|raw| raw.parse().ok()))
        }) {
            return Err(LastFmError::Api(code));
        }
        Ok(value)
    }

    pub async fn get_token(&self) -> Result<String, LastFmError> {
        self.call("auth.getToken", BTreeMap::new())
            .await?
            .get("token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .ok_or(LastFmError::Malformed)
    }

    pub async fn get_session(&self, token: &str) -> Result<LastFmSession, LastFmError> {
        let value = self
            .call(
                "auth.getSession",
                BTreeMap::from([("token".into(), token.into())]),
            )
            .await?;
        let session = value.get("session").ok_or(LastFmError::Malformed)?;
        let username = session
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or(LastFmError::Malformed)?;
        let key = session
            .get("key")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or(LastFmError::Malformed)?;
        Ok(LastFmSession {
            username: username.to_owned(),
            key: key.to_owned(),
        })
    }

    pub async fn verify_user(&self, session_key: &str) -> Result<String, LastFmError> {
        let value = self
            .call(
                "user.getInfo",
                BTreeMap::from([("sk".into(), session_key.into())]),
            )
            .await?;
        value
            .get("user")
            .and_then(|user| user.get("name"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .ok_or(LastFmError::Malformed)
    }

    pub async fn update_now_playing(
        &self,
        session_key: &str,
        artist: &str,
        track: &str,
        duration: Option<i64>,
    ) -> Result<i64, LastFmError> {
        let mut params = BTreeMap::from([
            ("artist".into(), artist.into()),
            ("track".into(), track.into()),
            ("sk".into(), session_key.into()),
        ]);
        if let Some(duration) = duration.filter(|value| *value > 0) {
            params.insert("duration".into(), duration.to_string());
        }
        let value = self.call("track.updateNowPlaying", params).await?;
        Ok(ignored_code(
            value
                .get("nowplaying")
                .and_then(|item| item.get("ignoredMessage")),
        ))
    }

    pub async fn scrobble(
        &self,
        session_key: &str,
        rows: &[LastFmScrobble],
    ) -> Result<Vec<LastFmScrobbleResult>, LastFmError> {
        let mut params = BTreeMap::from([("sk".into(), session_key.into())]);
        for (index, row) in rows.iter().enumerate() {
            params.insert(format!("artist[{index}]"), row.artist.clone());
            params.insert(format!("track[{index}]"), row.track.clone());
            params.insert(format!("timestamp[{index}]"), row.timestamp.to_string());
            if let Some(duration) = row.duration.filter(|value| *value > 0) {
                params.insert(format!("duration[{index}]"), duration.to_string());
            }
        }
        let value = self.call("track.scrobble", params).await?;
        let scrobbles = value
            .get("scrobbles")
            .and_then(|value| value.get("scrobble"))
            .ok_or(LastFmError::Malformed)?;
        let items: Vec<&Value> = match scrobbles {
            Value::Array(items) => items.iter().collect(),
            Value::Object(_) => vec![scrobbles],
            _ => return Err(LastFmError::Malformed),
        };
        if items.len() != rows.len() {
            return Err(LastFmError::Malformed);
        }
        Ok(items
            .into_iter()
            .map(|item| LastFmScrobbleResult {
                ignored_code: ignored_code(item.get("ignoredMessage")),
            })
            .collect())
    }

    pub fn authorization_url(&self, token: &str) -> Result<String, LastFmError> {
        let mut url =
            url::Url::parse("https://www.last.fm/api/auth/").map_err(|_| LastFmError::Malformed)?;
        url.query_pairs_mut()
            .append_pair("api_key", &self.credentials.api_key)
            .append_pair("token", token);
        Ok(url.into())
    }
}

pub fn sign_params(params: &BTreeMap<String, String>, secret: &str) -> String {
    let mut material = String::new();
    for (name, value) in params {
        if matches!(name.as_str(), "format" | "callback" | "api_sig") {
            continue;
        }
        material.push_str(name);
        material.push_str(value);
    }
    material.push_str(secret);
    format!("{:x}", md5::compute(material.as_bytes()))
}

fn ignored_code(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| value.get("code"))
        .and_then(|code| {
            code.as_i64()
                .or_else(|| code.as_str().and_then(|raw| raw.parse().ok()))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockTransport {
        response: Value,
        seen: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    }

    #[async_trait]
    impl LastFmTransport for MockTransport {
        async fn post(&self, params: &BTreeMap<String, String>) -> Result<Value, LastFmError> {
            self.seen.lock().unwrap().push(params.clone());
            Ok(self.response.clone())
        }
    }

    fn credentials() -> LastFmCredentials {
        LastFmCredentials {
            api_key: "key".into(),
            api_secret: "secret".into(),
        }
    }

    #[test]
    fn signing_is_sorted_and_omits_response_only_fields() {
        let params = BTreeMap::from([
            ("track[10]".into(), "ten".into()),
            ("artist[1]".into(), "one".into()),
            ("format".into(), "json".into()),
            ("callback".into(), "ignored".into()),
        ]);
        let expected = format!(
            "{:x}",
            md5::compute("artist[1]onetrack[10]tensecret".as_bytes())
        );
        assert_eq!(sign_params(&params, "secret"), expected);
    }

    #[tokio::test]
    async fn batch_uses_numeric_indices_without_losing_lexical_signing_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let response_items = (0..11)
            .map(|_| serde_json::json!({"ignoredMessage": {"code": "0", "#text": ""}}))
            .collect::<Vec<_>>();
        let client = LastFmClient::with_transport(
            credentials(),
            Box::new(MockTransport {
                response: serde_json::json!({"scrobbles": {"scrobble": response_items}}),
                seen: seen.clone(),
            }),
        );
        let rows = (0..11)
            .map(|index| LastFmScrobble {
                artist: format!("Исполнитель {index}"),
                track: format!("Track {index}"),
                duration: Some(60),
                timestamp: 100 + index,
            })
            .collect::<Vec<_>>();
        assert_eq!(client.scrobble("session", &rows).await.unwrap().len(), 11);
        let request = seen.lock().unwrap();
        assert_eq!(request[0].get("artist[1]").unwrap(), "Исполнитель 1");
        assert_eq!(request[0].get("track[10]").unwrap(), "Track 10");
        assert_eq!(request[0].get("format").unwrap(), "json");
        assert!(!request[0].get("api_sig").unwrap().contains("secret"));
    }

    #[tokio::test]
    async fn parses_top_level_errors_without_exposing_server_text() {
        let client = LastFmClient::with_transport(
            credentials(),
            Box::new(MockTransport {
                response: serde_json::json!({"error": "9", "message": "secret server detail"}),
                seen: Arc::new(Mutex::new(Vec::new())),
            }),
        );
        let error = client.get_token().await.unwrap_err();
        assert_eq!(error, LastFmError::Api(9));
        assert!(!error.safe_message().contains("server detail"));
    }

    #[tokio::test]
    async fn parses_mixed_accepted_and_filtered_batch_items() {
        let client = LastFmClient::with_transport(
            credentials(),
            Box::new(MockTransport {
                response: serde_json::json!({
                    "scrobbles": {"scrobble": [
                        {"ignoredMessage": {"code": "0"}},
                        {"ignoredMessage": {"code": "2"}},
                        {"ignoredMessage": {"code": "5"}}
                    ]}
                }),
                seen: Arc::new(Mutex::new(Vec::new())),
            }),
        );
        let rows = (0..3)
            .map(|index| LastFmScrobble {
                artist: "Artist".into(),
                track: format!("Track {index}"),
                duration: Some(60),
                timestamp: 100 + index,
            })
            .collect::<Vec<_>>();
        let result = client.scrobble("session", &rows).await.unwrap();
        assert_eq!(
            result
                .iter()
                .map(|item| item.ignored_code)
                .collect::<Vec<_>>(),
            vec![0, 2, 5]
        );
    }
}
