// src/backend/storage.rs
//! Storage and snapshot subsystem for CosmicReDroid.
//!
//! Provides Copy-on-Write (CoW) instance data cloning via `cp -a --reflink=auto`,
//! deterministic ADB port and binder index allocators, automated instance cloning
//! with hardware identity regeneration, and multi-threaded `.tar.zst` snapshot
//! archive creation, restoration, listing, and deletion.

#![allow(dead_code)]

use crate::backend::spoofing::{
    generate_android_id, generate_luhn_imei, generate_mac_address, generate_serial,
    write_local_prop,
};
use crate::models::{AndroidInstance, HardwareIdentity, InstanceStatus, SnapshotInfo};
use anyhow::{Context, Result};
use chrono::Utc;
use std::net::TcpListener;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Base port for ADB daemon scanning.
pub const ADB_PORT_BASE: u16 = 5555;

/// Port scanning step increment (odd ports reserved for ADB daemon).
pub const ADB_PORT_STEP: u16 = 2;

/// Checks if a TCP port is available for local binding on 127.0.0.1.
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Finds the next available ADB port starting from 5555 with step 2.
///
/// Verifies both that the candidate port is not present in `existing_ports`
/// and that `127.0.0.1:{port}` can be bound via `TcpListener`.
pub fn find_next_adb_port(existing_ports: &[u16]) -> u16 {
    let mut candidate = ADB_PORT_BASE;
    loop {
        if !existing_ports.contains(&candidate) && is_port_available(candidate) {
            debug!("Selected available ADB port: {}", candidate);
            return candidate;
        }

        match candidate.checked_add(ADB_PORT_STEP) {
            Some(next) => candidate = next,
            None => {
                warn!("ADB port range exhausted, falling back to {}", ADB_PORT_BASE);
                return ADB_PORT_BASE;
            }
        }
    }
}

/// Finds the lowest non-negative integer binder index not present in `existing_indices`.
pub fn find_next_binder_index(existing_indices: &[u32]) -> u32 {
    let mut candidate = 0u32;
    loop {
        if !existing_indices.contains(&candidate) {
            debug!("Selected available binder index: {}", candidate);
            return candidate;
        }
        candidate = candidate.saturating_add(1);
    }
}

/// Allocates the next available ADB port and binder index for a new instance.
pub fn allocate_next_port_and_binder(instances: &[AndroidInstance]) -> (u16, u32) {
    let ports: Vec<u16> = instances.iter().map(|inst| inst.adb_port).collect();
    let binders: Vec<u32> = instances.iter().map(|inst| inst.binder_index).collect();
    (
        find_next_adb_port(&ports),
        find_next_binder_index(&binders),
    )
}

