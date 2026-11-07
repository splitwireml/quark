//! Tests for the columnar cache key and its paths.

use super::*;
use std::fs;
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
