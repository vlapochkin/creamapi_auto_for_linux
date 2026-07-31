use std::path::PathBuf;

pub fn get_resources_path() -> PathBuf {
    let user_res = crate::smokeapi_manager::get_user_resources_dir();
    if user_res.exists() && user_res.join("smoke_api64.dll").exists() {
        return user_res;
    }

    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.join("resources")))
        .unwrap_or_else(|| PathBuf::from("resources"))
}