/// Clones an instance data directory using Copy-on-Write (`cp -a --reflink=auto -T`).
///
/// Automatically creates the parent directory of `dst_path` if it does not exist.
/// On CoW filesystems (Btrfs, XFS, ZFS), the clone completes in milliseconds.
/// On traditional filesystems (ext4), it gracefully falls back to recursive byte copy.
pub async fn clone_instance_data(src_path: &Path, dst_path: &Path) -> Result<()> {
    if !src_path.exists() {
        anyhow::bail!(
            "Source directory does not exist: {}",
            src_path.display()
        );
    }

    if let Some(parent) = dst_path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| {
            format!(
                "Failed to create parent directory for clone: {}",
                parent.display()
            )
        })?;
    }

    info!(
        "Cloning instance data from {} to {} via cp -a --reflink=auto",
        src_path.display(),
        dst_path.display()
    );

    let output = Command::new("cp")
        .arg("-a")
        .arg("--reflink=auto")
        .arg("-T")
        .arg(src_path)
        .arg(dst_path)
        .output()
        .await
        .with_context(|| "Failed to spawn cp process for CoW clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("cp clone failed with status {}: {}", output.status, stderr);
        anyhow::bail!(
            "Copy-on-write clone failed with exit status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    info!("Instance data clone completed successfully");
    Ok(())
}

/// Regenerates hardware identity parameters while preserving the device preset specifications.
pub fn regenerate_instance_identity(
    source_identity: &HardwareIdentity,
    tac_prefix: &str,
) -> HardwareIdentity {
    HardwareIdentity {
        manufacturer: source_identity.manufacturer.clone(),
        brand: source_identity.brand.clone(),
        model: source_identity.model.clone(),
        device: source_identity.device.clone(),
        board: source_identity.board.clone(),
        fingerprint: source_identity.fingerprint.clone(),
        serial_no: generate_serial(),
        imei: generate_luhn_imei(tac_prefix),
        android_id: generate_android_id(),
        mac_address: generate_mac_address(),
        width: source_identity.width,
        height: source_identity.height,
        dpi: source_identity.dpi,
        fps: source_identity.fps,
    }
}

/// Clones an Android instance including data directory, identity, port, and binder allocation.
///
/// Workflow:
/// 1. Generates new UUID for cloned instance.
/// 2. Allocates next available ADB port and binder index.
/// 3. Clones source data directory via `clone_instance_data`.
/// 4. Regenerates fresh hardware identity (IMEI, serial, Android ID, MAC).
/// 5. Writes updated `/data/local.prop` to the cloned destination.
/// 6. Returns a new `AndroidInstance` in `InstanceStatus::Stopped`.
pub async fn clone_instance(
    source: &AndroidInstance,
    existing_instances: &[AndroidInstance],
    new_name: Option<&str>,
) -> Result<AndroidInstance> {
    let new_id = Uuid::new_v4();
    let name = new_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{} (Clone)", source.name));

    let container_name = format!("cosmic-redroid-{}", &new_id.to_string()[..8]);
    let (new_port, new_binder) = allocate_next_port_and_binder(existing_instances);
    let new_data_path = crate::config::instance_data_dir(&new_id)?;

    info!(
        "Cloning instance '{}' ({}) -> '{}' ({}) on port {}, binder index {}",
        source.name, source.id, name, new_id, new_port, new_binder
    );

    if source.data_path.exists() {
        clone_instance_data(&source.data_path, &new_data_path).await?;
    } else {
        debug!(
            "Source data directory does not exist, initializing empty data directory: {}",
            new_data_path.display()
        );
        tokio::fs::create_dir_all(&new_data_path).await.with_context(|| {
            format!(
                "Failed to create destination data directory: {}",
                new_data_path.display()
            )
        })?;
    }

    let new_identity = regenerate_instance_identity(&source.identity, source.preset.tac_prefix());

    write_local_prop(&new_data_path, &new_identity).with_context(|| {
        format!(
            "Failed to write local.prop for cloned instance in {}",
            new_data_path.display()
        )
    })?;

    Ok(AndroidInstance {
        id: new_id,
        name,
        container_name,
        android_version: source.android_version,
        image_tag: source.image_tag.clone(),
        preset: source.preset,
        identity: new_identity,
        proxy: source.proxy.clone(),
        adb_port: new_port,
        data_path: new_data_path,
        binder_index: new_binder,
        status: InstanceStatus::Stopped,
    })
}

/// Creates a compressed `.tar.zst` snapshot archive from an instance data directory.
///
/// Compression uses multi-threaded Zstandard (`tar -I 'zstd -3 -T0' -cf ...`).
/// Saves archive and sidecar JSON metadata to `~/.local/share/cosmic-redroid/snapshots/<instance_id>/`.
pub async fn create_snapshot(
    instance_id: &Uuid,
    data_path: &Path,
    snapshot_name: &str,
) -> Result<SnapshotInfo> {
    if !data_path.exists() {
        anyhow::bail!(
            "Instance data directory does not exist: {}",
            data_path.display()
        );
    }

    let instance_snapshots_dir = crate::config::snapshots_dir()?.join(instance_id.to_string());
    tokio::fs::create_dir_all(&instance_snapshots_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create snapshots directory: {}",
                instance_snapshots_dir.display()
            )
        })?;

    let snapshot_id = Uuid::new_v4();
    let archive_path = instance_snapshots_dir.join(format!("{}.tar.zst", snapshot_id));
    let metadata_path = instance_snapshots_dir.join(format!("{}.json", snapshot_id));

    info!(
        "Creating snapshot '{}' ({}) for instance {} into {}",
        snapshot_name,
        snapshot_id,
        instance_id,
        archive_path.display()
    );

    let output = Command::new("tar")
        .arg("-I")
        .arg("zstd -3 -T0")
        .arg("-cf")
        .arg(&archive_path)
        .arg("-C")
        .arg(data_path)
        .arg(".")
        .output()
        .await
        .with_context(|| "Failed to execute tar command for snapshot creation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("tar snapshot creation failed: {}", stderr);
        anyhow::bail!(
            "Failed to create snapshot archive (status {}): {}",
            output.status,
            stderr.trim()
        );
    }

    let size_bytes = tokio::fs::metadata(&archive_path)
        .await
        .with_context(|| {
            format!(
                "Failed to read metadata for snapshot archive {}",
                archive_path.display()
            )
        })?
        .len();

    let snapshot_info = SnapshotInfo {
        id: snapshot_id,
        instance_id: *instance_id,
        name: snapshot_name.to_string(),
        created_at: Utc::now(),
        archive_path: archive_path.clone(),
        size_bytes,
    };

    let json_bytes = serde_json::to_vec_pretty(&snapshot_info)
        .context("Failed to serialize snapshot metadata to JSON")?;
    tokio::fs::write(&metadata_path, json_bytes)
        .await
        .with_context(|| {
            format!(
                "Failed to write snapshot metadata JSON to {}",
                metadata_path.display()
            )
        })?;

    info!(
        "Snapshot '{}' created successfully (size: {} bytes)",
        snapshot_name, size_bytes
    );
    Ok(snapshot_info)
}

