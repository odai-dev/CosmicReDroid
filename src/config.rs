#![allow(dead_code)]

use crate::models::{
    AndroidInstance, AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus, NavPage,
    ProxyConfig,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Application configuration wrapper supporting instances and optional active page state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub instances: Vec<AndroidInstance>,
    #[serde(default)]
    pub active_page: Option<NavPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum InstancesStorage {
    Config(AppConfig),
    Instances(Vec<AndroidInstance>),
}

/// Returns the configuration directory path for CosmicReDroid (~/.local/share/cosmic-redroid).
pub fn config_dir() -> Result<PathBuf> {
    let base_dir = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve local data directory"))?;
    Ok(base_dir.join("cosmic-redroid"))
}

/// Returns the path to the instances configuration file (~/.local/share/cosmic-redroid/instances.json).
pub fn config_file_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("instances.json"))
}

/// Alias for `config_file_path()` returning the instances configuration file path.
pub fn config_path() -> Result<PathBuf> {
    config_file_path()
}

/// Alias for `config_file_path()` returning the instances configuration file path.
pub fn instances_config_path() -> Result<PathBuf> {
    config_file_path()
}

/// Returns the directory path where instance snapshots are stored.
pub fn snapshots_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("snapshots"))
}

/// Returns the persistent `/data` directory path for a specific instance.
pub fn instance_data_dir(instance_id: &Uuid) -> Result<PathBuf> {
    Ok(config_dir()?.join("instances").join(instance_id.to_string()).join("data"))
}

/// Generates the two initial default instances: Galaxy S24 Ultra and Pixel 8 Pro.
pub fn seed_default_instances() -> Vec<AndroidInstance> {
    let base_dir = config_dir().unwrap_or_else(|_| PathBuf::from("."));
    let instances_base = base_dir.join("instances");

    let instance1_id = Uuid::new_v4();
    let instance1 = AndroidInstance {
        id: instance1_id,
        name: "Instance 1 - Galaxy S24 Ultra".to_string(),
        container_name: "cosmic-redroid-instance-1".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: AndroidVersion::Android13.image_tag().to_string(),
        preset: DevicePreset::GalaxyS24Ultra,
        identity: HardwareIdentity::galaxy_s24_ultra(),
        proxy: ProxyConfig::default(),
        adb_port: 5555,
        data_path: instances_base.join(instance1_id.to_string()).join("data"),
        binder_index: 0,
        status: InstanceStatus::Stopped,
    };

    let instance2_id = Uuid::new_v4();
    let instance2 = AndroidInstance {
        id: instance2_id,
        name: "Instance 2 - Pixel 8 Pro".to_string(),
        container_name: "cosmic-redroid-instance-2".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: AndroidVersion::Android13.image_tag().to_string(),
        preset: DevicePreset::Pixel8Pro,
        identity: HardwareIdentity::pixel_8_pro(),
        proxy: ProxyConfig::default(),
        adb_port: 5557,
        data_path: instances_base.join(instance2_id.to_string()).join("data"),
        binder_index: 1,
        status: InstanceStatus::Stopped,
    };

    vec![instance1, instance2]
}

/// Saves instance collection to the specified file path atomically using temp file write + flush + rename.
pub fn save_instances_to_path(instances: &[AndroidInstance], target_path: &Path) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
    }

    let temp_name = format!(
        ".instances.json.tmp.{}_{}",
        std::process::id(),
        Uuid::new_v4().simple()
    );
    let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent_dir.join(temp_name);

    let json_bytes = serde_json::to_vec_pretty(instances)
        .context("Failed to serialize instances to JSON")?;

    {
        let mut file = std::fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temporary file '{}'", temp_path.display()))?;
        file.write_all(&json_bytes)
            .with_context(|| format!("Failed to write to temporary file '{}'", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync temporary file '{}'", temp_path.display()))?;
    }

    std::fs::rename(&temp_path, target_path).with_context(|| {
        format!(
            "Failed to atomically rename temporary file '{}' to '{}'",
            temp_path.display(),
            target_path.display()
        )
    })?;

    Ok(())
}

/// Saves instance collection to the standard configuration file path atomically.
pub fn save_instances(instances: &[AndroidInstance]) -> Result<()> {
    let path = config_file_path()?;
    save_instances_to_path(instances, &path)
}

/// Loads instance collection from the specified file path, seeding defaults if nonexistent or empty.
pub fn load_instances_from_path(target_path: &Path) -> Result<Vec<AndroidInstance>> {
    if !target_path.exists() {
        let seeds = seed_default_instances();
        save_instances_to_path(&seeds, target_path)?;
        return Ok(seeds);
    }

    let content = std::fs::read_to_string(target_path)
        .with_context(|| format!("Failed to read instances file '{}'", target_path.display()))?;

    if content.trim().is_empty() {
        let seeds = seed_default_instances();
        save_instances_to_path(&seeds, target_path)?;
        return Ok(seeds);
    }

    match serde_json::from_str::<InstancesStorage>(&content) {
        Ok(InstancesStorage::Instances(instances)) => Ok(instances),
        Ok(InstancesStorage::Config(cfg)) => Ok(cfg.instances),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to parse instances JSON at '{}': {}",
            target_path.display(),
            err
        )),
    }
}

