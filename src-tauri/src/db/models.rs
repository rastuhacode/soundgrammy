use serde::Serialize;

/// A track as sent to the frontend. Field names are snake_case to match the
/// ported Zustand stores/components.
#[derive(Debug, Clone, Serialize)]
pub struct Track {
    pub id: i64,
    pub tg_user_id: i64,
    pub file_id: String,
    pub file_unique_id: String,
    pub title: Option<String>,
    #[serde(skip)]
    pub title_source: String,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    pub source: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: String,
    /// The serialized Telegram document JSON stays server-side only.
    #[serde(skip)]
    pub mtproto_document: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LikedPlaylist {
    pub id: i64,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomPlaylistSummary {
    pub id: i64,
    pub name: String,
    #[serde(rename = "trackIds")]
    pub track_ids: Vec<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistsBundle {
    pub liked: LikedPlaylist,
    pub custom: Vec<CustomPlaylistSummary>,
}

/// Fields needed to upsert a track discovered during saved-music sync.
pub struct UpsertTrack {
    pub tg_user_id: i64,
    pub file_id: String,
    pub file_unique_id: String,
    pub title: Option<String>,
    pub title_source: String,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub track_position: i64,
    pub mtproto_document: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackListenStats {
    pub track_id: i64,
    pub starts: i64,
    pub qualified_plays: i64,
    pub completes: i64,
    pub early_skips: i64,
    pub total_listened_ms: i64,
    pub first_played_at_ms: Option<i64>,
    pub last_played_at_ms: Option<i64>,
    pub likeness: f64,
}

#[derive(Debug, Clone)]
pub struct TrackBounceProfileRecord {
    pub track_id: i64,
    pub algorithm_version: i64,
    pub frame_ms: i64,
    pub duration_ms: i64,
    pub file_size: Option<i64>,
    pub loudness: Vec<u8>,
    pub onset: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListenEndResult {
    pub qualified: bool,
    pub early_skip: bool,
    pub listened_eff_ms: i64,
    pub stats: TrackListenStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    #[serde(rename = "tgUserId")]
    pub tg_user_id: i64,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}