/// Restores an instance data volume from a `.tar.zst` snapshot archive.
///
/// Cleans existing contents in `data_path` and unpacks archive via `tar -I zstd -xf ...`.
pub async fn restore_snapshot(snapshot: &SnapshotInfo, data_path: &Path) -> Result<()> {
    if !snapshot.archive_path.exists() {
        anyhow::bail!(
            "Snapshot archive does not exist at path: {}",
            snapshot.archive_path.display()
        );
    }

    info!(
        "Restoring snapshot '{}' ({}) into {}",
        snapshot.name,
        snapshot.id,
        data_path.display()
    );

    if data_path.exists() {
        debug!("Cleaning existing files in target data directory");
        let mut read_dir = tokio::fs::read_dir(data_path).await.with_context(|| {
            format!("Failed to read target data directory: {}", data_path.display())
        })?;

        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            if file_type.is_dir() {
                tokio::fs::remove_dir_all(&path).await.with_context(|| {
                    format!("Failed to remove directory entry: {}", path.display())
                })?;
            } else {
                tokio::fs::remove_file(&path).await.with_context(|| {
                    format!("Failed to remove file entry: {}", path.display())
                })?;
            }
        }
    } else {
        tokio::fs::create_dir_all(data_path).await.with_context(|| {
            format!(
                "Failed to create target data directory: {}",
                data_path.display()
            )
        })?;
    }

    let output = Command::new("tar")
        .arg("-I")
        .arg("zstd")
        .arg("-xf")
        .arg(&snapshot.archive_path)
        .arg("-C")
        .arg(data_path)
        .output()
        .await
        .with_context(|| "Failed to execute tar command for snapshot restoration")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("tar snapshot extraction failed: {}", stderr);
        anyhow::bail!(
            "Failed to extract snapshot archive (status {}): {}",
            output.status,
            stderr.trim()
        );
    }

    info!("Snapshot restoration completed successfully");
    Ok(())
}

/// Lists all available snapshots for a given instance.
///
/// Loads metadata from JSON files and validates archive presence. Results are
/// sorted in descending chronological order (newest snapshot first).
pub fn list_snapshots(instance_id: &Uuid) -> Result<Vec<SnapshotInfo>> {
    let dir = crate::config::snapshots_dir()?.join(instance_id.to_string());
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    let entries = std::fs::read_dir(&dir).with_context(|| {
        format!("Failed to read snapshot directory: {}", dir.display())
    })?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(mut info) = serde_json::from_str::<SnapshotInfo>(&content) {
                    if info.archive_path.exists() {
                        if let Ok(meta) = info.archive_path.metadata() {
                            info.size_bytes = meta.len();
                        }
                        snapshots.push(info);
                    }
                }
            }
        }
    }

    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

