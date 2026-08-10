use super::progress::{loaded_ranges, LoadedRange};
use super::*;

#[test]
fn merges_adjacent_ready_chunks_into_byte_ranges() {
    let mut chunks: Vec<ChunkSlot> = (0..5).map(|_| ChunkSlot::default()).collect();
    chunks[0].status = ChunkStatus::Ready;
    chunks[1].status = ChunkStatus::Ready;
    chunks[3].status = ChunkStatus::Ready;

    assert_eq!(
        loaded_ranges(&chunks, 5 * CHUNK_SIZE),
        vec![
            LoadedRange {
                start: 0,
                end: 2 * CHUNK_SIZE,
            },
            LoadedRange {
                start: 3 * CHUNK_SIZE,
                end: 4 * CHUNK_SIZE,
            },
        ]
    );
}

#[test]
fn clamps_the_last_loaded_range_to_file_size() {
    let mut chunks: Vec<ChunkSlot> = (0..2).map(|_| ChunkSlot::default()).collect();
    chunks[1].status = ChunkStatus::Ready;
    let total = CHUNK_SIZE + 17;

    assert_eq!(
        loaded_ranges(&chunks, total),
        vec![LoadedRange {
            start: CHUNK_SIZE,
            end: total,
        }]
    );
}

#[test]
fn resolve_read_path_prefers_final_then_destination_then_partial() {
    let dir = std::env::temp_dir().join(format!("soundgrammy-stream-path-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let destination = dir.join("track.mp3");
    let partial = partial_path(&destination);
    let finalized = dir.join("track.webm");

    std::fs::write(&partial, b"partial").unwrap();
    assert_eq!(
        resolve_stream_read_path(None, &destination, &partial),
        partial
    );

    std::fs::write(&destination, b"dest").unwrap();
    assert_eq!(
        resolve_stream_read_path(None, &destination, &partial),
        destination
    );

    assert_eq!(
        resolve_stream_read_path(Some(finalized.clone()), &destination, &partial),
        finalized
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn read_file_range_succeeds_after_partial_renamed_to_destination() {
    let dir =
        std::env::temp_dir().join(format!("soundgrammy-stream-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let destination = dir.join("track.mp3");
    let partial = partial_path(&destination);
    let payload = b"abcdefghijklmnopqrstuvwxyz";
    std::fs::write(&partial, payload).unwrap();

    // Simulate the MSE race: path was snapshotted as .part, then finalize
    // renamed it before open. Retry against the destination must succeed.
    std::fs::rename(&partial, &destination).unwrap();
    assert!(!partial.exists());

    let bytes = read_file_range(&destination, 0, 4).await.unwrap();
    assert_eq!(bytes, b"abcde");

    let stale = read_file_range(&partial, 0, 4).await;
    assert!(stale.is_err());
    let retry = resolve_stream_read_path(None, &destination, &partial);
    let recovered = read_file_range(&retry, 0, 4).await.unwrap();
    assert_eq!(recovered, b"abcde");

    let _ = std::fs::remove_dir_all(&dir);
}
