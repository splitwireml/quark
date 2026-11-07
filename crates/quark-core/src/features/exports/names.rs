//! Export naming: media types, sheet names, download file names and the Content-Disposition header.

use std::collections::HashSet;
use std::fmt::Write as _;

use super::model::ExportFormat;
use crate::shared::encode::export::is_illegal_xml;

const SHEET_NAME_LIMIT: usize = 31;
const DEFAULT_SHEET: &str = "Sheet";
const DEFAULT_FILE: &str = "export";

pub fn media_type(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => "text/csv",
        ExportFormat::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ExportFormat::Parquet => "application/vnd.apache.parquet",
        ExportFormat::Json => "application/json",
    }
}

pub fn extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => ".csv",
        ExportFormat::Xlsx => ".xlsx",
        ExportFormat::Parquet => ".parquet",
        ExportFormat::Json => ".json",
    }
}

fn is_invalid_sheet_char(c: char) -> bool {
    matches!(c, '\\' | '/' | '*' | '?' | ':' | '[' | ']')
}

/// The first `limit` characters of `text` without trailing apostrophes, or `Sheet` when none remain.
fn cut_sheet_name(text: &str, limit: usize) -> String {
    let cut: String = text.chars().take(limit).collect();
    let trimmed = cut.trim_end_matches('\'');
    if trimmed.is_empty() {
        DEFAULT_SHEET.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// An Excel-safe, case-insensitively unique sheet name; `used` remembers the lower-cased names taken.
pub fn export_sheet_name(name: &str, used: &mut HashSet<String>) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !is_illegal_xml(*c))
        .map(|c| if is_invalid_sheet_char(c) { '_' } else { c })
        .collect();
    let stripped = cleaned.trim().trim_matches('\'');
    let mut base = cut_sheet_name(
        if stripped.is_empty() {
            DEFAULT_SHEET
        } else {
            stripped
        },
        SHEET_NAME_LIMIT,
    );
    // ponytail: Python folds with `casefold`; `to_lowercase` differs only for a few letters such as `ß`.
    if base.to_lowercase() == "history" {
        base.push('_');
    }
    let mut candidate = base.clone();
    let mut number = 2;
    while used.contains(&candidate.to_lowercase()) {
        let suffix = format!(" ({number})");
        let room = SHEET_NAME_LIMIT - suffix.chars().count();
        candidate = cut_sheet_name(&base, room) + &suffix;
        number += 1;
    }
    used.insert(candidate.to_lowercase());
    candidate
}

/// The last non-empty path component, the way `pathlib.PurePosixPath(name).name` reads it.
fn path_name(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty() && *part != ".")
        .unwrap_or("")
}

/// The name without its final suffix, as `pathlib.PurePath.stem` gives it.
fn path_stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 && dot < name.len() - 1 => &name[..dot],
        _ => name,
    }
}

/// A download file name made of a safe stem and the format's extension.
pub fn export_filename(name: Option<&str>, extension: &str) -> String {
    let normalized = name
        .filter(|n| !n.is_empty())
        .unwrap_or(DEFAULT_FILE)
        .replace('\\', "/");
    let stem: String = path_stem(path_name(&normalized))
        .chars()
        .filter(|c| !c.is_ascii_control())
        .collect();
    let stem = stem.trim_matches([' ', '.']);
    let stem = if stem.is_empty() { DEFAULT_FILE } else { stem };
    format!("{stem}{extension}")
}

/// Python's `urllib.parse.quote` with `/` safe.
fn quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'~' | b'/') {
            quoted.push(char::from(byte));
        } else {
            let _ = write!(quoted, "%{byte:02X}");
        }
    }
    quoted
}

/// The `Content-Disposition` value Starlette's `FileResponse` builds for a download.
pub fn content_disposition(filename: &str) -> String {
    let quoted = quote(filename);
    if quoted == filename {
        format!("attachment; filename=\"{filename}\"")
    } else {
        format!("attachment; filename*=utf-8''{quoted}")
    }
}

#[cfg(test)]
mod tests;
