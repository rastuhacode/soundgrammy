//! SoundGrammy native services and GPUI application.

mod atomic_file;
pub mod audio;

#[cfg(target_os = "android")]
mod android_keyring;

#[cfg(target_os = "android")]
mod android_downloads;

mod bounce_analysis;
mod cache;
pub mod commands;
mod config;
mod db;
mod display_wake;
mod error;
mod export;
mod lastfm;
mod listen_stats;
mod playlist_recipe;
mod proxy_settings;
mod session;
mod state;
mod streaming;
mod telegram;
mod time;

#[cfg(all(target_os = "android", feature = "mobile"))]
mod mobile;
pub mod runtime;
#[cfg(feature = "ui")]
mod ui;

#[cfg(feature = "desktop")]
pub fn run() {
    ui::run();
}

#[cfg(test)]
mod build_flavor_tests {
    use serde_json::Value;

    #[test]
    fn development_build_uses_an_isolated_app_identity() {
        let release: Value = serde_json::from_str(include_str!("../../native-app.json")).unwrap();
        let development: Value =
            serde_json::from_str(include_str!("../../native-app.dev.json")).unwrap();

        assert_eq!(release["identifier"], "com.soundgrammy.app");
        assert_eq!(development["identifier"], "com.soundgrammy.app.dev");
        assert_ne!(release["identifier"], development["identifier"]);
        assert_eq!(development["productName"], "SoundGrammy Dev");
        #[cfg(debug_assertions)]
        assert_eq!(
            crate::runtime::application_config()["version"],
            format!("{}-develop", release["version"].as_str().unwrap())
        );
    }
}