/// Lists all available snapshots across all instances.
pub fn list_all_snapshots() -> Result<Vec<SnapshotInfo>> {
    let base_dir = crate::config::snapshots_dir()?;
    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    let entries = std::fs::read_dir(&base_dir).with_context(|| {
        format!("Failed to read base snapshots directory: {}", base_dir.display())
    })?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) {
                if let Ok(instance_snaps) = list_snapshots(&id) {
                    snapshots.extend(instance_snaps);
                }
            }
        }
    }

    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

/// Deletes a snapshot archive and its associated metadata JSON file.
pub fn delete_snapshot(snapshot: &SnapshotInfo) -> Result<()> {
    if snapshot.archive_path.exists() {
        std::fs::remove_file(&snapshot.archive_path).with_context(|| {
            format!(
                "Failed to delete snapshot archive: {}",
                snapshot.archive_path.display()
            )
        })?;
    }

    let meta_path = snapshot
        .archive_path
        .parent()
        .map(|p| p.join(format!("{}.json", snapshot.id)))
        .unwrap_or_else(|| snapshot.archive_path.with_extension("json"));

    if meta_path.exists() {
        let _ = std::fs::remove_file(&meta_path);
    }

    info!(
        "Snapshot '{}' ({}) deleted successfully",
        snapshot.name, snapshot.id
    );
    Ok(())
}

