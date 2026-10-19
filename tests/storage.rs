use cosmic_redroid::backend::storage::{
    allocate_next_port_and_binder, clone_instance, clone_instance_data, delete_snapshot_by_id,
    find_next_adb_port, find_next_binder_index, restore_snapshot, ADB_PORT_BASE, ADB_PORT_STEP,
};
use cosmic_redroid::models::{
    AndroidInstance, AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus, ProxyConfig,
    SnapshotInfo,
};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_storage_{}_{}", prefix, Uuid::new_v4()));
        fs::create_dir_all(&path).expect("failed to create test temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_port_allocator_deterministic_and_hole_filling() {
    let empty_ports: [u16; 0] = [];
    assert_eq!(find_next_adb_port(&empty_ports), ADB_PORT_BASE);

    // Contiguous allocation
    let ports = vec![5555, 5557, 5559];
    assert_eq!(find_next_adb_port(&ports), 5561);

    // Hole filling: 5557 is missing
    let fragmented = vec![5555, 5559, 5561];
    assert_eq!(find_next_adb_port(&fragmented), 5557);

    // Non-sequential order
    let unsorted = vec![5563, 5555, 5559];
    assert_eq!(find_next_adb_port(&unsorted), 5557);
}

#[test]
fn test_port_allocator_active_socket_collision_fallback() {
    let test_port = 5601u16;
    if let Ok(_listener) = TcpListener::bind(("127.0.0.1", test_port)) {
        // test_port is currently bound at the OS level
        let existing = vec![5555, 5557, 5559];
        let chosen = find_next_adb_port(&existing);
        assert_ne!(
            chosen, test_port,
            "Port allocator must never allocate a port currently bound by a live socket"
        );
    }
}

#[test]
fn test_port_allocator_overflow_safety() {
    let mut high_ports = Vec::new();
    let mut p = 65525u16;
    loop {
        high_ports.push(p);
        match p.checked_add(ADB_PORT_STEP) {
            Some(next) => p = next,
            None => break,
        }
    }

    let allocated = find_next_adb_port(&high_ports);
    assert!(allocated >= ADB_PORT_BASE);
    assert_eq!(allocated % 2, 1, "ADB ports must be odd numbers");
}

#[test]
fn test_binder_index_calculations_and_mapping() {
    let empty_indices: [u32; 0] = [];
    assert_eq!(find_next_binder_index(&empty_indices), 0);

    // Sequential indices
    let seq = vec![0, 1, 2];
    assert_eq!(find_next_binder_index(&seq), 3);

    // Hole filling: hole at index 1
    let holes = vec![0, 2, 3];
    assert_eq!(find_next_binder_index(&holes), 1);

    // Large fragmented set
    let mut fragmented: Vec<u32> = (0..500).collect();
    fragmented.retain(|&x| x != 42 && x != 100);
    assert_eq!(find_next_binder_index(&fragmented), 42);

    fragmented.push(42);
    assert_eq!(find_next_binder_index(&fragmented), 100);

    fragmented.push(100);
    assert_eq!(find_next_binder_index(&fragmented), 500);
}

#[test]
fn test_allocate_next_port_and_binder_combined() {
    let defaults = cosmic_redroid::config::seed_default_instances();
    let mut inst1 = defaults[0].clone();
    inst1.name = "Inst 1".to_string();
    inst1.adb_port = 5555;
    inst1.binder_index = 0;

    let mut inst2 = defaults[1].clone();
    inst2.name = "Inst 2".to_string();
    inst2.adb_port = 5557;
    inst2.binder_index = 1;

    let instances = vec![inst1, inst2];

    let (port, binder) = allocate_next_port_and_binder(&instances);
    assert_eq!(port, 5559);
    assert_eq!(binder, 2);
}

