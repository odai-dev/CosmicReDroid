use cosmic_redroid::config::{
    load_config_from_path, load_instances_from_path, save_instances_to_path,
    seed_default_instances, AppConfig,
};
use cosmic_redroid::models::{
    AndroidVersion, DevicePreset, InstanceStatus, NavPage,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_test_{}_{}", prefix, Uuid::new_v4()));
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
fn test_default_seeding_structure() {
    let defaults = seed_default_instances();
    assert_eq!(defaults.len(), 2, "Must seed exactly 2 default instances");

    // Instance 1: Galaxy S24 Ultra
    let inst1 = &defaults[0];
    assert_eq!(inst1.name, "Instance 1 - Galaxy S24 Ultra");
    assert_eq!(inst1.preset, DevicePreset::GalaxyS24Ultra);
    assert_eq!(inst1.android_version, AndroidVersion::Android13);
    assert_eq!(inst1.adb_port, 5555);
    assert_eq!(inst1.binder_index, 0);
    assert_eq!(inst1.status, InstanceStatus::Stopped);
    assert_eq!(inst1.identity.model, "SM-S928B");
    assert_eq!(inst1.identity.width, 1440);
    assert_eq!(inst1.identity.height, 3120);

    // Instance 2: Pixel 8 Pro
    let inst2 = &defaults[1];
    assert_eq!(inst2.name, "Instance 2 - Pixel 8 Pro");
    assert_eq!(inst2.preset, DevicePreset::Pixel8Pro);
    assert_eq!(inst2.android_version, AndroidVersion::Android13);
    assert_eq!(inst2.adb_port, 5557);
    assert_eq!(inst2.binder_index, 1);
    assert_eq!(inst2.status, InstanceStatus::Stopped);
    assert_eq!(inst2.identity.model, "Pixel 8 Pro");
    assert_eq!(inst2.identity.width, 1344);
    assert_eq!(inst2.identity.height, 2992);
}

#[test]
fn test_seeding_on_empty_and_whitespace_files() {
    let temp_dir = TestTempDir::new("seeding");
    let nonexistent = temp_dir.path().join("missing.json");
    let empty_file = temp_dir.path().join("empty.json");
    let whitespace_file = temp_dir.path().join("whitespace.json");

    // 1. Nonexistent file seeds defaults and writes them out
    let loaded1 = load_instances_from_path(&nonexistent).expect("load nonexistent");
    assert_eq!(loaded1.len(), 2);
    assert!(nonexistent.exists(), "Nonexistent path must be created on disk");

    // 2. 0-byte file seeds defaults
    fs::write(&empty_file, b"").expect("write 0-byte file");
    let loaded2 = load_instances_from_path(&empty_file).expect("load 0-byte");
    assert_eq!(loaded2.len(), 2);
    assert_eq!(loaded2[0].name, "Instance 1 - Galaxy S24 Ultra");

    // 3. File containing only whitespace seeds defaults
    fs::write(&whitespace_file, "   \n\t  \r\n   ").expect("write whitespace file");
    let loaded3 = load_instances_from_path(&whitespace_file).expect("load whitespace");
    assert_eq!(loaded3.len(), 2);
    assert_eq!(loaded3[1].name, "Instance 2 - Pixel 8 Pro");
}

#[test]
fn test_app_config_backward_compatibility() {
    let temp_dir = TestTempDir::new("compat");
    let defaults = seed_default_instances();

    // Format A: Legacy raw JSON array
    let raw_array_path = temp_dir.path().join("legacy_array.json");
    let raw_array_json = serde_json::to_string_pretty(&defaults).expect("serialize array");
    fs::write(&raw_array_path, &raw_array_json).expect("write raw array");

    // Format B: Structured AppConfig
    let structured_path = temp_dir.path().join("structured_config.json");
    let config_obj = AppConfig {
        instances: defaults.clone(),
        active_page: Some(NavPage::Snapshots),
    };
    let structured_json = serde_json::to_string_pretty(&config_obj).expect("serialize structured");
    fs::write(&structured_path, &structured_json).expect("write structured");

    // Format C: Structured AppConfig with forward-compatible unknown fields
    let future_structured_path = temp_dir.path().join("future_structured.json");
    let future_json = format!(
        "{{\"instances\": {}, \"active_page\": \"HostHealth\", \"extra_flag\": 42, \"dark_mode\": true}}",
        serde_json::to_string(&defaults).unwrap()
    );
    fs::write(&future_structured_path, &future_json).expect("write future structured");

    // Read with load_instances_from_path
    let insts_from_a = load_instances_from_path(&raw_array_path).expect("read A as instances");
    assert_eq!(insts_from_a.len(), 2);
    assert_eq!(insts_from_a[0].adb_port, 5555);

    let insts_from_b = load_instances_from_path(&structured_path).expect("read B as instances");
    assert_eq!(insts_from_b.len(), 2);
    assert_eq!(insts_from_b[1].adb_port, 5557);

    let insts_from_c = load_instances_from_path(&future_structured_path).expect("read C as instances");
    assert_eq!(insts_from_c.len(), 2);

    // Read with load_config_from_path
    let cfg_from_a = load_config_from_path(&raw_array_path).expect("read A as config");
    assert_eq!(cfg_from_a.instances.len(), 2);
    assert_eq!(cfg_from_a.active_page, Some(NavPage::Instances));

    let cfg_from_b = load_config_from_path(&structured_path).expect("read B as config");
    assert_eq!(cfg_from_b.instances.len(), 2);
    assert_eq!(cfg_from_b.active_page, Some(NavPage::Snapshots));

    let cfg_from_c = load_config_from_path(&future_structured_path).expect("read C as config");
    assert_eq!(cfg_from_c.instances.len(), 2);
    assert_eq!(cfg_from_c.active_page, Some(NavPage::HostHealth));

    // Explicit empty array '[]' preserves empty collection
    let empty_array_path = temp_dir.path().join("empty_array.json");
    fs::write(&empty_array_path, "[]").expect("write empty array");
    let loaded_empty = load_instances_from_path(&empty_array_path).expect("load empty array");
    assert_eq!(loaded_empty.len(), 0);
}

#[test]
fn test_crash_safety_dangling_temp_files() {
    let temp_dir = TestTempDir::new("crash_safety");
    let target_file = temp_dir.path().join("instances.json");

    let initial_seeds = seed_default_instances();
    save_instances_to_path(&initial_seeds, &target_file).expect("initial save");
    assert!(target_file.exists());

    // Stray temp files from interrupted writes
    let stray_temp = temp_dir.path().join(".instances.json.tmp.99999_abcd1234");
    fs::write(&stray_temp, b"{\"instances\": [{\"half_written\": true").unwrap();

    let loaded = load_instances_from_path(&target_file).expect("load during dangling temp");
    assert_eq!(loaded.len(), 2);

    let mut updated = loaded.clone();
    updated[0].name = "Updated Galaxy S24 Ultra".to_string();
    save_instances_to_path(&updated, &target_file).expect("save with dangling temp present");

    let reloaded = load_instances_from_path(&target_file).expect("reload");
    assert_eq!(reloaded[0].name, "Updated Galaxy S24 Ultra");
    assert!(stray_temp.exists(), "Stray temp file remains isolated");
}

#[test]
fn test_crash_safety_nested_directory_creation() {
    let temp_dir = TestTempDir::new("nested_dir");
    let deep_nested = temp_dir
        .path()
        .join("a")
        .join("b")
        .join("c")
        .join("instances.json");

    let instances = seed_default_instances();
    save_instances_to_path(&instances, &deep_nested).expect("save to deep nested path");
    assert!(deep_nested.exists());

    let loaded = load_instances_from_path(&deep_nested).expect("load from deep nested path");
    assert_eq!(loaded.len(), 2);
}

#[test]
fn test_corrupt_json_fails_and_preserves_backup() {
    let temp_dir = TestTempDir::new("corrupt_backup");
    let target_file = temp_dir.path().join("instances.json");

    fs::write(&target_file, b"NOT_VALID_JSON_CONTENT").unwrap();

    let result = load_instances_from_path(&target_file);
    assert!(result.is_err(), "Loading corrupt json must return Err");

    // Check that a .corrupted backup file was created to prevent data loss
    let entries: Vec<_> = fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let has_backup = entries.iter().any(|name| name.contains("corrupt"));
    assert!(has_backup, "Corrupted file backup must be generated");
}

#[test]
fn test_concurrent_readers_and_writers_no_corruption() {
    let temp_dir = TestTempDir::new("concurrent");
    let target_path = Arc::new(temp_dir.path().join("instances_concurrent.json"));

    let base_instances = seed_default_instances();
    save_instances_to_path(&base_instances, &target_path).expect("initial save");

    let num_writer_threads = 4;
    let num_reader_threads = 6;
    let iterations = 30;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let read_successes = Arc::new(AtomicUsize::new(0));
    let write_successes = Arc::new(AtomicUsize::new(0));
    let read_corruptions = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    for writer_id in 0..num_writer_threads {
        let path = Arc::clone(&target_path);
        let write_ok = Arc::clone(&write_successes);
        let stop = Arc::clone(&stop_flag);

        handles.push(std::thread::spawn(move || {
            let mut instances = seed_default_instances();
            for i in 0..iterations {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                instances[0].name = format!("Writer {writer_id} Iteration {i}");
                if save_instances_to_path(&instances, &path).is_ok() {
                    write_ok.fetch_add(1, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }));
    }

    for _ in 0..num_reader_threads {
        let path = Arc::clone(&target_path);
        let read_ok = Arc::clone(&read_successes);
        let read_corrupt = Arc::clone(&read_corruptions);
        let stop = Arc::clone(&stop_flag);

        handles.push(std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                match load_instances_from_path(&path) {
                    Ok(insts) => {
                        if insts.len() == 2 {
                            read_ok.fetch_add(1, Ordering::Relaxed);
                        } else {
                            read_corrupt.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        read_corrupt.fetch_add(1, Ordering::Relaxed);
                    }
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }));
    }

    // Wait for writers to complete
    for handle in handles.drain(..num_writer_threads) {
        let _ = handle.join();
    }

    stop_flag.store(true, Ordering::Relaxed);

    // Wait for readers
    for handle in handles {
        let _ = handle.join();
    }

    assert!(
        write_successes.load(Ordering::Relaxed) > 0,
        "Writers must succeed"
    );
    assert!(
        read_successes.load(Ordering::Relaxed) > 0,
        "Readers must succeed"
    );
    assert_eq!(
        read_corruptions.load(Ordering::Relaxed),
        0,
        "Zero corruptions allowed under concurrent read/write"
    );
}

#[test]
fn test_high_volume_instances_persistence() {
    let temp_dir = TestTempDir::new("high_volume");
    let target_file = temp_dir.path().join("large_config.json");

    let count = 200;
    let mut instances = Vec::with_capacity(count);
    let defaults = seed_default_instances();
    for i in 0..count {
        let mut inst = defaults[0].clone();
        inst.id = Uuid::new_v4();
        inst.name = format!("Instance {i}");
        inst.adb_port = 5555 + (i as u16) * 2;
        inst.binder_index = i as u32;
        instances.push(inst);
    }

    save_instances_to_path(&instances, &target_file).expect("save 200 instances");
    let loaded = load_instances_from_path(&target_file).expect("load 200 instances");
    assert_eq!(loaded.len(), count);
    assert_eq!(loaded[0].name, "Instance 0");
    assert_eq!(loaded[count - 1].name, format!("Instance {}", count - 1));
}

#[test]
fn test_models_serde_roundtrips() {
    // NavPage
    for page in NavPage::ALL {
        let serialized = serde_json::to_string(&page).unwrap();
        let deserialized: NavPage = serde_json::from_str(&serialized).unwrap();
        assert_eq!(page, deserialized);
    }

    // AndroidVersion
    for ver in AndroidVersion::ALL {
        let serialized = serde_json::to_string(&ver).unwrap();
        let deserialized: AndroidVersion = serde_json::from_str(&serialized).unwrap();
        assert_eq!(ver, deserialized);
    }

    // DevicePreset
    for preset in [
        DevicePreset::GalaxyS24Ultra,
        DevicePreset::Pixel8Pro,
        DevicePreset::AsusRogPhone8,
        DevicePreset::Custom,
    ] {
        let serialized = serde_json::to_string(&preset).unwrap();
        let deserialized: DevicePreset = serde_json::from_str(&serialized).unwrap();
        assert_eq!(preset, deserialized);
    }

    // InstanceStatus
    for status in [
        InstanceStatus::Stopped,
        InstanceStatus::Starting,
        InstanceStatus::Running,
        InstanceStatus::Stopping,
        InstanceStatus::Error("Test failure".to_string()),
        InstanceStatus::DownloadingImage("75% complete".to_string()),
    ] {
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: InstanceStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }
}
