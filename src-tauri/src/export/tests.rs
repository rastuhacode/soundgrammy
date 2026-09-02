use std::fs;

use super::*;
use crate::db::Track;

fn sample_track(id: i64, title: Option<&str>, performer: Option<&str>) -> Track {
    Track {
        id,
        tg_user_id: 1,
        file_id: format!("f{id}"),
        file_unique_id: format!("u{id}"),
        title: title.map(str::to_string),
        title_source: "telegram_audio".into(),
        performer: performer.map(str::to_string),
        duration: Some(120),
        source: "mtproto".into(),
        mime_type: Some("audio/mpeg".into()),
        file_size: Some(1024),
        created_at: "2024-01-01T00:00:00Z".into(),
        mtproto_document: None,
    }
}

#[test]
fn sanitize_replaces_forbidden_filename_chars() {
    assert_eq!(
        sanitize_filename_part("a/b:c*d?e\"f<g>h|i"),
        "a_b_c_d_e_f_g_h_i"
    );
    assert_eq!(sanitize_filename_part("  spaced  "), "spaced");
    assert_eq!(sanitize_filename_part("...dots..."), "dots");
    assert_eq!(sanitize_filename_part("   "), "");
}

#[test]
fn export_basename_prefers_artist_title() {
    let track = sample_track(7, Some("Bangarang"), Some("Skrillex"));
    assert_eq!(track_export_basename(&track), "Skrillex - Bangarang");
}

#[test]
fn export_basename_falls_back_when_metadata_missing() {
    let no_artist = sample_track(3, Some("Solo"), None);
    assert_eq!(track_export_basename(&no_artist), "Solo");

    let empty = sample_track(9, None, None);
    assert_eq!(track_export_basename(&empty), "Track 9");

    let unsafe_chars = sample_track(1, Some("a/b"), Some("x:y"));
    assert_eq!(track_export_basename(&unsafe_chars), "x_y - a_b");
}

#[test]
fn unique_path_appends_numeric_suffix_on_collision() {
    let dir = std::env::temp_dir().join(format!("soundgrammy-export-path-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let first = unique_path(&dir, "Song", "mp3");
    assert_eq!(first.file_name().unwrap(), "Song.mp3");
    fs::write(&first, b"a").unwrap();

    let second = unique_path(&dir, "Song", "mp3");
    assert_eq!(second.file_name().unwrap(), "Song (2).mp3");
    fs::write(&second, b"b").unwrap();

    let third = unique_path(&dir, "Song", "mp3");
    assert_eq!(third.file_name().unwrap(), "Song (3).mp3");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unique_dir_appends_numeric_suffix_on_collision() {
    let parent =
        std::env::temp_dir().join(format!("soundgrammy-export-dir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&parent);
    fs::create_dir_all(&parent).unwrap();

    let first = unique_dir(&parent, "Export 2026-07-22");
    fs::create_dir_all(&first).unwrap();
    let second = unique_dir(&parent, "Export 2026-07-22");
    assert_eq!(second.file_name().unwrap(), "Export 2026-07-22 (2)");

    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn local_date_ymd_is_iso_shaped() {
    let value = local_date_ymd();
    assert!(regex_is_ymd(&value), "expected YYYY-MM-DD, got {value}");
}

fn regex_is_ymd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit)
}

#[test]
fn playlist_folder_basename_sanitizes_and_falls_back() {
    assert_eq!(playlist_folder_basename("Gym"), "Gym");
    assert_eq!(playlist_folder_basename("a/b:c"), "a_b_c");
    assert_eq!(playlist_folder_basename("   "), "Playlist");
    assert_eq!(playlist_folder_basename("..."), "Playlist");
}

#[test]
fn build_m3u8_uses_relative_paths_and_extinf() {
    let body = build_m3u8(&[
        M3uEntry {
            duration_sec: 215,
            display: "Skrillex - Bangarang".into(),
            file_name: "Skrillex - Bangarang.mp3".into(),
        },
        M3uEntry {
            duration_sec: -1,
            display: "Unknown title".into(),
            file_name: "Track 9.mp3".into(),
        },
    ]);
    assert_eq!(
        body,
        "#EXTM3U\n\
         #EXTINF:215,Skrillex - Bangarang\n\
         Skrillex - Bangarang.mp3\n\
         #EXTINF:-1,Unknown title\n\
         Track 9.mp3\n"
    );
}

#[test]
fn extinf_duration_falls_back_to_minus_one() {
    assert_eq!(extinf_duration(Some(120)), 120);
    assert_eq!(extinf_duration(Some(0)), 0);
    assert_eq!(extinf_duration(None), -1);
    assert_eq!(extinf_duration(Some(-5)), -1);
}

#[test]
fn track_display_label_prefers_artist_title() {
    let track = sample_track(1, Some("Bangarang"), Some("Skrillex"));
    assert_eq!(track_display_label(&track), "Skrillex - Bangarang");
    let solo = sample_track(2, Some("Solo"), None);
    assert_eq!(track_display_label(&solo), "Solo");
    let empty = sample_track(3, None, None);
    assert_eq!(track_display_label(&empty), "Unknown title");
}
