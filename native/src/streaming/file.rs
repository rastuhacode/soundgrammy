use crate::error::AppResult;
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn read_file_range(path: &Path, start: u64, end: u64) -> AppResult<Vec<u8>> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = vec![0; (end - start + 1) as usize];
    file.read_exact(&mut bytes).await?;
    Ok(bytes)
}
