//! Less-common repository tasks. Daily development uses Cargo directly.
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .into()
}
fn run(command: &mut Command) -> Result {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{command:?} exited with {status}").into());
    }
    Ok(())
}
fn option(args: &[String], key: &str) -> Result<Option<String>> {
    args.iter()
        .position(|v| v == key)
        .map(|i| {
            args.get(i + 1)
                .filter(|v| !v.starts_with("--"))
                .cloned()
                .ok_or_else(|| format!("{key} requires a value").into())
        })
        .transpose()
}
fn config(debug: bool) -> Result<Value> {
    let mut app: Value = serde_json::from_slice(&fs::read(root().join("native-app.json"))?)?;
    if debug {
        let development: Value =
            serde_json::from_slice(&fs::read(root().join("native-app.dev.json"))?)?;
        for (key, value) in development
            .as_object()
            .ok_or("Invalid development config")?
        {
            app[key] = value.clone();
        }
    }
    Ok(app)
}
fn cargo_build(debug: bool, target: Option<&str>) -> Command {
    cargo_operation("build", debug, target)
}
fn cargo_operation(operation: &str, debug: bool, target: Option<&str>) -> Command {
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .current_dir(root())
        .args([operation, "--locked", "-p", "soundgrammy"]);
    if !debug {
        command.arg("--release");
    }
    if let Some(target) = target {
        command.args(["--target", target]);
    }
    command
}
fn target_dir() -> PathBuf {
    let path = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target"));
    if path.is_absolute() {
        path
    } else {
        root().join(path)
    }
}
fn execute() -> Result {
    let args: Vec<String> = env::args().skip(1).collect();
    let debug = args.iter().any(|v| v == "--debug");
    let target = option(&args, "--target")?;
    match args.first().map(String::as_str).unwrap_or("help") {
        "check-dependencies" => check_dependencies(),
        "check-versions" => check_versions(args.iter().any(|v| v == "--fix")),
        "package" => package(&args, debug, target.as_deref()),
        "android" => android(&args, debug, target.as_deref()),
        "icons" => icons::generate(args.iter().any(|v| v == "--check")),
        "help" | "--help" | "-h" => {
            println!("Daily: cargo run | cargo check | cargo test | cargo fmt | cargo clippy\n\ncargo xtask check-versions [--fix]\ncargo xtask check-dependencies\ncargo xtask package [--debug] [--target TRIPLE] [--no-build] [--formats app,dmg]\ncargo xtask android [--debug] [--target TRIPLE] [--offline]\ncargo xtask icons [--check]\n\nRelease testing: SOUNDGRAMMY_DATA_DIR=/absolute/sandbox cargo run --release\nAccount-free UI: cargo run --release -- --preview");
            Ok(())
        }
        other => Err(format!("Unknown task: {other}; use cargo xtask help").into()),
    }
}
fn check_versions(fix: bool) -> Result {
    let app = config(false)?;
    let version = app["version"]
        .as_str()
        .ok_or("Missing application version")?;
    if let Ok(expected) = env::var("EXPECTED_VERSION") {
        if expected.trim_start_matches('v') != version {
            return Err(format!("Expected {expected}, got {version}").into());
        }
    }
    let manifest = fs::read_to_string(root().join("native/Cargo.toml"))?;
    let cargo_version = manifest
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|s| s.strip_suffix('"'))
        })
        .ok_or("Missing Cargo package version")?;
    let lock = fs::read_to_string(root().join("Cargo.lock"))?;
    if cargo_version != version
        || fs::read_to_string(root().join("version.txt"))?.trim() != version
        || !lock.contains(&format!("name = \"soundgrammy\"\nversion = \"{version}\""))
    {
        return Err("Cargo manifest/lock and native-app.json versions differ".into());
    }
    let path = root().join("native-app.dev.json");
    let mut development: Value = serde_json::from_slice(&fs::read(&path)?)?;
    let expected = format!("{version}-develop");
    if development["version"] != expected {
        if !fix {
            return Err("Development version differs; run cargo xtask check-versions --fix".into());
        }
        development["version"] = json!(expected);
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&development)?),
        )?;
    }
    println!("Application versions synchronized at {version} ({expected}).");
    Ok(())
}
// A tested Kit/renderer pair plus the local fixes that must not silently
// resolve back to an unpatched registry crate during dependency maintenance.
fn locked_package<'a>(lock: &'a str, name: &str) -> Result<&'a str> {
    let expected = format!("name = \"{name}\"");
    lock.split("[[package]]")
        .find(|package| package.lines().any(|line| line == expected))
        .ok_or_else(|| format!("Missing locked package {name}").into())
}
fn check_local_patch(lock: &str, name: &str, version: &str) -> Result {
    let package = locked_package(lock, name)?;
    let expected = format!("version = \"{version}\"");
    if !package.lines().any(|line| line == expected)
        || package.lines().any(|line| line.starts_with("source ="))
    {
        return Err(format!("{name} must resolve to the reviewed local patch at {version}; see docs/dependencies.md").into());
    }
    Ok(())
}
fn check_dependencies() -> Result {
    let lock = fs::read_to_string(root().join("Cargo.lock"))?;
    let (name, version) = ("gpui-pre-mobile", "0.1.0");
    check_local_patch(&lock, name, version)?;
    let directory = root().join("native/vendor").join(name);
    let manifest = fs::read_to_string(directory.join("Cargo.toml"))?;
    if !manifest
        .lines()
        .any(|line| line == format!("version = \"{version}\""))
    {
        return Err(format!("Patch manifest version differs for {name}").into());
    }
    fs::metadata(directory.join("SOUNDGRAMMY-PATCH.md"))?;
    if !locked_package(&lock, "gpui-pre-apple")?
        .lines()
        .any(|line| line == "source = \"registry+https://github.com/rust-lang/crates.io-index\"")
    {
        return Err(
            "The Apple renderer must resolve from crates.io; see docs/dependencies.md".into(),
        );
    }
    for (name, version) in [("gpui-kit", "0.10.0"), ("gpui-pre", "0.3.12")] {
        if !locked_package(&lock, name)?
            .lines()
            .any(|line| line == format!("version = \"{version}\""))
        {
            return Err(format!(
                "Review the Kit/renderer pair before updating {name}; see docs/dependencies.md"
            )
            .into());
        }
    }
    println!("Reviewed GPUI Kit/renderer pair and Android host patch verified.");
    Ok(())
}
fn windows_upgrade_code(name: &str) -> String {
    let dns = [
        0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30,
        0xc8,
    ];
    let mut hash = Sha1::new();
    hash.update(dns);
    hash.update(format!("{name}.exe.app.x64"));
    let mut bytes = hash.finalize();
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes[..16].iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}
fn package(args: &[String], debug: bool, target: Option<&str>) -> Result {
    if !args.iter().any(|v| v == "--no-build") {
        run(cargo_build(debug, target).args(["--bin", "soundgrammy"]))?;
    }
    let profile = if debug { "debug" } else { "release" };
    let platform = match env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let directory = root()
        .join(".build")
        .join(target.unwrap_or(platform))
        .join(profile);
    fs::create_dir_all(&directory)?;
    let mut app = config(debug)?;
    let template = fs::read_to_string(root().join("native/packaging/windows.wxs.hbs"))?;
    let wix = directory.join("windows.wxs");
    fs::write(
        &wix,
        template.replace(
            "{{upgrade_code}}",
            &windows_upgrade_code(app["productName"].as_str().ok_or("Missing product name")?),
        ),
    )?;
    app.as_object_mut().unwrap().remove("window");
    let formats = option(args, "--formats")?.unwrap_or_else(|| {
        match env::consts::OS {
            "macos" => "app,dmg",
            "windows" => "nsis,wix",
            _ => "deb,appimage",
        }
        .into()
    });
    let mut binaries = target_dir();
    if let Some(target) = target {
        binaries.push(target);
    }
    binaries.push(profile);
    let extra = json!({"name":"soundgrammy", "binaries":[{"path":"soundgrammy","main":true}], "binariesDir":binaries, "outDir":root().join("dist").join(target.unwrap_or(platform)).join(profile), "formats":formats.split(',').collect::<Vec<_>>(), "description":"A native music player for your Telegram profile music", "authors":["SoundGrammy"], "category":"Music", "icons":[root().join("native/icons/icon.icns"),root().join("native/icons/icon.ico"),root().join("native/icons/128x128.png")], "licenseFile":root().join("LICENSE"), "macos":{"minimumSystemVersion":"11.0","signingIdentity":"-"}, "wix":{"template":wix}});
    app.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    if let Some(target) = target {
        app["targetTriple"] = json!(target);
    }
    let path = directory.join("packager.json");
    fs::write(&path, serde_json::to_vec_pretty(&app)?)?;
    run(Command::new("cargo")
        .current_dir(root())
        .args(["packager", "--config"])
        .arg(path))
    .map_err(|e| {
        format!(
            "{e}\nInstall packaging once: cargo install cargo-packager --version 0.11.2 --locked"
        )
        .into()
    })
}
fn version_code(version: &str) -> Result<u32> {
    let numbers = version
        .split('-')
        .next()
        .unwrap()
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match numbers.as_slice() {
        [major, minor, patch] if *major <= 2000 && *minor < 100 && *patch < 100 => {
            Ok(major * 1_000_000 + minor * 10_000 + patch * 100)
        }
        _ => Err("Version does not fit Android versionCode".into()),
    }
}
fn android(args: &[String], debug: bool, target: Option<&str>) -> Result {
    let target = target.unwrap_or("aarch64-linux-android");
    let (abi, compiler) = match target {
        "aarch64-linux-android" => ("arm64-v8a", "aarch64-linux-android26"),
        "armv7-linux-androideabi" => ("armeabi-v7a", "armv7a-linux-androideabi26"),
        "x86_64-linux-android" => ("x86_64", "x86_64-linux-android26"),
        "i686-linux-android" => ("x86", "i686-linux-android26"),
        _ => return Err("Unsupported Android target".into()),
    };
    let ndk = env::var_os("NDK_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("ANDROID_HOME")
                .or_else(|| env::var_os("ANDROID_SDK_ROOT"))
                .map(|sdk| PathBuf::from(sdk).join("ndk/29.0.13846066"))
        })
        .ok_or("Set ANDROID_HOME or NDK_HOME")?;
    let host = match env::consts::OS {
        "macos" => "darwin-x86_64",
        "linux" => "linux-x86_64",
        "windows" => "windows-x86_64",
        _ => return Err("Unsupported NDK host".into()),
    };
    let toolchain = ndk.join("toolchains/llvm/prebuilt").join(host).join("bin");
    let clang = toolchain.join(format!(
        "{compiler}-clang{}",
        if cfg!(windows) { ".cmd" } else { "" }
    ));
    let suffix = target.replace('-', "_");
    let mut build = cargo_operation("rustc", debug, Some(target));
    build
        .args([
            "--lib",
            "--crate-type",
            "cdylib",
            "--no-default-features",
            "--features",
            "mobile",
        ])
        .env(
            format!("CARGO_TARGET_{}_LINKER", suffix.to_uppercase()),
            &clang,
        )
        .env(format!("CC_{suffix}"), clang)
        .env(format!("AR_{suffix}"), toolchain.join("llvm-ar"));
    if args.iter().any(|v| v == "--offline") {
        build.arg("--offline");
    }
    run(&mut build)?;
    let android = root().join("native/gen/android");
    let libs = android.join("app/src/main/jniLibs").join(abi);
    fs::create_dir_all(&libs)?;
    let output = libs.join("libsoundgrammy_lib.so");
    fs::copy(
        target_dir()
            .join(target)
            .join(if debug { "debug" } else { "release" })
            .join("libsoundgrammy_lib.so"),
        &output,
    )?;
    run(Command::new(toolchain.join("llvm-strip"))
        .arg("--strip-debug")
        .arg(output))?;
    let app = config(debug)?;
    let version = app["version"].as_str().ok_or("Missing version")?;
    let mut gradle = Command::new(android.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    }));
    gradle
        .current_dir(android)
        .args(if debug {
            [":app:assembleDebug", ":app:testDebugUnitTest"]
        } else {
            [":app:assembleRelease", ":app:testReleaseUnitTest"]
        })
        .arg(format!("-PappVersion={version}"))
        .arg(format!("-PappVersionCode={}", version_code(version)?));
    if args.iter().any(|v| v == "--offline") {
        gradle.arg("--offline");
    }
    run(&mut gradle)
}
mod icons;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dependency_policy_rejects_registry_fallback_and_version_drift() {
        let local = "[[package]]\nname = \"example\"\nversion = \"1.0.0\"\n";
        assert!(check_local_patch(local, "example", "1.0.0").is_ok());
        assert!(check_local_patch(local, "example", "1.0.1").is_err());
        assert!(check_local_patch(
            &(local.to_owned() + "source = \"registry+https://example.com\"\n"),
            "example",
            "1.0.0"
        )
        .is_err());
        assert!(check_local_patch(local, "missing", "1.0.0").is_err());
    }
    #[test]
    fn android_version_contract() {
        assert_eq!(version_code("3.5.0-develop").unwrap(), 3_050_000);
        assert!(version_code("3.100.0").is_err());
        assert!(version_code("broken").is_err());
    }
    #[test]
    fn installer_upgrade_lineage() {
        assert_eq!(
            windows_upgrade_code("SoundGrammy"),
            "44200a38-5e62-517d-96dd-5d09aeb4be5b"
        );
    }
    #[test]
    fn development_identity_and_builds_are_separate_and_locked() {
        assert_eq!(config(false).unwrap()["identifier"], "com.soundgrammy.app");
        assert_eq!(
            config(true).unwrap()["identifier"],
            "com.soundgrammy.app.dev"
        );
        assert_ne!(
            config(false).unwrap()["productName"],
            config(true).unwrap()["productName"]
        );
        let args: Vec<_> = cargo_build(false, Some("aarch64-apple-darwin"))
            .get_args()
            .map(|a| a.to_owned())
            .collect();
        assert!(args.contains(&"--locked".into()));
        assert!(args.contains(&"--release".into()));
        assert!(args.contains(&"aarch64-apple-darwin".into()));
    }
    #[test]
    fn missing_option_value_fails() {
        assert!(option(&["--target".into()], "--target").is_err());
    }
}
