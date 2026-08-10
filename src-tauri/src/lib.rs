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
            let proxy = proxy_settings::load(&db)?;
            let want_proxy = proxy.for_connect();

            let (client, proxy_active, proxy_last_error) = tauri::async_runtime::block_on(async {
                match telegram::client::build(&config, &data_dir, want_proxy).await {
                    Ok((client, shutdown)) => {
                        (Some((client, shutdown)), want_proxy.is_some(), None)
                    }
                    Err(err) if want_proxy.is_some() => {
                        match telegram::client::build(&config, &data_dir, None).await {
                            Ok((client, shutdown)) => {
                                (Some((client, shutdown)), false, Some(err.to_string()))
                            }
                            Err(direct_err) => (
                                None,
                                false,
                                Some(format!(
                                    "proxy failed ({err}); direct also failed ({direct_err})"
                                )),
                            ),
                        }
                    }
                    Err(err) => (None, false, Some(err.to_string())),
                }
            });

            app.manage(AppState::new(
                config,
                db,
                client,
                data_dir,
                cache_dir,
                proxy_active,
                proxy_last_error,
            ));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let _ = cache::enforce_ttl(&state, &handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::refresh_auth,
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
            commands::list_playlists,
            commands::create_playlist,
            commands::update_playlist,
            commands::delete_playlist,
            commands::get_playlist_thumbnail,
            commands::add_track_to_playlist,
            commands::add_tracks_to_playlist,
            commands::remove_track_from_playlist,
            commands::reorder_playlist_tracks,
            commands::toggle_like,
            commands::export_playlist_json,
            commands::analyze_playlist_json,
            commands::import_playlist_json,
            commands::record_listen_start,
            commands::record_listen_end,
            commands::get_listen_statistics_enabled,
            commands::set_listen_statistics_enabled,
            commands::get_track_listen_stats,
            commands::list_listen_stats,
            commands::rebuild_listen_stats,
            commands::clear_listen_statistics,
            commands::get_proxy_settings,
            commands::set_proxy_settings,
            commands::parse_proxy_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SoundGrammy");
}
