//! Serialization and parsing of Telegram audio documents.
//!
//! Each track row stores a [`StoredDocument`] JSON blob carrying everything
//! needed to (re)download the audio and its thumbnail: the document identity,
//! file reference, home DC, and the chosen remote thumbnail size. This mirrors
//! the web app's `lib/mtproto/document.ts`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ferogram::tl;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredThumbnail {
    #[serde(rename = "type")]
    pub type_: String,
    pub width: i32,
    pub height: i32,
    #[serde(rename = "fileSize")]
    pub file_size: String,
}

/// Persisted, JSON-safe representation of a Telegram document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredDocument {
    pub id: i64,
    #[serde(rename = "accessHash")]
    pub access_hash: i64,
    /// Base64 of the file reference bytes (may expire; refreshed on demand).
    #[serde(rename = "fileReference")]
    pub file_reference: String,
    #[serde(rename = "dcId")]
    pub dc_id: i32,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    /// Document size in bytes, as a string (can exceed JS-safe integers).
    pub size: String,
    /// Chosen remote thumbnail size type (e.g. "m"), if any.
    #[serde(rename = "thumbSize", skip_serializing_if = "Option::is_none", default)]
    pub thumb_size: Option<String>,
    #[serde(
        rename = "thumbFileSize",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub thumb_file_size: Option<String>,
    /// Downloadable thumbnail variants retained for quality-aware callers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thumbnails: Vec<StoredThumbnail>,
    #[serde(default)]
    pub attributes: Vec<serde_json::Value>,
}

impl StoredDocument {
    pub fn file_reference_bytes(&self) -> Vec<u8> {
        B64.decode(self.file_reference.as_bytes())
            .unwrap_or_default()
    }

    pub fn size_bytes(&self) -> i64 {
        self.size.parse::<i64>().unwrap_or(0)
    }

    /// Builds the input location for downloading the full document.
    pub fn input_location(&self) -> tl::enums::InputFileLocation {
        tl::enums::InputFileLocation::InputDocumentFileLocation(
            tl::types::InputDocumentFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference_bytes(),
                thumb_size: String::new(),
            },
        )
    }

    /// Builds the input location for downloading the remote thumbnail, if one
    /// was selected during sync.
    pub fn thumb_input_location_for(
        &self,
        high_quality: bool,
    ) -> Option<tl::enums::InputFileLocation> {
        let thumb_size = if high_quality {
            best_thumbnail(&self.thumbnails)
                .map(|thumb| thumb.type_.clone())
                .or_else(|| self.thumb_size.clone())?
        } else {
            self.thumb_size.clone()?
        };
        Some(tl::enums::InputFileLocation::InputDocumentFileLocation(
            tl::types::InputDocumentFileLocation {
                id: self.id,
                access_hash: self.access_hash,
                file_reference: self.file_reference_bytes(),
                thumb_size,
            },
        ))
    }

    /// Builds an `InputDocument` for `users.GetSavedMusicByID` refreshes.
    /// An empty file reference is valid for that lookup.
    pub fn input_document(&self) -> tl::enums::InputDocument {
        tl::enums::InputDocument::InputDocument(tl::types::InputDocument {
            id: self.id,
            access_hash: self.access_hash,
            file_reference: Vec::new(),
        })
    }
}

/// Display fields plus the persisted JSON, parsed from a Telegram document.
pub struct ParsedDocument {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub duration: Option<i64>,
    pub file_unique_id: String,
    pub file_id: String,
    pub mime_type: String,
    pub size: i64,
    pub stored_json: String,
}

/// Parses a Telegram document into display metadata and its stored JSON blob.
pub fn parse_document(doc: &tl::types::Document) -> ParsedDocument {
    let mut title = None;
    let mut performer = None;
    let mut duration = None;
    let mut filename_title = None;

    for attr in &doc.attributes {
        match attr {
            tl::enums::DocumentAttribute::Audio(audio) => {
                title = audio.title.clone();
                performer = audio.performer.clone();
                duration = Some(audio.duration as i64);
            }
            tl::enums::DocumentAttribute::Filename(f) => {
                filename_title = Some(f.file_name.clone());
            }
            _ => {}
        }
    }

    if title.is_none() {
        title = filename_title;
    }

    let stored = StoredDocument {
        id: doc.id,
        access_hash: doc.access_hash,
        file_reference: B64.encode(&doc.file_reference),
        dc_id: doc.dc_id,
        mime_type: if doc.mime_type.is_empty() {
            "audio/mpeg".to_string()
        } else {
            doc.mime_type.clone()
        },
        size: doc.size.to_string(),
        thumb_size: None,
        thumb_file_size: None,
        thumbnails: Vec::new(),
        attributes: serialize_attributes(&doc.attributes),
    };

    let thumbnails = collect_remote_thumbnails(doc.thumbs.as_deref());
    let (thumb_size, thumb_file_size) = pick_best_remote_thumb(&thumbnails);
    let stored = StoredDocument {
        thumb_size,
        thumb_file_size,
        thumbnails,
        ..stored
    };

    ParsedDocument {
        title,
        performer,
        duration,
        // Telegram's file_unique_id concept isn't in the TL doc; the document
        // id is stable per account and serves the same de-duplication role.
        file_unique_id: doc.id.to_string(),
        file_id: doc.id.to_string(),
        mime_type: stored.mime_type.clone(),
        size: doc.size,
        stored_json: serde_json::to_string(&stored).unwrap_or_default(),
    }
}

