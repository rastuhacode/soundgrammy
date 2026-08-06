use std::path::PathBuf;

fn main() {
    // CI: secrets already in the environment → leave them alone.
    // Local: fall back to gitignored .env.local next to Cargo.toml.
    let env_local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.local");

    if std::env::var_os("TELEGRAM_API_ID").is_none()
        || std::env::var_os("TELEGRAM_API_HASH").is_none()
    {
        let _ = dotenvy::from_path(&env_local);
    }

    // Expose to option_env! / env! in the main crate (build-script env alone is not enough).
    for key in ["TELEGRAM_API_ID", "TELEGRAM_API_HASH"] {
        match std::env::var(key) {
            Ok(value) if !value.trim().is_empty() => {
                println!("cargo:rustc-env={key}={value}");
            }
            _ => {
                panic!("{key} is not set. Add src-tauri/.env.local or export it (CI secrets).");
            }
        }
    }

    println!("cargo:rerun-if-env-changed=TELEGRAM_API_ID");
    println!("cargo:rerun-if-env-changed=TELEGRAM_API_HASH");
    println!("cargo:rerun-if-changed=.env.local");

    tauri_build::build()
}
