//! Columnar cache identity: a hash of the source file's identity and how it is scanned.

use std::fmt::Write;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::time::Duration;

    const SCAN: &str = "read_csv_auto('x.csv')";
    const VERSION: &str = "v1.5.4";

    fn key(path: &Path, scan: &str, version: &str) -> ColumnarKey {
        ColumnarKey::new(path, scan, version).unwrap()
    }

    #[test]
    fn key_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a,b\n1,2\n").unwrap();
        let first = key(&file, SCAN, VERSION);
        assert_eq!(first, key(&file, SCAN, VERSION));
        assert_eq!(first.as_str().len(), 64);
        assert!(
            first
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }

    #[test]
    fn key_changes_with_size_mtime_path_scan_or_version() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a,b\n1,2\n").unwrap();
        let base = key(&file, SCAN, VERSION);

        assert_ne!(base, key(&file, "read_csv_auto('y.csv')", VERSION));
        assert_ne!(base, key(&file, SCAN, "v1.5.5"));

        let other = dir.path().join("b.csv");
        fs::copy(&file, &other).unwrap();
        assert_ne!(base, key(&other, SCAN, VERSION));

        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified + Duration::from_secs(1))
            .unwrap();
        let touched = key(&file, SCAN, VERSION);
        assert_ne!(base, touched);

        fs::write(&file, "a,b\n1,2\n3,4\n").unwrap();
        File::options()
            .write(true)
            .open(&file)
            .unwrap()
            .set_modified(modified + Duration::from_secs(1))
            .unwrap();
        assert_ne!(touched, key(&file, SCAN, VERSION));
    }

    #[test]
    fn paths_live_under_the_columnar_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.csv");
        fs::write(&file, "a\n1\n").unwrap();
        let key = key(&file, SCAN, VERSION);
        let cache = dir.path().join("cache");
        fs::create_dir_all(cache.join("columnar")).unwrap();

        assert_eq!(
            key.final_path(&cache),
            cache
                .join("columnar")
                .join(format!("{}.duckdb", key.as_str()))
        );
        assert!(
            key.partial_path(&cache)
                .to_string_lossy()
                .ends_with(".duckdb.partial")
        );
        assert!(!key.is_ready(&cache));
        fs::write(key.final_path(&cache), b"").unwrap();
        assert!(key.is_ready(&cache));
    }
}
