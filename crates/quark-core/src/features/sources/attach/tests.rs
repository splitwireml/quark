//! Tests for `~` expansion: which environment variable names the home folder and how a leading `~` resolves.

use super::{expand_path, expand_with_home, home_from};

#[test]
fn home_follows_python_expanduser_per_os() {
    let home = || Some("/home/me".into());
    let profile = || Some(r"C:\Users\me".into());
    assert_eq!(
        home_from(true, home(), profile()),
        Some(r"C:\Users\me".into())
    );
    assert_eq!(home_from(false, home(), profile()), Some("/home/me".into()));
    // Neither OS falls back to the other's variable, and an empty value is no home.
    assert_eq!(home_from(false, None, profile()), None);
    assert_eq!(home_from(true, home(), None), None);
    assert_eq!(home_from(false, Some("".into()), None), None);
}

#[test]
fn a_leading_tilde_expands_to_the_home_passed_in() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().to_path_buf();
    assert_eq!(
        expand_with_home("~/x", Some(home.clone())),
        expand_path(&home.join("x").to_string_lossy())
    );
    assert_eq!(
        expand_with_home("~", Some(home.clone())),
        expand_path(&home.to_string_lossy())
    );
}