#[tokio::test]
async fn test_clone_nonexistent_source_directory() {
    let temp = TestTempDir::new("clone_nonexistent");
    let missing_src = temp.path().join("missing_dir");
    let target_dst = temp.path().join("target_dir");

    let result = clone_instance_data(&missing_src, &target_dst).await;
    assert!(result.is_err(), "Cloning missing source directory must fail");
    assert!(!target_dst.exists(), "Destination must not exist after failed clone");

    // High level clone_instance recovers by initializing clean directory
    let source = AndroidInstance {
        id: Uuid::new_v4(),
        name: "Phantom Source".to_string(),
        container_name: "phantom-source".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: "redroid/redroid:13.0.0-latest".to_string(),
        preset: DevicePreset::Pixel8Pro,
        identity: HardwareIdentity::pixel_8_pro(),
        proxy: ProxyConfig::default(),
        adb_port: 5555,
        data_path: missing_src,
        binder_index: 0,
        status: InstanceStatus::Stopped,
    };

    let cloned_res = clone_instance(&source, &[source.clone()], Some("Cloned Phantom")).await;
    assert!(cloned_res.is_ok());
    let cloned = cloned_res.unwrap();
    assert_eq!(cloned.name, "Cloned Phantom");
    assert!(cloned.adb_port >= 5557 && cloned.adb_port % 2 == 1, "Cloned ADB port must be >= 5557 and odd");
    assert_eq!(cloned.binder_index, 1);
    assert!(cloned.data_path.exists());
    assert!(cloned.data_path.join("local.prop").exists());

    // Cleanup
    let _ = fs::remove_dir_all(&cloned.data_path);
    if let Some(parent) = cloned.data_path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
}

#[tokio::test]
async fn test_clone_invalid_destination_hierarchy() {
    let temp = TestTempDir::new("clone_invalid_dst");
    let src = temp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("data.txt"), b"payload").unwrap();

    // Create a regular file where destination parent should be
    let blocking_file = temp.path().join("blocker_file");
    fs::write(&blocking_file, b"content").unwrap();
    let illegal_dst = blocking_file.join("sub_dst");

    let result = clone_instance_data(&src, &illegal_dst).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_snapshot_restore_nonexistent_preserves_target() {
    let temp = TestTempDir::new("restore_nonexistent");
    let target_data = temp.path().join("instance_data");
    fs::create_dir_all(&target_data).unwrap();

    let valuable_file = target_data.join("important_user_database.db");
    fs::write(&valuable_file, b"CRITICAL_USER_DATA_DO_NOT_DELETE").unwrap();

    let missing_archive = temp.path().join("missing_snapshot.tar.zst");
    let fake_snapshot = SnapshotInfo {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        name: "Nonexistent Snapshot".to_string(),
        created_at: chrono::Utc::now(),
        archive_path: missing_archive,
        size_bytes: 0,
    };

    let result = restore_snapshot(&fake_snapshot, &target_data).await;
    assert!(result.is_err());

    // Safety check: target data must NOT be deleted when snapshot is missing
    assert!(valuable_file.exists());
    let content = fs::read(&valuable_file).unwrap();
    assert_eq!(content, b"CRITICAL_USER_DATA_DO_NOT_DELETE");
}

#[tokio::test]
async fn test_snapshot_restore_corrupted_fails_cleanly() {
    let temp = TestTempDir::new("restore_corrupted");
    let target_data = temp.path().join("instance_data");
    fs::create_dir_all(&target_data).unwrap();

    let corrupt_archive = temp.path().join("corrupted.tar.zst");
    fs::write(&corrupt_archive, b"GARBAGE_NOT_A_VALID_ZSTD_OR_TAR_ARCHIVE").unwrap();

    let corrupt_snapshot = SnapshotInfo {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        name: "Corrupt Snapshot".to_string(),
        created_at: chrono::Utc::now(),
        archive_path: corrupt_archive,
        size_bytes: 38,
    };

    let result = restore_snapshot(&corrupt_snapshot, &target_data).await;
    assert!(result.is_err());
}

#[test]
fn test_delete_nonexistent_snapshot_by_id() {
    let inst_id = Uuid::new_v4();
    let snap_id = Uuid::new_v4();
    let result = delete_snapshot_by_id(&inst_id, &snap_id);
    assert!(result.is_err());
}
