//! Tests for the orphan-upload sweep.

use crate::runtime::state::AppState;
use crate::runtime::state::fixtures::*;

#[test]
fn orphan_uploads_are_deleted() {
    let root = tempfile::tempdir().unwrap();
    let dirs = dirs(root.path());
    let kept = dirs.uploads().join("kept.csv");
    let orphan = dirs.uploads().join("orphan.csv");
    let nested = dirs.uploads().join("nested").join("file.csv");
    for path in [&kept, &orphan, &nested] {
        write(path, "x\n1\n");
    }
    let roundabout = dirs.uploads().join("nested").join("..").join("kept.csv");
    write(
        &dirs.registry_file(),
        &format!(
            r#"[{{"id":"a","name":"A","kind":"csv","source":{:?}}}]"#,
            roundabout.to_string_lossy()
        ),
    );

    AppState::load(dirs).unwrap();

    assert!(kept.exists());
    assert!(!orphan.exists());
    assert!(nested.exists());
}
