//! One-time import of UI settings from the previous WebView. Source storage is
//! only read; Chromium recovery runs against a temporary copy, never user data.
use super::model::{BounceSettings, PlaylistId, Preferences};
use crate::state::AppState;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
type Values = HashMap<String, String>;
fn decode_utf16(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect::<Vec<_>>(),
    )
    .ok()
}
fn chromium_string(bytes: &[u8]) -> Option<String> {
    match bytes.split_first()? {
        (0, rest) => decode_utf16(rest),
        (1, rest) => Some(rest.iter().map(|b| char::from(*b)).collect()),
        _ => None,
    }
}
fn known(key: &str) -> bool {
    key.starts_with("soundgrammy")
        || key == "react-resizable-panels:player-layout:playlist-sidebar:track-list"
}
fn sqlite(path: &Path, values: &mut Values) {
    let Ok(db) = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return;
    };
    // WebKit uses UTF-16 blobs; older WebKitGTK uses text.
    for query in [
        "SELECT key,value FROM ItemTable",
        "SELECT key,value FROM localStorage",
    ] {
        let Ok(mut statement) = db.prepare(query) else {
            continue;
        };
        let Ok(rows) = statement.query_map([], |row| {
            let key: String = row.get(0)?;
            let value = match row.get_ref(1)? {
                rusqlite::types::ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
                rusqlite::types::ValueRef::Blob(v) => decode_utf16(v).unwrap_or_default(),
                _ => String::new(),
            };
            Ok((key, value))
        }) else {
            continue;
        };
        for (key, value) in rows.flatten() {
            if known(&key) {
                values.insert(key, value);
            }
        }
    }
}
fn leveldb(path: &Path, values: &mut Values) {
    use rusty_leveldb::{LdbIterator, Options, DB};
    let Ok(copy) = tempfile::tempdir() else {
        return;
    };
    let Ok(files) = std::fs::read_dir(path) else {
        return;
    };
    for file in files.flatten() {
        let name = file.file_name();
        if file.path().is_file()
            && name != "LOCK"
            && std::fs::copy(file.path(), copy.path().join(name)).is_err()
        {
            return;
        }
    }
    let options = Options {
        create_if_missing: false,
        ..Default::default()
    };
    let Ok(mut db) = DB::open(copy.path(), options) else {
        return;
    };
    let Ok(mut iter) = db.new_iter() else { return };
    while let Some((key, value)) = iter.next() {
        // Chromium: '_' + origin + NUL + encoded DOM-string key.
        if key.first() != Some(&b'_') {
            continue;
        }
        let Some(split) = key.iter().position(|v| *v == 0) else {
            continue;
        };
        let origin = String::from_utf8_lossy(&key[1..split]);
        if !matches!(
            origin.as_ref(),
            "tauri://localhost"
                | "http://tauri.localhost"
                | "https://tauri.localhost"
                | "http://localhost:1420"
        ) {
            continue;
        }
        if let (Some(key), Some(value)) =
            (chromium_string(&key[split + 1..]), chromium_string(&value))
        {
            if known(&key) {
                values.insert(key, value);
            }
        }
    }
}
fn scan(path: &Path, depth: usize, values: &mut Values) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if entry.file_name() == "leveldb" {
                leveldb(&path, values);
            } else {
                scan(&path, depth - 1, values);
            }
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("localstorage" | "sqlite3")
        ) {
            sqlite(&path, values);
        }
    }
}
fn roots(state: &AppState) -> Vec<PathBuf> {
    #[allow(unused_mut)] // Desktop adds its platform WebView storage root.
    let mut roots = vec![
        state.data_dir.join("EBWebView"),
        state.data_dir.join("app_webview"),
        state.data_dir.join("localstorage"),
    ];
    #[cfg(target_os = "macos")]
    if let Some(home) = dirs::home_dir() {
        if let Some(id) = state.data_dir.file_name() {
            roots.push(
                home.join("Library/WebKit")
                    .join(id)
                    .join("WebsiteData/Default"),
            );
        }
    }
    #[cfg(target_os = "windows")]
    if let (Some(local), Some(id)) = (dirs::data_local_dir(), state.data_dir.file_name()) {
        roots.push(local.join(id).join("EBWebView"));
    }
    roots
}
pub(super) fn import(host: &crate::runtime::AppHost) -> Preferences {
    let state = host.state::<AppState>();
    let mut values = Values::new();
    for root in roots(&state) {
        scan(&root, 8, &mut values);
    }
    let result = from_values(&values);
    if let Some(raw) = values
        .get("soundgrammy-diagnostic-logs")
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
    {
        if let Some(entries) = raw.get("entries").and_then(Value::as_array) {
            let lines = entries
                .iter()
                .filter_map(|e| {
                    Some(format!(
                        "[{}] {} {} — {}\n{}{}",
                        e.get("timestamp")?.as_str()?,
                        e.get("level")?.as_str()?.to_uppercase(),
                        e.get("source")?.as_str()?,
                        e.get("title")?.as_str()?,
                        e.get("description")?.as_str()?,
                        e.get("details")
                            .and_then(Value::as_str)
                            .map(|s| format!("\n{s}"))
                            .unwrap_or_default()
                    ))
                })
                .collect::<Vec<_>>();
            let _ = state.db.set_setting(
                "native_diagnostic_logs",
                &serde_json::to_string(&lines).unwrap_or_default(),
            );
        }
    }
    result
}
fn json(values: &Values, key: &str) -> Option<Value> {
    values.get(key).and_then(|v| serde_json::from_str(v).ok())
}
fn from_values(values: &Values) -> Preferences {
    let mut p = Preferences::default();
    if let Some(value) = values.get("soundgrammy:selectedPlaylistId") {
        p.selected = PlaylistId::parse(value.trim_matches('"')).key();
    }
    if let Some(list) =
        json(values, "soundgrammy:hiddenPlaylists").and_then(|v| v.as_array().cloned())
    {
        p.hidden = list
            .iter()
            .filter_map(Value::as_str)
            .filter(|v| matches!(*v, "liked" | "popular" | "recent"))
            .map(String::from)
            .collect();
    }
    if let Some(value) = values
        .get("soundgrammy:playlistSortMode")
        .filter(|v| matches!(v.as_str(), "recency" | "custom" | "alphabetical"))
    {
        p.playlist_sort = value.clone();
    }
    p.reversed = values
        .get("soundgrammy:playlistSortReversed")
        .is_some_and(|v| v == "true");
    if let Some(list) =
        json(values, "soundgrammy:playlistCustomOrder").and_then(|v| v.as_array().cloned())
    {
        p.custom_order = list
            .iter()
            .filter_map(|v| {
                v.as_str()
                    .map(String::from)
                    .or_else(|| v.as_i64().filter(|v| *v > 0).map(|v| v.to_string()))
            })
            .map(|v| PlaylistId::parse(&v).key())
            .collect();
    }
    p.keep_awake = values
        .get("soundgrammy-fullscreen-keep-display-awake-v1")
        .is_none_or(|v| v != "false");
    if let Some(value) = json(values, "soundgrammy-fullscreen-bounce-v1") {
        p.bounce = BounceSettings {
            enabled: value
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
    }
    let key = if cfg!(target_os = "android") {
        "soundgrammy-volume-android"
    } else {
        "soundgrammy-volume"
    };
    if let Some(v) = json(values, key)
        .and_then(|v| v.as_f64())
        .filter(|v| v.is_finite() && (0. ..=100.).contains(v))
    {
        p.volume = v.round();
    }
    for (key, target) in [("soundgrammy-repeat", &mut p.repeat)] {
        if let Some(v) = json(values, key)
            .or_else(|| values.get(key).map(|v| Value::String(v.clone())))
            .and_then(|v| serde_json::from_value(v).ok())
        {
            *target = v;
        }
    }
    if let Some(v) = json(values, "soundgrammy-shuffle")
        .or_else(|| {
            values
                .get("soundgrammy-shuffle")
                .map(|v| Value::String(v.clone()))
        })
        .and_then(|v| serde_json::from_value(v).ok())
    {
        p.shuffle = v;
    }
    if let Some(v) = json(values, "soundgrammy-shuffle-mode")
        .or_else(|| {
            values
                .get("soundgrammy-shuffle-mode")
                .map(|s| Value::String(s.clone()))
        })
        .and_then(|v| serde_json::from_value(v).ok())
    {
        p.shuffle_mode = v;
    }
    if let Some(v) = json(values, "soundgrammy-diagnostic-logs") {
        p.logs_enabled = v.get("enabled") == Some(&Value::Bool(true));
    }
    if let Some(v) = json(values, "soundgrammy-queue-sections") {
        p.history_open = v
            .get("historyOpen")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        p.up_next_open = v.get("upNextOpen").and_then(Value::as_bool).unwrap_or(true);
    }
    if let Some(value) = json(
        values,
        "react-resizable-panels:player-layout:playlist-sidebar:track-list",
    ) {
        let width = value
            .get("playlist-sidebar")
            .and_then(Value::as_f64)
            .or_else(|| {
                value
                    .as_array()
                    .and_then(|v| v.first())
                    .and_then(Value::as_f64)
            });
        if let Some(width) = width.filter(|v| v.is_finite() && *v > 0. && *v < 100.) {
            p.sidebar_width = (1200. * width as f32 / 100.).clamp(220., 480.);
        }
    }
    p
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn valid_preferences_survive_import_with_validation() {
        let mut values = Values::from([
            ("soundgrammy:selectedPlaylistId".into(), "42".into()),
            (
                "soundgrammy:hiddenPlaylists".into(),
                "[\"all\",\"liked\",\"unknown\"]".into(),
            ),
            (
                "soundgrammy-fullscreen-bounce-v1".into(),
                "{\"enabled\":true,\"strength\":200,\"balance\":-2}".into(),
            ),
            (
                "soundgrammy:playlistCustomOrder".into(),
                "[42,\"all\",\"liked\"]".into(),
            ),
        ]);
        values.insert(
            if cfg!(target_os = "android") {
                "soundgrammy-volume-android"
            } else {
                "soundgrammy-volume"
            }
            .into(),
            "37.5".into(),
        );
        let p = from_values(&values);
        assert_eq!(p.selected, "42");
        assert_eq!(p.volume, 38.);
        assert_eq!(p.hidden.len(), 1);
        assert!(p.bounce.enabled);
        assert_eq!(p.custom_order, vec!["42", "all", "liked"]);
    }
    #[test]
    fn webkit_utf16_and_chromium_latin1_and_utf16_decode() {
        let bytes = "Art 🎵"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode_utf16(&bytes).as_deref(), Some("Art 🎵"));
        assert_eq!(chromium_string(&[1, b'A', 0xe9]).as_deref(), Some("Aé"));
        let mut encoded = vec![0];
        encoded.extend(bytes);
        assert_eq!(chromium_string(&encoded).as_deref(), Some("Art 🎵"));
        assert!(chromium_string(&[0, 1]).is_none());
    }
    #[test]
    fn sqlite_import_never_changes_source_storage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("localstorage.sqlite3");
        {
            let db = rusqlite::Connection::open(&path).unwrap();
            db.execute_batch("CREATE TABLE ItemTable(key TEXT,value BLOB)")
                .unwrap();
            let value = "42"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            db.execute(
                "INSERT INTO ItemTable VALUES(?1,?2)",
                rusqlite::params!["soundgrammy:selectedPlaylistId", value],
            )
            .unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let mut values = Values::new();
        sqlite(&path, &mut values);
        assert_eq!(values["soundgrammy:selectedPlaylistId"], "42");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    #[test]
    fn chromium_import_recovers_only_the_app_origin_and_leaves_source_unchanged() {
        use rusty_leveldb::{Options, DB};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("leveldb");
        {
            let mut db = DB::open(&path, Options::default()).unwrap();
            db.put(
                b"_http://tauri.localhost\0\x01soundgrammy:selectedPlaylistId",
                b"\x0142",
            )
            .unwrap();
            db.put(
                b"_https://unrelated.example\0\x01soundgrammy-volume",
                b"\x0199",
            )
            .unwrap();
            db.flush().unwrap();
        }
        let before = std::fs::read(path.join("CURRENT")).unwrap();
        let mut values = Values::new();
        leveldb(&path, &mut values);
        assert_eq!(values["soundgrammy:selectedPlaylistId"], "42");
        assert!(!values.contains_key("soundgrammy-volume"));
        assert_eq!(std::fs::read(path.join("CURRENT")).unwrap(), before);
    }
}
