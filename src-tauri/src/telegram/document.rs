//! Serialization and parsing of Telegram audio documents.
//!
//! Each track row stores a [`StoredDocument`] JSON blob carrying everything
//! needed to (re)download the audio and its thumbnail: the document identity,
//! file reference, home DC, and the chosen remote thumbnail size. This mirrors
//! the web app's `lib/mtproto/document.ts`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use grammers_tl_types as tl;
use serde::{Deserialize, Serialize};
use serde_json::json;

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
    pub fn thumb_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        let thumb_size = self.thumb_size.clone()?;
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
        tl::enums::InputDocument::Document(tl::types::InputDocument {
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
        attributes: serialize_attributes(&doc.attributes),
    };

    let (thumb_size, thumb_file_size) = pick_best_remote_thumb(doc.thumbs.as_deref());
    let stored = StoredDocument {
        thumb_size,
        thumb_file_size,
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

/// Picks the best remote thumbnail (prefers a mid "m" size, else the largest
/// declared byte size) and returns its `(type, file_size)`.
fn pick_best_remote_thumb(
    thumbs: Option<&[tl::enums::PhotoSize]>,
) -> (Option<String>, Option<String>) {
    let Some(thumbs) = thumbs else {
        return (None, None);
    };

    let mut best: Option<(String, i64, i32)> = None; // (type, size, priority)
    for thumb in thumbs {
        let (type_, size, priority) = match thumb {
            tl::enums::PhotoSize::Size(s) => (s.r#type.clone(), s.size as i64, 2),
            tl::enums::PhotoSize::Progressive(s) => (
                s.r#type.clone(),
                s.sizes.iter().copied().max().unwrap_or(0) as i64,
                1,
            ),
            _ => continue,
        };
        // Prefer the "m" size when present; otherwise fall back to largest.
        let this_priority = if type_ == "m" {
            priority + 10
        } else {
            priority
        };
        let take = match &best {
            None => true,
            Some((_, best_size, best_prio)) => {
                this_priority > *best_prio || (this_priority == *best_prio && size > *best_size)
            }
        };
        if take {
            best = Some((type_, size, this_priority));
        }
    }

    match best {
        Some((type_, size, _)) => (Some(type_), Some(size.to_string())),
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
        _ => "mp3",
    }
}
