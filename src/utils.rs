use std::path::PathBuf;

pub fn get_resources_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.join("resources")))
        .unwrap_or_else(|| PathBuf::from("resources"))
}
