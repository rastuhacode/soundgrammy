//! Publish a fully written file while preserving the previous artifact on failure.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, bytes, |temp, destination| {
        fs::rename(temp, destination)
    })
}
fn write_with(
    path: &Path,
    bytes: &[u8],
    publish: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("File has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".soundgrammy-{}-{:016x}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        publish(&temp, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_publication_preserves_previous_file_and_cleans_temp() {
        let dir =
            std::env::temp_dir().join(format!("soundgrammy-atomic-{:016x}", rand::random::<u64>()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.enc");
        fs::write(&path, b"old").unwrap();
        assert!(write_with(&path, b"new", |_, _| Err(io::Error::other(
            "injected failure"
        )))
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        fs::remove_dir_all(dir).unwrap();
    }
}
