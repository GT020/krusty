//! Persistence for dashboard descriptors.
//!
//! Only serializable specifications are stored; widget entities and streams are
//! recreated at startup.

use std::path::PathBuf;

use crate::models::dashboard::{DASHBOARD_VERSION, DashboardFile};

const DASHBOARD_FILE: &str = "dashboard.json";

pub fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("krusty").join(DASHBOARD_FILE))
}

pub fn load() -> DashboardFile {
    let Some(path) = path() else {
        return DashboardFile::default();
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => return DashboardFile::default(),
    };
    match serde_json::from_slice::<DashboardFile>(&bytes) {
        Ok(mut file) if file.version == DASHBOARD_VERSION && !file.dashboards.is_empty() => {
            if !file
                .dashboards
                .iter()
                .any(|dashboard| dashboard.id == file.active)
            {
                file.active = file.dashboards[0].id;
            }
            file
        }
        Ok(file) => {
            tracing::warn!(
                version = file.version,
                expected = DASHBOARD_VERSION,
                "dashboard: unsupported file version, starting empty"
            );
            backup(&path);
            DashboardFile::default()
        }
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "dashboard: invalid file, starting empty");
            backup(&path);
            DashboardFile::default()
        }
    }
}

pub fn save(file: &DashboardFile) -> Result<(), String> {
    let Some(path) = path() else {
        return Err("no config directory".to_string());
    };
    let Some(parent) = path.parent() else {
        return Err("invalid config path".to_string());
    };
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())
}

fn backup(path: &std::path::Path) {
    let backup = path.with_extension("json.bak");
    if let Err(error) = std::fs::rename(path, &backup) {
        tracing::warn!(%error, path = %path.display(), "dashboard: could not back up file");
    }
}
