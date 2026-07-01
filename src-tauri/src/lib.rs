//! SoundGrammy desktop backend: Tauri builder, state, and command registration.

mod cache;
mod commands;
mod config;
mod db;
mod error;
mod session;
mod state;
mod telegram;

use tauri::Manager;

use crate::state::AppState;

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();

            let config = config::Config::load()?;

            let data_dir = handle.path().app_data_dir()?;
            let cache_dir = handle.path().app_cache_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&cache_dir)?;

            let db = db::Db::open(&data_dir.join("library.db"))?;
            let (session, client) = telegram::client::build(&config, &data_dir)?;

            app.manage(AppState {
                config,
                db,
                session,
                client,
                data_dir,
                cache_dir,
                pending: Default::default(),
                download_locks: Default::default(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::phone_send_code,
            commands::phone_sign_in,
            commands::phone_check_password,
            commands::qr_start,
            commands::qr_poll,
            commands::qr_check_password,
            commands::logout,
            commands::sync_saved_music,
            commands::list_tracks,
            commands::get_profile,
            commands::sync_status,
            commands::get_track_source,
            commands::prefetch_track,
            commands::get_track_thumbnail,
            commands::get_user_avatar,
            commands::track_metadata,
            commands::list_playlists,
            commands::create_playlist,
            commands::update_playlist,
            commands::delete_playlist,
            commands::get_playlist_thumbnail,
            commands::add_track_to_playlist,
            commands::remove_track_from_playlist,
            commands::toggle_like,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SoundGrammy");
}
