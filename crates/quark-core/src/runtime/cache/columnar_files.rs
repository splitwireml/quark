//! Housekeeping for the columnar folder: partial imports, stray logs and eviction down to the cap.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Deletes a partial import and the write-ahead log DuckDB may have left beside it.
pub(super) fn remove_partial(partial: &Path) {
    let mut wal = OsString::from(partial);
    wal.push(".wal");
    for path in [partial, Path::new(&wal)] {
        if let Err(error) = fs::remove_file(path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not delete a partial columnar file");
        }
    }
}

/// Deletes the least recently used cache files until the folder fits in `cap` bytes.
/// The newest file stays even when it alone is over the cap: deleting it would make
/// the next mount import it again. A file the system refuses to delete (Windows keeps
/// an attached one) waits for the next pass.
pub(super) fn evict(columnar_dir: &Path, cap: u64) {
    let Ok(entries) = fs::read_dir(columnar_dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "duckdb"))
        .filter_map(|path| {
            let metadata = path.metadata().ok()?;
            Some((metadata.modified().ok()?, metadata.len(), path))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.sort();
    files.pop();
    for (_, len, path) in files {
        if total <= cap {
            break;
        }
        match fs::remove_file(&path) {
            Ok(()) => {
                total -= len;
                // DuckDB may leave a write-ahead log beside the file; it is useless alone.
                let mut wal = path.into_os_string();
                wal.push(".wal");
                let _ = fs::remove_file(wal);
            }
            Err(error) => tracing::debug!(%error, "columnar file in use; keeping it for now"),
        }
    }
}

pub(super) fn remove_stray_partials(columnar_dir: &Path) {
    let Ok(entries) = fs::read_dir(columnar_dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        let name = path.to_string_lossy();
        if name.ends_with(".partial") {
            remove_partial(&path);
        } else if name.ends_with(".partial.wal")
            && let Err(error) = fs::remove_file(&path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not delete an orphan partial log");
        }
    }
}