/// Loads instance collection from the standard configuration file path, seeding defaults if nonexistent or empty.
pub fn load_instances() -> Result<Vec<AndroidInstance>> {
    let path = config_file_path()?;
    load_instances_from_path(&path)
}

/// Saves the complete `AppConfig` to the specified file path atomically.
pub fn save_config_to_path(config: &AppConfig, target_path: &Path) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory '{}'", parent.display()))?;
    }

    let temp_name = format!(
        ".config.json.tmp.{}_{}",
        std::process::id(),
        Uuid::new_v4().simple()
    );
    let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
    let temp_path = parent_dir.join(temp_name);

    let json_bytes = serde_json::to_vec_pretty(config)
        .context("Failed to serialize configuration to JSON")?;

    {
        let mut file = std::fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temporary file '{}'", temp_path.display()))?;
        file.write_all(&json_bytes)
            .with_context(|| format!("Failed to write to temporary file '{}'", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync temporary file '{}'", temp_path.display()))?;
    }

    std::fs::rename(&temp_path, target_path).with_context(|| {
        format!(
            "Failed to atomically rename temporary file '{}' to '{}'",
            temp_path.display(),
            target_path.display()
        )
    })?;

    Ok(())
}

/// Saves the complete `AppConfig` to the standard configuration file path atomically.
pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_file_path()?;
    save_config_to_path(config, &path)
}

/// Loads the complete `AppConfig` from the specified file path, seeding defaults if nonexistent or empty.
pub fn load_config_from_path(target_path: &Path) -> Result<AppConfig> {
    if !target_path.exists() {
        let seeds = seed_default_instances();
        let config = AppConfig {
            instances: seeds,
            active_page: Some(NavPage::Instances),
        };
        save_config_to_path(&config, target_path)?;
        return Ok(config);
    }

    let content = std::fs::read_to_string(target_path)
        .with_context(|| format!("Failed to read configuration file '{}'", target_path.display()))?;

    if content.trim().is_empty() {
        let seeds = seed_default_instances();
        let config = AppConfig {
            instances: seeds,
            active_page: Some(NavPage::Instances),
        };
        save_config_to_path(&config, target_path)?;
        return Ok(config);
    }

    match serde_json::from_str::<InstancesStorage>(&content) {
        Ok(InstancesStorage::Config(cfg)) => Ok(cfg),
        Ok(InstancesStorage::Instances(instances)) => Ok(AppConfig {
            instances,
            active_page: Some(NavPage::Instances),
        }),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to parse configuration JSON at '{}': {}",
            target_path.display(),
            err
        )),
    }
}

