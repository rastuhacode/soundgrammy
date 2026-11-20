use std::path::PathBuf;

fn main() {
    // CI: secrets already in the environment → leave them alone.
    // Local: fall back to gitignored .env.local next to Cargo.toml.
    let env_local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");

    if [
        "TELEGRAM_API_ID",
        "TELEGRAM_API_HASH",
        "LASTFM_API_KEY",
        "LASTFM_API_SECRET",
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_none())
    {
        let _ = dotenvy::from_path(&env_local);
    }

    // Expose to option_env! / env! in the main crate (build-script env alone is not enough).
    for key in ["TELEGRAM_API_ID", "TELEGRAM_API_HASH"] {
        match std::env::var(key) {
            Ok(value) if !value.trim().is_empty() => {
                println!("cargo:rustc-env={key}={value}");
            }
            _ if std::env::var("PROFILE").as_deref() == Ok("debug") => {
                // Account-free checks and previews need no Telegram application.
                // A real launch still validates credentials in Config::load.
            }
            _ => panic!("{key} is not set. Add native/.env.local or export it (CI secrets)."),
        }
    }

    // Last.fm is optional for local/self builds. Embed only complete pairs;
    // runtime values can still override them in debug/development builds.
    let lastfm_key = std::env::var("LASTFM_API_KEY").ok();
    let lastfm_secret = std::env::var("LASTFM_API_SECRET").ok();
    if let (Some(key), Some(secret)) = (lastfm_key, lastfm_secret) {
        if !key.trim().is_empty() && !secret.trim().is_empty() {
            println!("cargo:rustc-env=LASTFM_API_KEY={key}");
            println!("cargo:rustc-env=LASTFM_API_SECRET={secret}");
        }
    }

    println!("cargo:rerun-if-env-changed=TELEGRAM_API_ID");
    println!("cargo:rerun-if-env-changed=TELEGRAM_API_HASH");
    println!("cargo:rerun-if-env-changed=LASTFM_API_KEY");
    println!("cargo:rerun-if-env-changed=LASTFM_API_SECRET");
    println!("cargo:rerun-if-changed=.env.local");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rerun-if-changed=icons/icon.ico");
        winresource::WindowsResource::new()
            .set_icon("icons/icon.ico")
            .set("ProductName", "SoundGrammy")
            .set("FileDescription", "SoundGrammy")
            .compile_for("soundgrammy")
            .expect("compile SoundGrammy executable resources");
    }
}