/// Deletes a snapshot by its instance ID and snapshot ID.
pub fn delete_snapshot_by_id(instance_id: &Uuid, snapshot_id: &Uuid) -> Result<()> {
    let dir = crate::config::snapshots_dir()?.join(instance_id.to_string());
    let archive_path = dir.join(format!("{}.tar.zst", snapshot_id));
    let meta_path = dir.join(format!("{}.json", snapshot_id));

    let mut found = false;
    if archive_path.exists() {
        std::fs::remove_file(&archive_path).with_context(|| {
            format!(
                "Failed to delete snapshot archive: {}",
                archive_path.display()
            )
        })?;
        found = true;
    }

    if meta_path.exists() {
        let _ = std::fs::remove_file(&meta_path);
        found = true;
    }

    if !found {
        anyhow::bail!(
            "Snapshot {} not found for instance {}",
            snapshot_id,
            instance_id
        );
    }

    info!(
        "Snapshot {} for instance {} deleted successfully",
        snapshot_id, instance_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AndroidVersion, DevicePreset, ProxyConfig};
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    struct TempTestDir {
        path: PathBuf,
    }

    impl TempTestDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4()));
            std::fs::create_dir_all(&path).expect("failed to create temp test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_find_next_adb_port_empty_list() {
        let next = find_next_adb_port(&[]);
        assert!(next >= 5555);
        assert_eq!(next % 2, 1, "Port must be odd");
    }

    #[test]
    fn test_find_next_adb_port_stepping_and_gaps() {
        let existing = vec![5555, 5557];
        let next = find_next_adb_port(&existing);
        assert_eq!(next, 5559);

        let with_gap = vec![5555, 5559];
        let next_gap = find_next_adb_port(&with_gap);
        assert_eq!(next_gap, 5557);
    }

    #[test]
    fn test_find_next_adb_port_active_socket_bind_collision() {
        let test_port = 5599u16;
        let listener = TcpListener::bind(("127.0.0.1", test_port))
            .expect("bind test socket for collision");

        let available = is_port_available(test_port);
        assert!(!available, "Bound port must report unavailable");

        drop(listener);
        let mut available_after_drop = is_port_available(test_port);
        for _ in 0..10 {
            if available_after_drop {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            available_after_drop = is_port_available(test_port);
        }
        assert!(available_after_drop, "Freed port must report available");
    }

    #[test]
    fn test_find_next_binder_index_allocation() {
        assert_eq!(find_next_binder_index(&[]), 0);
        assert_eq!(find_next_binder_index(&[0]), 1);
        assert_eq!(find_next_binder_index(&[0, 1]), 2);
        assert_eq!(find_next_binder_index(&[1, 2]), 0, "Must fill hole at 0");
        assert_eq!(find_next_binder_index(&[0, 2]), 1, "Must fill hole at 1");
        assert_eq!(find_next_binder_index(&[0, 0, 1, 1, 3]), 2);
    }

    #[test]
    fn test_allocate_next_port_and_binder_helper() {
        let inst1 = AndroidInstance {
            id: Uuid::new_v4(),
            name: "Instance 1".to_string(),
            container_name: "test-1".to_string(),
            android_version: AndroidVersion::Android13,
            image_tag: "redroid/redroid:13.0.0-latest".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity: HardwareIdentity::default(),
            proxy: ProxyConfig::default(),
            adb_port: 5555,
            data_path: PathBuf::from("/tmp/d1"),
            binder_index: 0,
            status: InstanceStatus::Stopped,
        };

        let inst2 = AndroidInstance {
            adb_port: 5557,
            binder_index: 1,
            ..inst1.clone()
        };

        let instances = vec![inst1, inst2];
        let (port, binder) = allocate_next_port_and_binder(&instances);
        assert_eq!(port, 5559);
        assert_eq!(binder, 2);
    }

    #[tokio::test]
    async fn test_clone_instance_data_success() {
        let temp = TempTestDir::new("test_clone_cow");
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        tokio::fs::create_dir_all(src.join("subdir")).await.unwrap();
        let mut f = File::create(src.join("file1.txt")).unwrap();
        writeln!(f, "hello cosmic redroid").unwrap();
        let mut f2 = File::create(src.join("subdir/file2.txt")).unwrap();
        writeln!(f2, "nested data").unwrap();

        clone_instance_data(&src, &dst).await.expect("clone data");

        assert!(dst.exists());
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("subdir/file2.txt").exists());

        let content = std::fs::read_to_string(dst.join("file1.txt")).unwrap();
        assert_eq!(content.trim(), "hello cosmic redroid");
    }

    #[tokio::test]
    async fn test_clone_instance_data_nonexistent_src() {
        let temp = TempTestDir::new("test_clone_nonexistent");
        let src = temp.path().join("nonexistent_src");
        let dst = temp.path().join("dst");

        let res = clone_instance_data(&src, &dst).await;
        assert!(res.is_err());
        let err_str = res.unwrap_err().to_string();
        assert!(err_str.contains("Source directory does not exist"));
    }

    #[test]
    fn test_regenerate_instance_identity_uniqueness() {
        let original = HardwareIdentity::galaxy_s24_ultra();
        let regenerated = regenerate_instance_identity(&original, "35275311");

        assert_eq!(regenerated.manufacturer, original.manufacturer);
        assert_eq!(regenerated.model, original.model);
        assert_eq!(regenerated.width, original.width);
        assert_eq!(regenerated.height, original.height);
        assert_eq!(regenerated.dpi, original.dpi);

        assert_ne!(regenerated.serial_no, original.serial_no);
        assert_ne!(regenerated.android_id, original.android_id);
        assert_ne!(regenerated.mac_address, original.mac_address);
        assert_ne!(regenerated.imei, original.imei);

        assert_eq!(regenerated.imei.len(), 15);
        assert!(regenerated.imei.starts_with("35275311"));
        assert_eq!(regenerated.serial_no.len(), 11);
        assert_eq!(regenerated.android_id.len(), 16);
        assert!(regenerated.mac_address.starts_with("02:42:ac:"));
    }

    #[tokio::test]
    async fn test_clone_instance_workflow_full() {
        let temp = TempTestDir::new("test_clone_instance");
        let src_data = temp.path().join("src_data");
        tokio::fs::create_dir_all(&src_data).await.unwrap();

        let initial_ident = HardwareIdentity::galaxy_s24_ultra();
        let local_prop_lines = initial_ident.to_local_prop_lines().join("\n");
        tokio::fs::write(src_data.join("local.prop"), local_prop_lines).await.unwrap();

        let source = AndroidInstance {
            id: Uuid::new_v4(),
            name: "Original S24".to_string(),
            container_name: "cosmic-redroid-orig".to_string(),
            android_version: AndroidVersion::Android14,
            image_tag: "redroid/redroid:14.0.0-latest".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity: initial_ident.clone(),
            proxy: ProxyConfig::default(),
            adb_port: 5555,
            data_path: src_data.clone(),
            binder_index: 0,
            status: InstanceStatus::Stopped,
        };

        let cloned = clone_instance(&source, &[source.clone()], Some("Cloned S24"))
            .await
            .expect("clone instance workflow");

        assert_ne!(cloned.id, source.id);
        assert_eq!(cloned.name, "Cloned S24");
        assert_eq!(cloned.adb_port, 5557);
        assert_eq!(cloned.binder_index, 1);
        assert_eq!(cloned.status, InstanceStatus::Stopped);
        assert_eq!(cloned.android_version, AndroidVersion::Android14);
        assert_eq!(cloned.preset, DevicePreset::GalaxyS24Ultra);

        assert_ne!(cloned.identity.serial_no, source.identity.serial_no);
        assert_ne!(cloned.identity.imei, source.identity.imei);
        assert_ne!(cloned.identity.android_id, source.identity.android_id);

        let cloned_prop_path = cloned.data_path.join("local.prop");
        assert!(cloned_prop_path.exists());
        let prop_content = tokio::fs::read_to_string(&cloned_prop_path).await.unwrap();
        assert!(prop_content.contains(&format!("ro.serialno={}", cloned.identity.serial_no)));
        assert!(prop_content.contains(&format!("persist.radio.imei={}", cloned.identity.imei)));

        let _ = tokio::fs::remove_dir_all(&cloned.data_path).await;
        if let Some(parent) = cloned.data_path.parent() {
            let _ = tokio::fs::remove_dir_all(parent).await;
        }
    }

    #[tokio::test]
    async fn test_create_and_restore_snapshot_cycle() {
        let temp = TempTestDir::new("test_snapshot_cycle");
        let data_dir = temp.path().join("instance_data");
        tokio::fs::create_dir_all(&data_dir).await.unwrap();

        tokio::fs::write(data_dir.join("test_app.txt"), "important user data")
            .await
            .unwrap();

        let inst_id = Uuid::new_v4();
        let snapshot = create_snapshot(&inst_id, &data_dir, "Golden Image")
            .await
            .expect("create snapshot");

        assert_eq!(snapshot.name, "Golden Image");
        assert_eq!(snapshot.instance_id, inst_id);
        assert!(snapshot.archive_path.exists());
        assert!(snapshot.size_bytes > 0);

        tokio::fs::write(data_dir.join("test_app.txt"), "corrupted data")
            .await
            .unwrap();
        tokio::fs::write(data_dir.join("junk_file.bin"), "junk")
            .await
            .unwrap();

        restore_snapshot(&snapshot, &data_dir)
            .await
            .expect("restore snapshot");

        let restored_content = tokio::fs::read_to_string(data_dir.join("test_app.txt"))
            .await
            .unwrap();
        assert_eq!(restored_content, "important user data");
        assert!(
            !data_dir.join("junk_file.bin").exists(),
            "Junk file must be purged during clean restoration"
        );

        let list = list_snapshots(&inst_id).expect("list snapshots");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, snapshot.id);

        delete_snapshot(&snapshot).expect("delete snapshot");
        assert!(!snapshot.archive_path.exists());

        let list_after = list_snapshots(&inst_id).expect("list snapshots after delete");
        assert_eq!(list_after.len(), 0);

        if let Ok(snap_dir) = crate::config::snapshots_dir() {
            let inst_snap_dir = snap_dir.join(inst_id.to_string());
            let _ = tokio::fs::remove_dir_all(&inst_snap_dir).await;
        }
    }
}
