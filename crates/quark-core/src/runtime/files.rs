//! Deleting cache files, uploads and orphans from the data folders.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use serde_json::Value;

use crate::runtime::cache::columnar_key::ColumnarKey;
use crate::runtime::mount::database_paths;
use crate::runtime::registry::RawRecord;

/// Deletes a columnar cache file and its write-ahead log. One that stays behind is evicted later.
pub(crate) fn remove_cache_files(key: &ColumnarKey, cache_dir: &Path) {
    for path in database_paths(&key.final_path(cache_dir)) {
        remove_with_retry(Path::new(&path));
    }
}

/// Deletes a file, retrying once after 100 ms because Windows may still hold it.
/// An upload that stays behind is an orphan the next startup deletes; a cache file is evicted.
pub(crate) fn remove_with_retry(path: &Path) {
    let remove = || match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    };
    if remove().is_err() {
        thread::sleep(Duration::from_millis(100));
        if let Err(error) = remove() {
            tracing::warn!(path = %path.display(), %error, "could not delete file; leaving it for later cleanup");
        }
    }
}

/// Deletes files in `uploads` that no registry entry points at, comparing canonical paths.
pub(crate) fn remove_orphan_uploads(uploads: &Path, registry: &[RawRecord]) -> anyhow::Result<()> {
    let referenced: Vec<PathBuf> = registry
        .iter()
        .filter_map(|record| record.get("source").and_then(Value::as_str))
        .map(|source| fs::canonicalize(source).unwrap_or_else(|_| PathBuf::from(source)))
        .collect();
    for entry in fs::read_dir(uploads).context("listing uploads")? {
        let path = entry.context("listing uploads")?.path();
        let Ok(canonical) = fs::canonicalize(&path) else {
            continue;
        };
        if canonical.is_file()
            && !referenced.contains(&canonical)
            && let Err(error) = fs::remove_file(&path)
        {
            tracing::warn!(path = %path.display(), %error, "could not delete orphan upload");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
