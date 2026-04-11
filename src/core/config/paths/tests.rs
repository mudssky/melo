use std::path::PathBuf;

use super::{
    default_config_path, default_database_path, default_melo_root, resolve_from_config_dir,
};

#[test]
fn default_paths_are_nested_under_melo_root() {
    let root = default_melo_root();
    assert_eq!(default_config_path(), root.join("config.toml"));
    assert_eq!(default_database_path(), root.join("melo.db"));
}

#[test]
fn resolve_from_config_dir_keeps_absolute_paths() {
    let config_path = PathBuf::from("C:/temp/melo/config.toml");
    let absolute = PathBuf::from("D:/data/melo.db");

    assert_eq!(resolve_from_config_dir(&config_path, &absolute), absolute);
}

#[test]
fn resolve_from_config_dir_anchors_relative_paths_to_config_parent() {
    let config_path = PathBuf::from("C:/temp/melo/config.toml");
    let value = PathBuf::from("local/melo.db");

    assert_eq!(
        resolve_from_config_dir(&config_path, &value),
        PathBuf::from("C:/temp/melo/local/melo.db")
    );
}
