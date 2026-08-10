use std::path::{Path, PathBuf};

pub(super) fn partial_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_owned();
    value.push(".part");
    PathBuf::from(value)
}

pub(super) fn complete_path(destination: &Path) -> PathBuf {
    let mut value = destination.as_os_str().to_owned();
    value.push(".complete");
    PathBuf::from(value)
}

/// Prefer the post-finalize path, then the destination, then the sparse partial.
pub(super) fn resolve_stream_read_path(
    final_path: Option<PathBuf>,
    destination: &Path,
    partial: &Path,
) -> PathBuf {
    if let Some(path) = final_path {
        return path;
    }
    if destination.exists() {
        return destination.to_path_buf();
    }
    partial.to_path_buf()
}
