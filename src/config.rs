use std::path::PathBuf;

/// Returns the configuration directory path for CosmicReDroid.
pub fn config_dir() -> Result<PathBuf, anyhow::Error> {
    let base_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve local data directory"))?;
    Ok(base_dir.join("cosmic-redroid"))
}

/// Returns the path to the instances configuration file.
pub fn config_file_path() -> Result<PathBuf, anyhow::Error> {
    Ok(config_dir()?.join("instances.json"))
}
