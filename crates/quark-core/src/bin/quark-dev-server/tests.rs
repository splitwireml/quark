//! Tests for the dev server's command-line parsing.

use super::*;

fn args(list: &[&str]) -> impl Iterator<Item = String> {
    list.iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn no_arguments_use_port_8000_and_the_data_folder() {
    let options = parse(args(&[])).unwrap();
    assert_eq!(options.port, 8000);
    assert_eq!(options.data, PathBuf::from("data"));
    assert_eq!(options.cache, None);
}

#[test]
fn cache_defaults_to_a_folder_inside_the_data_folder() {
    let dirs = parse(args(&[])).unwrap().into_dirs();
    assert_eq!(dirs.data, PathBuf::from("data"));
    assert_eq!(dirs.cache, PathBuf::from("data").join("cache"));

    let dirs = parse(args(&["--data-dir", "d"])).unwrap().into_dirs();
    assert_eq!(dirs.cache, PathBuf::from("d").join("cache"));
}

#[test]
fn explicit_cache_dir_wins_over_the_default() {
    let dirs = parse(args(&["--data-dir", "d", "--cache-dir", "c"]))
        .unwrap()
        .into_dirs();
    assert_eq!(dirs.cache, PathBuf::from("c"));
}

#[test]
fn unknown_flags_and_missing_values_are_rejected() {
    assert!(parse(args(&["--bogus", "1"])).is_none());
    assert!(parse(args(&["--port"])).is_none());
}
