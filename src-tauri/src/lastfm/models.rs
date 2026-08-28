use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LastFmAuthState {
    UnavailableInBuild,
    Disconnected,
    RequestingToken,
    WaitingForBrowserApproval,
    ExchangingSession,
    Connected,
    NeedsReauthentication,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastFmSafeIssue {
    pub kind: String,
    pub code: Option<i64>,
    pub message: String,
    pub at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastFmStatus {
    pub state: LastFmAuthState,
    pub username: Option<String>,
    pub enabled: bool,
    pub pending_count: i64,
    pub retained_queues: Vec<crate::db::LastFmQueueSummary>,
    pub last_scrobble_at_ms: Option<i64>,
    pub last_error: Option<LastFmSafeIssue>,
    pub last_metadata_warning: Option<LastFmSafeIssue>,
}

#[derive(Debug, Clone)]
pub struct LastFmSession {
    pub username: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct LastFmScrobble {
    pub artist: String,
    pub track: String,
    pub duration: Option<i64>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastFmScrobbleResult {
    pub ignored_code: i64,
}

#[derive(Debug, Clone)]
pub struct PlaybackSnapshot {
    pub attempt_id: String,
    pub username: String,
    pub account_key: String,
    pub track_id: i64,
    pub artist: String,
    pub track_title: String,
    pub duration_seconds: Option<i64>,
    pub started_at_utc: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_status_cannot_serialize_credentials() {
        let status = LastFmStatus {
            state: LastFmAuthState::Connected,
            username: Some("alice".into()),
            enabled: true,
            pending_count: 0,
            retained_queues: Vec::new(),
            last_scrobble_at_ms: None,
            last_error: None,
            last_metadata_warning: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("session"));
        assert!(!json.contains("apiKey"));
        assert!(!json.contains("secret"));
    }
}