/// Loads the complete `AppConfig` from the standard configuration file path, seeding defaults if nonexistent or empty.
pub fn load_config() -> Result<AppConfig> {
    let path = config_file_path()?;
    load_config_from_path(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("cosmic_redroid_test_{}", Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("create test temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_config_paths() {
        let dir = config_dir().expect("config dir resolution");
        assert!(dir.ends_with("cosmic-redroid"));

        let file_path = config_file_path().expect("config file path");
        assert!(file_path.ends_with("instances.json"));

        let path_alias = config_path().expect("config path");
        assert_eq!(path_alias, file_path);

        let instances_alias = instances_config_path().expect("instances config path");
        assert_eq!(instances_alias, file_path);

        let snap_dir = snapshots_dir().expect("snapshots dir");
        assert!(snap_dir.ends_with("snapshots"));

        let test_uuid = Uuid::new_v4();
        let inst_data = instance_data_dir(&test_uuid).expect("instance data dir");
        assert!(inst_data.ends_with(format!("instances/{}/data", test_uuid)));
    }

    #[test]
    fn test_seed_default_instances_spec() {
        let defaults = seed_default_instances();
        assert_eq!(defaults.len(), 2);

        let inst1 = &defaults[0];
        assert_eq!(inst1.name, "Instance 1 - Galaxy S24 Ultra");
        assert_eq!(inst1.container_name, "cosmic-redroid-instance-1");
        assert_eq!(inst1.adb_port, 5555);
        assert_eq!(inst1.binder_index, 0);
        assert_eq!(inst1.preset, DevicePreset::GalaxyS24Ultra);
        assert_eq!(inst1.android_version, AndroidVersion::Android13);
        assert_eq!(inst1.identity.model, "SM-S928B");
        assert_eq!(inst1.status, InstanceStatus::Stopped);
        assert_eq!(inst1.binder_indices(), (1, 2, 3));

        let inst2 = &defaults[1];
        assert_eq!(inst2.name, "Instance 2 - Pixel 8 Pro");
        assert_eq!(inst2.container_name, "cosmic-redroid-instance-2");
        assert_eq!(inst2.adb_port, 5557);
        assert_eq!(inst2.binder_index, 1);
        assert_eq!(inst2.preset, DevicePreset::Pixel8Pro);
        assert_eq!(inst2.android_version, AndroidVersion::Android13);
        assert_eq!(inst2.identity.model, "Pixel 8 Pro");
        assert_eq!(inst2.status, InstanceStatus::Stopped);
        assert_eq!(inst2.binder_indices(), (4, 5, 6));

        assert_ne!(inst1.id, inst2.id);
    }

    #[test]
    fn test_atomic_save_and_load_instances_roundtrip() {
        let temp_dir = TestTempDir::new();
        let target_path = temp_dir.path().join("test_instances.json");

        let instances = seed_default_instances();
        save_instances_to_path(&instances, &target_path).expect("save instances");
        assert!(target_path.exists());

        let loaded = load_instances_from_path(&target_path).expect("load instances");
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, instances[0].name);
        assert_eq!(loaded[1].name, instances[1].name);
        assert_eq!(loaded[0].adb_port, instances[0].adb_port);
        assert_eq!(loaded[1].adb_port, instances[1].adb_port);
    }

    #[test]
    fn test_load_nonexistent_seeds_defaults() {
        let temp_dir = TestTempDir::new();
        let target_path = temp_dir.path().join("nonexistent_instances.json");

        assert!(!target_path.exists());
        let loaded = load_instances_from_path(&target_path).expect("load seeds");
        assert_eq!(loaded.len(), 2);
        assert!(target_path.exists(), "Target file must be seeded on disk");
    }

    #[test]
    fn test_load_empty_file_seeds_defaults() {
        let temp_dir = TestTempDir::new();
        let target_path = temp_dir.path().join("empty_instances.json");
        std::fs::write(&target_path, "   \n").expect("write empty file");

        let loaded = load_instances_from_path(&target_path).expect("load seeds");
        assert_eq!(loaded.len(), 2);
    }

    #[test]
    fn test_atomic_save_and_load_config_roundtrip() {
        let temp_dir = TestTempDir::new();
        let target_path = temp_dir.path().join("test_config.json");

        let config = AppConfig {
            instances: seed_default_instances(),
            active_page: Some(NavPage::HostHealth),
        };

        save_config_to_path(&config, &target_path).expect("save config");
        let loaded = load_config_from_path(&target_path).expect("load config");
        assert_eq!(loaded.active_page, Some(NavPage::HostHealth));
        assert_eq!(loaded.instances.len(), 2);
    }

    #[test]
    fn test_load_backward_compatibility_json_array() {
        let temp_dir = TestTempDir::new();
        let target_path = temp_dir.path().join("array_instances.json");

        let instances = seed_default_instances();
        let json = serde_json::to_string_pretty(&instances).expect("serialize");
        std::fs::write(&target_path, json).expect("write array");

        let loaded_as_config = load_config_from_path(&target_path).expect("load as config");
        assert_eq!(loaded_as_config.instances.len(), 2);

        let loaded_as_instances =
            load_instances_from_path(&target_path).expect("load as instances");
        assert_eq!(loaded_as_instances.len(), 2);
    }
}

