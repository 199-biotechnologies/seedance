//! Sidecar `.seedance.json` manifest written next to every generated mp4.
//!
//! Agents can list an output directory and read the sidecar to know exactly
//! which prompt produced which file -- no guessing from the task-id filename.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppError;

pub const MANIFEST_EXT: &str = "seedance.json";

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub schema: &'static str,
    /// Which code path wrote this sidecar. `"generate"` means the manifest
    /// was written from the originating `generate --wait` call and carries
    /// the full request payload (prompt + references). `"download"` means it
    /// was reconstructed from the API's GetTask response, which does not
    /// echo the original request -- so `prompt` is null and `references`
    /// is empty. Agents should key on this field before trusting either.
    pub source: &'static str,
    pub task_id: String,
    pub model: String,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ratio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_audio: Option<bool>,
    #[serde(default)]
    pub references: References,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_frame_url: Option<String>,
    pub downloaded_to: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct References {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub videos: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_frame: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_frame: Option<String>,
}

/// Path of the sidecar manifest for a given mp4 path: `foo.mp4` -> `foo.seedance.json`.
pub fn sidecar_path(mp4: &Path) -> PathBuf {
    mp4.with_extension(MANIFEST_EXT)
}

/// Write the manifest as pretty JSON beside the mp4. Never clobbers the mp4 itself.
pub fn write(mp4_path: &Path, m: &Manifest) -> Result<PathBuf, AppError> {
    let json_path = sidecar_path(mp4_path);
    let json = serde_json::to_string_pretty(m)
        .map_err(|e| AppError::Transient(format!("manifest serialize failed: {e}")))?;
    if let Some(parent) = json_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&json_path, json)?;
    Ok(json_path)
}

/// A short human-friendly slug from an arbitrary label.
/// "Alice at the cafe!" -> "alice-at-the-cafe". Empty / all-junk -> None.
pub fn slug(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = true;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            for low in c.to_lowercase() {
                out.push(low);
            }
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let final_ = if trimmed.len() > 48 {
        &trimmed[..48]
    } else {
        trimmed
    };
    if final_.is_empty() {
        None
    } else {
        Some(final_.trim_matches('-').to_string())
    }
}

/// Compact UTC timestamp used in filenames. Format: `20260420T023015Z`.
pub fn timestamp_compact() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = civil_from_epoch(secs);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// ISO 8601 UTC timestamp for the manifest `created_at` field.
pub fn iso8601_from_epoch_secs(secs: i64) -> String {
    let s = if secs < 0 { 0u64 } else { secs as u64 };
    let (y, mo, d, h, mi, s) = civil_from_epoch(s);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

pub fn iso8601_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    iso8601_from_epoch_secs(secs as i64)
}

/// Last 8 hex-ish chars of a task id for use in compact filenames.
/// `cgt-20260416-abcd1234` -> `abcd1234`.
pub fn short_id(task_id: &str) -> String {
    let tail: String = task_id
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if tail.len() >= 8 {
        tail[tail.len() - 8..].to_string()
    } else if !tail.is_empty() {
        tail
    } else {
        task_id.chars().take(8).collect()
    }
}

/// Howard Hinnant's civil_from_days, adapted for epoch seconds.
/// Returns (year, month, day, hour, minute, second) in UTC.
fn civil_from_epoch(total_secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = total_secs / 86_400;
    let rem = total_secs % 86_400;
    let h = (rem / 3_600) as u32;
    let mi = ((rem % 3_600) / 60) as u32;
    let s = (rem % 60) as u32;

    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if mo <= 2 { (y + 1) as i32 } else { y as i32 };
    (y, mo, d, h, mi, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_basic() {
        assert_eq!(
            slug("Alice at the cafe!").as_deref(),
            Some("alice-at-the-cafe")
        );
        assert_eq!(slug("   ---   ").as_deref(), None);
        assert_eq!(slug("").as_deref(), None);
        assert_eq!(slug("A").as_deref(), Some("a"));
    }

    #[test]
    fn slug_truncates() {
        let long = "a".repeat(100);
        let s = slug(&long).unwrap();
        assert!(s.len() <= 48);
    }

    #[test]
    fn civil_known_dates() {
        // 1970-01-01T00:00:00Z
        assert_eq!(civil_from_epoch(0), (1970, 1, 1, 0, 0, 0));
        // 2000-01-01T00:00:00Z = 946684800
        assert_eq!(civil_from_epoch(946_684_800), (2000, 1, 1, 0, 0, 0));
        // 2026-04-20T00:00:00Z = 1776643200
        assert_eq!(civil_from_epoch(1_776_643_200), (2026, 4, 20, 0, 0, 0));
        // 2024-02-29T12:34:56Z (leap-day sanity check) = 1709210096
        assert_eq!(civil_from_epoch(1_709_210_096), (2024, 2, 29, 12, 34, 56));
    }

    #[test]
    fn iso_round_trip() {
        assert_eq!(iso8601_from_epoch_secs(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso8601_from_epoch_secs(1_776_643_200),
            "2026-04-20T00:00:00Z"
        );
    }

    #[test]
    fn short_id_cases() {
        assert_eq!(short_id("cgt-20260416-abcd1234"), "abcd1234");
        assert_eq!(short_id("tiny"), "tiny");
        assert_eq!(short_id(""), "");
    }

    #[test]
    fn sidecar_path_swap() {
        let p = std::path::Path::new("/tmp/foo/bar.mp4");
        assert_eq!(
            sidecar_path(p).to_string_lossy(),
            "/tmp/foo/bar.seedance.json"
        );
    }
}
