use std::path::Path;

use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::cache;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const MAX_PROTOCOL_RESPONSE: u64 = super::CHUNK_SIZE;

pub async fn protocol_response(app: &AppHandle, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match try_protocol_response(app, request).await {
        Ok(response) => response,
        Err(error) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(error.to_string().into_bytes())
            .expect("valid streaming error response"),
    }
}

async fn try_protocol_response(
    app: &AppHandle,
    request: Request<Vec<u8>>,
) -> AppResult<Response<Vec<u8>>> {
    let track_id = request
        .uri()
        .path()
        .trim_matches('/')
        .rsplit('/')
        .next()
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| AppError::msg("invalid streaming track id"))?;
    let state = app.state::<AppState>();
    let (stream, cached_path, total, mime_type) =
        if let Some(stream) = state.streaming.get(track_id).await {
            let total = stream.total();
            let mime_type = stream.mime_type();
            (Some(stream), None, total, mime_type)
        } else {
            let Ok(track) = cache::require_track(&state, track_id) else {
                return Ok(not_found_response());
            };
            let path = cache::audio_path(&state, &track)?;
            if !path.exists() {
                return Ok(not_found_response());
            }
            let total = tokio::fs::metadata(&path).await?.len();
            if total == 0 {
                return Err(AppError::msg("cached audio file is empty"));
            }
            let mime_type = track.mime_type.unwrap_or_else(|| "audio/mpeg".into());
            (None, Some(path), total, mime_type)
        };

    if request.method() == Method::HEAD {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::CONTENT_LENGTH, total)
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "no-store")
            .body(Vec::new())
            .expect("valid streaming HEAD response"));
    }

    let (start, requested_end) =
        match parse_single_range(request.headers().get(header::RANGE), total) {
            Ok(range) => range,
            Err(()) => {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{total}"))
                    .header(header::ACCEPT_RANGES, "bytes")
                    .body(Vec::new())
                    .expect("valid range error response"));
            }
        };
    let end = requested_end.min(start + MAX_PROTOCOL_RESPONSE - 1);
    let bytes = if let Some(stream) = stream {
        stream.read_range(start, end).await?
    } else {
        read_file_range(
            cached_path
                .as_deref()
                .expect("cached path exists without an active stream"),
            start,
            end,
        )
        .await?
    };

    Ok(Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_TYPE, mime_type)
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(bytes)
        .expect("valid streaming range response"))
}

fn not_found_response() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .expect("valid not-found response")
}

pub async fn read_file_range(path: &Path, start: u64, end: u64) -> AppResult<Vec<u8>> {
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut bytes = vec![0; (end - start + 1) as usize];
    file.read_exact(&mut bytes).await?;
    Ok(bytes)
}

fn parse_single_range(
    header_value: Option<&tauri::http::HeaderValue>,
    total: u64,
) -> Result<(u64, u64), ()> {
    let Some(value) = header_value else {
        return Ok((0, total.saturating_sub(1)));
    };
    let value = value.to_str().map_err(|_| ())?;
    let range = value.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;

    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        let start = total.saturating_sub(suffix);
        return Ok((start, total.saturating_sub(1)));
    }

    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= total {
        return Err(());
    }
    let end = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(total - 1)
    };
    if end < start {
        return Err(());
    }
    Ok((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_and_suffix_http_ranges() {
        let open = tauri::http::HeaderValue::from_static("bytes=131072-");
        let suffix = tauri::http::HeaderValue::from_static("bytes=-256");

        assert_eq!(
            parse_single_range(Some(&open), 1_000_000),
            Ok((131_072, 999_999))
        );
        assert_eq!(parse_single_range(Some(&suffix), 1_000), Ok((744, 999)));
    }

    #[test]
    fn rejects_multiple_or_out_of_bounds_ranges() {
        let multiple = tauri::http::HeaderValue::from_static("bytes=0-1,4-5");
        let outside = tauri::http::HeaderValue::from_static("bytes=1000-");

        assert_eq!(parse_single_range(Some(&multiple), 1_000), Err(()));
        assert_eq!(parse_single_range(Some(&outside), 1_000), Err(()));
    }
}
