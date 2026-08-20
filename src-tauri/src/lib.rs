//! SoundGrammy desktop backend: Tauri builder, state, and command registration.

mod bounce_analysis;
mod cache;
mod commands;
mod config;
mod db;
mod error;
mod export;
mod listen_stats;
mod playlist_recipe;
mod proxy_settings;
mod session;
mod state;
mod streaming;
mod telegram;

use tauri::Manager;

use crate::state::AppState;

/// Builds and runs the Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol("stream", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                responder.respond(streaming::protocol_response(&app, request).await);
            });
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();

            let config = config::Config::load()?;

            let data_dir = handle.path().app_data_dir()?;
            let cache_dir = handle.path().app_cache_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&cache_dir)?;

            let db = db::Db::open(&data_dir.join("library.db"))?;
            app.manage(AppState::new(
                config, db, None, data_dir, cache_dir, false, None,
            ));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let _ = cache::enforce_ttl(&state, &handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::auth_status,
            commands::auth::refresh_auth,
            commands::auth::phone_send_code,
            commands::auth::phone_sign_in,
            commands::auth::phone_check_password,
            commands::auth::qr_start,
            commands::auth::qr_poll,
            commands::auth::qr_check_password,
            commands::auth::logout,
            commands::sync_saved_music,
            commands::list_tracks,
            commands::get_profile,
            commands::sync_status,
            commands::get_track_source,
            commands::get_track_bounce_profile,
            commands::read_stream_range,
            commands::ensure_stream_range,
            commands::download_track,
            commands::prefetch_track,
            commands::cache_track,
            commands::cache_tracks,
            commands::remove_track_from_cache,
            commands::clear_audio_cache,
            commands::get_cache_status,
            commands::get_cache_settings,
            commands::set_cache_settings,
            commands::get_cache_usage,
            commands::export_track,
            commands::export_tracks,
            commands::download_playlist,
            commands::get_track_thumbnail,
            commands::get_user_avatar,
            commands::track_metadata,
            commands::playlists::list_playlists,
            commands::playlists::create_playlist,
            commands::playlists::update_playlist,
            commands::playlists::delete_playlist,
            commands::playlists::add_track_to_playlist,
            commands::playlists::add_tracks_to_playlist,
            commands::playlists::remove_track_from_playlist,
            commands::playlists::reorder_playlist_tracks,
            commands::playlists::toggle_like,
            commands::playlists::export_playlist_json,
            commands::playlists::analyze_playlist_json,
            commands::playlists::import_playlist_json,
            commands::listen_stats::record_listen_start,
            commands::listen_stats::record_listen_end,
            commands::listen_stats::get_listen_statistics_enabled,
            commands::listen_stats::set_listen_statistics_enabled,
            commands::listen_stats::get_track_listen_stats,
            commands::listen_stats::list_listen_stats,
            commands::listen_stats::rebuild_listen_stats,
            commands::listen_stats::clear_listen_statistics,
            commands::get_proxy_settings,
            commands::set_proxy_settings,
            commands::parse_proxy_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SoundGrammy");
}
