//! The identity of a columnar cache file (a hash of the source's identity and scan) and the job that fills it.

use std::fmt::Write;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// Content-addressed name of a columnar cache file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnarKey(String);

impl ColumnarKey {
    pub fn new(path: &Path, scan_expression: &str, engine_version: &str) -> io::Result<Self> {
        let canonical = path.canonicalize()?;
        let metadata = canonical.metadata()?;
        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_os_str().as_encoded_bytes());
        for part in [
            metadata.len().to_string(),
            mtime.to_string(),
            engine_version.to_owned(),
            scan_expression.to_owned(),
        ] {
            hasher.update(b"\0");
            hasher.update(part.as_bytes());
        }
        let mut hex = String::with_capacity(64);
        for byte in hasher.finalize() {
            write!(hex, "{byte:02x}").map_err(io::Error::other)?;
        }
        Ok(Self(hex))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn final_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join("columnar")
            .join(format!("{}.duckdb", self.0))
    }

    pub fn partial_path(&self, cache_dir: &Path) -> PathBuf {
        cache_dir
            .join("columnar")
            .join(format!("{}.duckdb.partial", self.0))
    }

    pub fn is_ready(&self, cache_dir: &Path) -> bool {
        self.final_path(cache_dir).is_file()
    }

    /// Marks the cache file as just used, so eviction keeps it longest.
    pub fn touch(&self, cache_dir: &Path) {
        let mut options = File::options();
        #[cfg(windows)]
        {
            // Attribute access alone cannot clash with the handle DuckDB holds on the file.
            use std::os::windows::fs::OpenOptionsExt;
            options.access_mode(0x0100); // FILE_WRITE_ATTRIBUTES
        }
        #[cfg(not(windows))]
        options.write(true);
        if let Err(error) = options
            .open(self.final_path(cache_dir))
            .and_then(|file| file.set_modified(SystemTime::now()))
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::debug!(%error, "could not mark a columnar file as used");
        }
    }
}

/// One flat file to import into the columnar cache.
pub struct ColumnarJob {
    pub source_id: String,
    pub key: ColumnarKey,
    pub source_path: PathBuf,
    pub scan_expression: String,
}

#[cfg(test)]
mod tests;