fn collect_remote_thumbnails(thumbs: Option<&[tl::enums::PhotoSize]>) -> Vec<StoredThumbnail> {
    let Some(thumbs) = thumbs else {
        return Vec::new();
    };

    thumbs
        .iter()
        .filter_map(|thumb| match thumb {
            tl::enums::PhotoSize::PhotoSize(size) => Some(StoredThumbnail {
                type_: size.r#type.clone(),
                width: size.w,
                height: size.h,
                file_size: size.size.to_string(),
            }),
            tl::enums::PhotoSize::Progressive(size) => Some(StoredThumbnail {
                type_: size.r#type.clone(),
                width: size.w,
                height: size.h,
                file_size: size.sizes.iter().copied().max().unwrap_or(0).to_string(),
            }),
            _ => None,
        })
        .collect()
}

fn thumbnail_rank(thumb: &StoredThumbnail) -> (i64, i64) {
    let area = i64::from(thumb.width) * i64::from(thumb.height);
    let bytes = thumb.file_size.parse::<i64>().unwrap_or(0);
    (area, bytes)
}

fn best_thumbnail(thumbs: &[StoredThumbnail]) -> Option<&StoredThumbnail> {
    thumbs.iter().max_by_key(|thumb| thumbnail_rank(thumb))
}

fn pick_best_remote_thumb(thumbs: &[StoredThumbnail]) -> (Option<String>, Option<String>) {
    match best_thumbnail(thumbs) {
        Some(thumb) => (Some(thumb.type_.clone()), Some(thumb.file_size.clone())),
        None => (None, None),
    }
}

/// Converts document attributes into JSON-safe objects for storage/display.
fn serialize_attributes(attributes: &[tl::enums::DocumentAttribute]) -> Vec<serde_json::Value> {
    attributes
        .iter()
        .map(|attr| match attr {
            tl::enums::DocumentAttribute::Audio(a) => json!({
                "type": "DocumentAttributeAudio",
                "voice": a.voice,
                "duration": a.duration,
                "title": a.title,
                "performer": a.performer,
            }),
            tl::enums::DocumentAttribute::Filename(f) => json!({
                "type": "DocumentAttributeFilename",
                "fileName": f.file_name,
            }),
            tl::enums::DocumentAttribute::Video(v) => json!({
                "type": "DocumentAttributeVideo",
                "duration": v.duration,
                "w": v.w,
                "h": v.h,
            }),
            tl::enums::DocumentAttribute::ImageSize(i) => json!({
                "type": "DocumentAttributeImageSize",
                "w": i.w,
                "h": i.h,
            }),
            tl::enums::DocumentAttribute::Animated => {
                json!({ "type": "DocumentAttributeAnimated" })
            }
            tl::enums::DocumentAttribute::Sticker(_) => {
                json!({ "type": "DocumentAttributeSticker" })
            }
            _ => json!({ "type": "DocumentAttribute" }),
        })
        .collect()
}

/// XOR-hash over document ids, matching Telegram's saved-music list hash.
pub fn compute_saved_music_hash(document_ids: &[i64]) -> i64 {
    document_ids.iter().fold(0i64, |acc, id| acc ^ id)
}

/// Chooses a file extension for a cached audio document from its MIME type.
pub fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/webm" | "video/webm" => "webm",
        _ => "mp3",
    }
}

/// Sniffs a container MIME from the first bytes of a media file.
///
/// Telegram metadata is sometimes wrong (e.g. Opus-in-WebM labeled `audio/mpeg`).
/// Returning `None` means the header was too short or unrecognized.
pub fn sniff_container_mime(header: &[u8]) -> Option<&'static str> {
    if header.len() >= 4
        && header[0] == 0x1a
        && header[1] == 0x45
        && header[2] == 0xdf
        && header[3] == 0xa3
    {
        return Some("audio/webm");
    }
    if header.starts_with(b"ID3") {
        return Some("audio/mpeg");
    }
    if header.len() >= 2 && header[0] == 0xff && header[1] & 0xe0 == 0xe0 {
        return Some("audio/mpeg");
    }
    if header.len() >= 8 && &header[4..8] == b"ftyp" {
        return Some("audio/mp4");
    }
    if header.starts_with(b"fLaC") {
        return Some("audio/flac");
    }
    if header.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    if header.len() >= 12 && header.starts_with(b"RIFF") && &header[8..12] == b"WAVE" {
        return Some("audio/wav");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_largest_thumbnail_by_dimensions_then_size() {
        let thumbnails = vec![
            StoredThumbnail {
                type_: "m".into(),
                width: 320,
                height: 320,
                file_size: "50000".into(),
            },
            StoredThumbnail {
                type_: "x".into(),
                width: 800,
                height: 800,
                file_size: "120000".into(),
            },
            StoredThumbnail {
                type_: "y".into(),
                width: 800,
                height: 800,
                file_size: "150000".into(),
            },
        ];

        assert_eq!(
            pick_best_remote_thumb(&thumbnails),
            (Some("y".into()), Some("150000".into()))
        );
    }

    #[test]
    fn extension_for_mime_handles_webm() {
        assert_eq!(extension_for_mime("audio/webm"), "webm");
        assert_eq!(extension_for_mime("video/webm"), "webm");
    }

    #[test]
    fn sniff_container_mime_detects_webm_and_mp3() {
        assert_eq!(
            sniff_container_mime(&[0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0]),
            Some("audio/webm")
        );
        assert_eq!(sniff_container_mime(b"ID3\x04\0\0\0"), Some("audio/mpeg"));
        assert_eq!(
            sniff_container_mime(&[0xff, 0xfb, 0x90, 0x00]),
            Some("audio/mpeg")
        );
        assert_eq!(sniff_container_mime(b"fLaC...."), Some("audio/flac"));
    }
}
