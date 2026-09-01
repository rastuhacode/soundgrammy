use super::paths::{partial_metadata_path, partial_path, resolve_stream_read_path};
use super::progress::{loaded_ranges, ChunkSlot, ChunkStatus, LoadedRange};
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

#[tokio::test]
async fn partial_manifest_reports_only_durable_ready_chunks() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "soundgrammy-partial-info-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let partial = dir.join("track.mp3.part");
    let total = CHUNK_SIZE + 17;
    let file = std::fs::File::create(&partial).unwrap();
    file.set_len(total).unwrap();

    let manifest = super::partial_cache::PartialMetadata {
        version: 1,
        track_id: 42,
        file_unique_id: "unique".into(),
        total,
        chunk_size: CHUNK_SIZE,
        ready_chunks: vec![0, 1],
    };
    std::fs::write(
        partial_metadata_path(&partial),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    assert_eq!(partial_cache_info(&partial).await.unwrap().received, total);

    let mut invalid = manifest;
    invalid.ready_chunks = vec![0, 0];
    std::fs::write(
        partial_metadata_path(&partial),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert!(partial_cache_info(&partial).await.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn metadata_prefix_chunk_count_uses_an_exclusive_end() {
    assert_eq!(
        TrackStream::prefix_chunk_count(0, CHUNK_SIZE * 3).unwrap(),
        0
    );
    assert_eq!(
        TrackStream::prefix_chunk_count(CHUNK_SIZE, CHUNK_SIZE * 3).unwrap(),
        1
    );
    assert_eq!(
        TrackStream::prefix_chunk_count(CHUNK_SIZE + 1, CHUNK_SIZE * 3).unwrap(),
        2
    );
    assert!(TrackStream::prefix_chunk_count(CHUNK_SIZE * 3 + 1, CHUNK_SIZE * 3).is_err());
}

#[test]
fn parses_and_validates_id3v2_prefix_length() {
    let header = [b'I', b'D', b'3', 4, 0, 0, 0, 0, 2, 0];
    assert_eq!(
        super::transfer::id3v2_tag_byte_length(&header, 1_000),
        Some(266)
    );

    let with_footer = [b'I', b'D', b'3', 4, 0, 0x10, 0, 0, 0, 20];
    assert_eq!(
        super::transfer::id3v2_tag_byte_length(&with_footer, 1_000),
        Some(40)
    );
    assert_eq!(
        super::transfer::id3v2_tag_byte_length(&with_footer, 40),
        None
    );
    assert_eq!(
        super::transfer::id3v2_tag_byte_length(b"not-an-id3", 1_000),
        None
    );
    assert_eq!(super::transfer::id3v2_tag_byte_length(b"ID3", 1_000), None);
}

#[test]
fn closed_playback_token_rejects_more_stream_work() {
    let active = std::sync::atomic::AtomicBool::new(false);
    assert!(TrackStream::require_active(Some(&active)).is_err());
}
