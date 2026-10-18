#[path = "../src/models.rs"]
pub mod models;

#[path = "../src/config.rs"]
pub mod config;

use chrono::Utc;
use config::*;
use models::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use uuid::Uuid;

struct ScopedTempDir {
    path: PathBuf,
}

impl ScopedTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_redroid_stress_{}_{}", prefix, Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create scoped temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// -----------------------------------------------------------------------------
// 1. Serde Edge-Cases and Malformed Payloads
// -----------------------------------------------------------------------------

#[test]
fn test_serde_nav_page_edge_cases() {
    for page in NavPage::ALL {
        let json = serde_json::to_string(&page).expect("serialize page");
        let decoded: NavPage = serde_json::from_str(&json).expect("deserialize page");
        assert_eq!(page, decoded);
    }

    // Invalid enum string
    assert!(serde_json::from_str::<NavPage>("\"InvalidPage\"").is_err());
    // Empty string
    assert!(serde_json::from_str::<NavPage>("\"\"").is_err());
    // Number
    assert!(serde_json::from_str::<NavPage>("0").is_err());
    // Null
    assert!(serde_json::from_str::<NavPage>("null").is_err());
    // Empty object
    assert!(serde_json::from_str::<NavPage>("{}").is_err());
}

#[test]
fn test_serde_android_version_edge_cases() {
    for version in AndroidVersion::ALL {
        let json = serde_json::to_string(&version).expect("serialize version");
        let decoded: AndroidVersion = serde_json::from_str(&json).expect("deserialize version");
        assert_eq!(version, decoded);
    }

    assert!(serde_json::from_str::<AndroidVersion>("\"Android12\"").is_err());
    assert!(serde_json::from_str::<AndroidVersion>("\"Android15\"").is_err());
    assert!(serde_json::from_str::<AndroidVersion>("\"\"").is_err());
    assert!(serde_json::from_str::<AndroidVersion>("13").is_err());
}

#[test]
fn test_serde_device_preset_edge_cases() {
    for preset in DevicePreset::ALL {
        let json = serde_json::to_string(&preset).expect("serialize preset");
        let decoded: DevicePreset = serde_json::from_str(&json).expect("deserialize preset");
        assert_eq!(preset, decoded);
    }

    assert!(serde_json::from_str::<DevicePreset>("\"iPhone15\"").is_err());
    assert!(serde_json::from_str::<DevicePreset>("\"\"").is_err());
    assert!(serde_json::from_str::<DevicePreset>("null").is_err());
}

#[test]
fn test_serde_instance_status_edge_cases() {
    let standard_statuses = vec![
        InstanceStatus::Stopped,
        InstanceStatus::Starting,
        InstanceStatus::WaitingForBoot,
        InstanceStatus::Running,
        InstanceStatus::Stopping,
        InstanceStatus::Cloning,
        InstanceStatus::Snapshotting,
    ];

    for status in standard_statuses {
        let json = serde_json::to_string(&status).expect("serialize status");
        let decoded: InstanceStatus = serde_json::from_str(&json).expect("deserialize status");
        assert_eq!(status, decoded);
    }

    // Error with empty string
    let empty_err = InstanceStatus::Error("".to_string());
    let json = serde_json::to_string(&empty_err).expect("serialize empty error");
    let decoded: InstanceStatus = serde_json::from_str(&json).expect("deserialize empty error");
    assert_eq!(empty_err, decoded);
    assert_eq!(decoded.display_status(), "Error: ");

    // Error with special characters: quotes, newlines, control chars
    let complex_err = InstanceStatus::Error("Docker timeout: \"OCI runtime failure\"\nLine 2: \\path\\to\\file\t[critical]".to_string());
    let json = serde_json::to_string(&complex_err).expect("serialize complex error");
    let decoded: InstanceStatus = serde_json::from_str(&json).expect("deserialize complex error");
    assert_eq!(complex_err, decoded);

    // Error with huge string (100 KB)
    let huge_msg = "x".repeat(100_000);
    let huge_err = InstanceStatus::Error(huge_msg.clone());
    let json = serde_json::to_string(&huge_err).expect("serialize huge error");
    let decoded: InstanceStatus = serde_json::from_str(&json).expect("deserialize huge error");
    assert_eq!(decoded, InstanceStatus::Error(huge_msg));

    // Malformed JSON payloads
    assert!(serde_json::from_str::<InstanceStatus>("{\"Error\": 12345}").is_err());
    assert!(serde_json::from_str::<InstanceStatus>("\"NonexistentStatus\"").is_err());
    assert!(serde_json::from_str::<InstanceStatus>("{\"UnknownVariant\": true}").is_err());
}

#[test]
fn test_serde_hardware_identity_edge_cases() {
    // Identity with all zeros and empty strings
    let empty_identity = HardwareIdentity {
        manufacturer: "".to_string(),
        brand: "".to_string(),
        model: "".to_string(),
        device: "".to_string(),
        board: "".to_string(),
        fingerprint: "".to_string(),
        serial_no: "".to_string(),
        imei: "".to_string(),
        android_id: "".to_string(),
        mac_address: "".to_string(),
        width: 0,
        height: 0,
        dpi: 0,
        fps: 0,
    };

    let json = serde_json::to_string(&empty_identity).expect("serialize empty identity");
    let decoded: HardwareIdentity = serde_json::from_str(&json).expect("deserialize empty identity");
    assert_eq!(empty_identity, decoded);
    assert_eq!(decoded.resolution_display(), "0x0 @ 0 FPS (0 DPI)");

    let prop_lines = decoded.to_local_prop_lines();
    assert_eq!(prop_lines.len(), 12);
    assert!(prop_lines.contains(&"ro.product.manufacturer=".to_string()));
    assert!(prop_lines.contains(&"ro.sf.lcd_density=0".to_string()));

    // Identity with maximum values
    let max_identity = HardwareIdentity {
        width: u32::MAX,
        height: u32::MAX,
        dpi: u32::MAX,
        fps: u32::MAX,
        ..empty_identity
    };
    let json = serde_json::to_string(&max_identity).expect("serialize max identity");
    let decoded: HardwareIdentity = serde_json::from_str(&json).expect("deserialize max identity");
    assert_eq!(max_identity, decoded);

    // Malformed JSON: width as string instead of integer
    let malformed_json = r#"{
        "manufacturer": "m", "brand": "b", "model": "m", "device": "d",
        "board": "b", "fingerprint": "f", "serial_no": "s", "imei": "i",
        "android_id": "a", "mac_address": "m", "width": "not_a_number",
        "height": 100, "dpi": 100, "fps": 60
    }"#;
    assert!(serde_json::from_str::<HardwareIdentity>(malformed_json).is_err());

    // Missing required field
    let missing_field_json = r#"{
        "manufacturer": "m", "brand": "b", "model": "m", "device": "d",
        "board": "b", "fingerprint": "f", "serial_no": "s", "imei": "i",
        "android_id": "a", "mac_address": "m", "width": 1080,
        "height": 1920, "dpi": 420
    }"#;
    assert!(serde_json::from_str::<HardwareIdentity>(missing_field_json).is_err());
}

#[test]
fn test_serde_proxy_config_edge_cases() {
    let disabled = ProxyConfig::default();
    assert_eq!(disabled.summary_display(), "Disabled");
    assert_eq!(disabled.host_port_string(), ":0");

    let active = ProxyConfig {
        enabled: true,
        protocol: "http".to_string(),
        host: "proxy.local".to_string(),
        port: 8080,
        username: Some("alice".to_string()),
        password: Some("p@ssword:123".to_string()),
    };
    assert_eq!(active.summary_display(), "http://proxy.local:8080");

    // Enabled but empty host -> should display "Disabled"
    let empty_host = ProxyConfig {
        enabled: true,
        protocol: "http".to_string(),
        host: "".to_string(),
        port: 8080,
        username: None,
        password: None,
    };
    assert_eq!(empty_host.summary_display(), "Disabled");

    // Enabled but port 0 -> should display "Disabled"
    let zero_port = ProxyConfig {
        enabled: true,
        protocol: "http".to_string(),
        host: "10.0.0.1".to_string(),
        port: 0,
        username: None,
        password: None,
    };
    assert_eq!(zero_port.summary_display(), "Disabled");

    // Port overflow in JSON (>65535)
    let overflow_json = r#"{"enabled":true,"protocol":"http","host":"h","port":70000,"username":null,"password":null}"#;
    assert!(serde_json::from_str::<ProxyConfig>(overflow_json).is_err());
}

#[test]
fn test_serde_container_metrics_edge_cases() {
    // Zero limit protection against NaN / panic
    let zero_metrics = ContainerMetrics {
        cpu_percent: 0.0,
        memory_used_mb: 0,
        memory_limit_mb: 0,
    };
    assert_eq!(zero_metrics.memory_percent(), 0.0);
    assert_eq!(zero_metrics.cpu_display(), "0.0%");
    assert_eq!(zero_metrics.memory_display(), "0 / 0 MB (0.0%)");

    // Utilization exceeding 100%
    let overutilized = ContainerMetrics {
        cpu_percent: 245.8,
        memory_used_mb: 5000,
        memory_limit_mb: 4096,
    };
    assert_eq!(overutilized.cpu_display(), "245.8%");
    assert!((overutilized.memory_percent() - 122.07031).abs() < 0.01);

    let json = serde_json::to_string(&overutilized).expect("serialize metrics");
    let decoded: ContainerMetrics = serde_json::from_str(&json).expect("deserialize metrics");
    assert_eq!(overutilized, decoded);
}

#[test]
fn test_serde_snapshot_info_edge_cases() {
    let now = Utc::now();
    let zero_size = SnapshotInfo {
        id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        name: "Zero Snapshot".to_string(),
        created_at: now,
        archive_path: PathBuf::from("/snapshots/zero.tar.zst"),
        size_bytes: 0,
    };
    assert_eq!(zero_size.size_display(), "0.0 MB");

    // Exact 1024 MB boundary
    let exact_1gb = SnapshotInfo {
        size_bytes: 1024 * 1024 * 1024,
        ..zero_size.clone()
    };
    assert_eq!(exact_1gb.size_display(), "1.00 GB");

    // 1023 MB (just below 1GB boundary)
    let sub_1gb = SnapshotInfo {
        size_bytes: 1023 * 1024 * 1024,
        ..zero_size.clone()
    };
    assert_eq!(sub_1gb.size_display(), "1023.0 MB");

    // Malformed UUID in JSON
    let malformed_uuid = r#"{
        "id": "not-a-uuid",
        "instance_id": "00000000-0000-0000-0000-000000000000",
        "name": "snap",
        "created_at": "2026-10-18T00:00:00Z",
        "archive_path": "/path",
        "size_bytes": 100
    }"#;
    assert!(serde_json::from_str::<SnapshotInfo>(malformed_uuid).is_err());
}

#[test]
fn test_serde_host_health_report_logic() {
    let report = HostHealthReport {
        docker_available: true,
        binder_available: true,
        gpu_available: true,
        adb_available: true,
        scrcpy_available: true,
        reflink_supported: true,
        remediation_needed: false,
        details: vec!["Check 1".into(), "Check 2".into()],
    };
    assert!(report.all_ok());
    assert!(!report.needs_remediation());

    // Single failure triggers needs_remediation
    let failed_docker = HostHealthReport {
        docker_available: false,
        ..report.clone()
    };
    assert!(!failed_docker.all_ok());
    assert!(failed_docker.needs_remediation());

    let failed_binder = HostHealthReport {
        binder_available: false,
        ..report.clone()
    };
    assert!(!failed_binder.all_ok());
    assert!(failed_binder.needs_remediation());

    let failed_gpu = HostHealthReport {
        gpu_available: false,
        ..report.clone()
    };
    assert!(!failed_gpu.all_ok());
    assert!(failed_gpu.needs_remediation());

    // Remediation flag true
    let flagged = HostHealthReport {
        remediation_needed: true,
        ..report
    };
    assert!(!flagged.all_ok());
    assert!(flagged.needs_remediation());
}

// -----------------------------------------------------------------------------
// 2. Binder Index Calculation Logic (Boundary: i = 0, 1, 10, 100)
// -----------------------------------------------------------------------------

#[test]
fn test_binder_index_calculations_at_boundaries() {
    let base_instance = AndroidInstance {
        id: Uuid::new_v4(),
        name: "Binder Test".to_string(),
        container_name: "binder-test".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: "redroid/redroid:13.0.0-latest".to_string(),
        preset: DevicePreset::Custom,
        identity: HardwareIdentity::default(),
        proxy: ProxyConfig::default(),
        adb_port: 5555,
        data_path: PathBuf::from("/data"),
        binder_index: 0,
        status: InstanceStatus::Stopped,
    };

    // Boundary: i = 0
    let inst_0 = AndroidInstance { binder_index: 0, ..base_instance.clone() };
    assert_eq!(inst_0.binder_indices(), (1, 2, 3));
    assert_eq!(
        inst_0.binder_nodes(),
        (
            "/dev/binder1".to_string(),
            "/dev/hwbinder2".to_string(),
            "/dev/vndbinder3".to_string()
        )
    );

    // Boundary: i = 1
    let inst_1 = AndroidInstance { binder_index: 1, ..base_instance.clone() };
    assert_eq!(inst_1.binder_indices(), (4, 5, 6));
    assert_eq!(
        inst_1.binder_nodes(),
        (
            "/dev/binder4".to_string(),
            "/dev/hwbinder5".to_string(),
            "/dev/vndbinder6".to_string()
        )
    );

    // Boundary: i = 10
    let inst_10 = AndroidInstance { binder_index: 10, ..base_instance.clone() };
    assert_eq!(inst_10.binder_indices(), (31, 32, 33));
    assert_eq!(
        inst_10.binder_nodes(),
        (
            "/dev/binder31".to_string(),
            "/dev/hwbinder32".to_string(),
            "/dev/vndbinder33".to_string()
        )
    );

    // Boundary: i = 100
    let inst_100 = AndroidInstance { binder_index: 100, ..base_instance.clone() };
    assert_eq!(inst_100.binder_indices(), (301, 302, 303));
    assert_eq!(
        inst_100.binder_nodes(),
        (
            "/dev/binder301".to_string(),
            "/dev/hwbinder302".to_string(),
            "/dev/vndbinder303".to_string()
        )
    );

    // Verify disjointness across 0..=100
    let mut seen = std::collections::HashSet::new();
    for i in 0..=100 {
        let (b1, b2, b3) = (i * 3 + 1, i * 3 + 2, i * 3 + 3);
        assert!(seen.insert(b1), "Duplicate binder node index: {}", b1);
        assert!(seen.insert(b2), "Duplicate binder node index: {}", b2);
        assert!(seen.insert(b3), "Duplicate binder node index: {}", b3);
    }
}

// -----------------------------------------------------------------------------
// 3. Stress-Test Atomic Configuration Persistence
// -----------------------------------------------------------------------------

#[test]
fn test_stress_concurrent_atomic_file_writes() {
    let temp_dir = ScopedTempDir::new("concurrent_writes");
    let target_file = Arc::new(temp_dir.path().join("instances_concurrent.json"));

    // Spawn 20 threads writing concurrently to the exact same file
    let thread_count = 20;
    let mut handles = Vec::with_capacity(thread_count);

    for thread_idx in 0..thread_count {
        let file_path = Arc::clone(&target_file);
        let handle = thread::spawn(move || {
            let mut instances = seed_default_instances();
            instances[0].name = format!("Instance from thread {}", thread_idx);
            instances[0].adb_port = 6000 + thread_idx as u16;

            for _ in 0..5 {
                save_instances_to_path(&instances, &file_path).expect("concurrent atomic save");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    // Verify the file on disk is completely valid and uncorrupted
    assert!(target_file.exists());
    let loaded = load_instances_from_path(&target_file).expect("load instances after concurrent writes");
    assert_eq!(loaded.len(), 2);
    assert!(loaded[0].name.starts_with("Instance from thread "));

    // Verify no stray .instances.json.tmp.* files remain in the directory
    let entries = fs::read_dir(temp_dir.path()).expect("read temp dir");
    for entry in entries {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.starts_with(".instances.json.tmp."),
            "Stray temporary file detected: {}",
            name
        );
    }
}

#[test]
fn test_atomic_persistence_permission_and_path_boundaries() {
    let temp_dir = ScopedTempDir::new("perm_boundaries");

    // 1. Target directory is read-only
    let ro_dir = temp_dir.path().join("ro_dir");
    fs::create_dir_all(&ro_dir).expect("create ro dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o555)).expect("set ro permissions");

        let ro_target = ro_dir.join("instances.json");
        let seeds = seed_default_instances();
        let save_result = save_instances_to_path(&seeds, &ro_target);
        assert!(save_result.is_err(), "Saving to read-only directory must return an Error");

        // Restore permissions for cleanup
        fs::set_permissions(&ro_dir, fs::Permissions::from_mode(0o755)).expect("restore permissions");
    }

    // 2. Target file is an existing directory (rename must fail)
    let dir_as_file = temp_dir.path().join("dir_as_target");
    fs::create_dir_all(&dir_as_file).expect("create dir_as_target");
    let seeds = seed_default_instances();
    let save_to_dir_result = save_instances_to_path(&seeds, &dir_as_file);
    assert!(save_to_dir_result.is_err(), "Renaming file over directory must return an Error");

    // 3. Deeply nested non-existent directory creation
    let deep_target = temp_dir.path().join("level1").join("level2").join("level3").join("instances.json");
    let save_deep_result = save_instances_to_path(&seeds, &deep_target);
    assert!(save_deep_result.is_ok(), "Parent directories must be created automatically");
    assert!(deep_target.exists());
}

#[test]
fn test_corrupt_json_fails_without_clobbering() {
    let temp_dir = ScopedTempDir::new("corrupt_json");
    let target = temp_dir.path().join("corrupt.json");

    // Malformed JSON syntax
    let malformed_content = "{ \"instances\": [ { \"id\": \"bad-uuid\" ";
    fs::write(&target, malformed_content).expect("write malformed file");

    let load_result = load_instances_from_path(&target);
    assert!(load_result.is_err(), "Loading corrupted JSON must return an error");

    // Ensure the original file content was preserved and NOT overwritten by seed defaults
    let content_after = fs::read_to_string(&target).expect("read content after");
    assert_eq!(content_after, malformed_content);

    // Empty whitespace file SHOULD seed defaults
    let empty_target = temp_dir.path().join("empty.json");
    fs::write(&empty_target, "   \n\t  \n").expect("write whitespace file");
    let load_empty = load_instances_from_path(&empty_target);
    assert!(load_empty.is_ok(), "Whitespace file should trigger seed defaults");
    let loaded = load_empty.unwrap();
    assert_eq!(loaded.len(), 2);
}

#[test]
fn test_app_config_dual_format_storage() {
    let temp_dir = ScopedTempDir::new("dual_format");
    let target = temp_dir.path().join("config.json");

    // Format A: Full AppConfig object with active_page
    let config = AppConfig {
        instances: seed_default_instances(),
        active_page: Some(NavPage::Snapshots),
    };
    save_config_to_path(&config, &target).expect("save config");

    let loaded_config = load_config_from_path(&target).expect("load config");
    assert_eq!(loaded_config.active_page, Some(NavPage::Snapshots));
    assert_eq!(loaded_config.instances.len(), 2);

    // Format B: Raw instance array (legacy or external edit)
    let instances = seed_default_instances();
    save_instances_to_path(&instances, &target).expect("save instances");

    let loaded_via_config = load_config_from_path(&target).expect("load raw array as config");
    assert_eq!(loaded_via_config.instances.len(), 2);
    assert_eq!(loaded_via_config.active_page, Some(NavPage::Instances)); // defaults to Instances

    let loaded_via_instances = load_instances_from_path(&target).expect("load as instances");
    assert_eq!(loaded_via_instances.len(), 2);
}

#[test]
fn test_concurrent_readers_and_writers_no_corruption() {
    let temp_dir = ScopedTempDir::new("readers_writers");
    let target = Arc::new(temp_dir.path().join("rw_instances.json"));

    // Seed file initially
    let initial_seeds = seed_default_instances();
    save_instances_to_path(&initial_seeds, &target).expect("initial save");

    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Spawn 8 reader threads
    let mut reader_handles = Vec::new();
    for _ in 0..8 {
        let file_path = Arc::clone(&target);
        let flag = Arc::clone(&stop_flag);
        let handle = thread::spawn(move || {
            let mut read_count = 0;
            while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                // Must NEVER fail with parse error or EOF mid-read
                let res = load_instances_from_path(&file_path);
                assert!(res.is_ok(), "Concurrent read failed: {:?}", res.err());
                read_count += 1;
                thread::yield_now();
            }
            read_count
        });
        reader_handles.push(handle);
    }

    // Spawn 4 writer threads
    let mut writer_handles = Vec::new();
    for writer_id in 0..4 {
        let file_path = Arc::clone(&target);
        let handle = thread::spawn(move || {
            for i in 0..20 {
                let mut instances = seed_default_instances();
                instances[0].name = format!("Writer {} iteration {}", writer_id, i);
                save_instances_to_path(&instances, &file_path).expect("writer atomic save");
                thread::yield_now();
            }
        });
        writer_handles.push(handle);
    }

    // Wait for writers to complete
    for handle in writer_handles {
        handle.join().expect("writer thread join");
    }

    // Signal readers to stop and join
    stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    for handle in reader_handles {
        let reads = handle.join().expect("reader thread join");
        assert!(reads > 0, "Reader must have executed at least 1 read");
    }

    // Final integrity check
    let final_instances = load_instances_from_path(&target).expect("final load");
    assert_eq!(final_instances.len(), 2);
}

#[test]
fn test_serde_forward_compatibility_extra_fields() {
    // Serde structs without #[serde(deny_unknown_fields)] should ignore unknown keys
    let json_with_future_fields = r#"{
        "id": "11111111-2222-3333-4444-555555555555",
        "name": "Future Instance",
        "container_name": "future-container",
        "android_version": "Android14",
        "image_tag": "redroid/redroid:14.0.0-latest",
        "preset": "GalaxyS24Ultra",
        "identity": {
            "manufacturer": "samsung",
            "brand": "samsung",
            "model": "SM-S928B",
            "device": "e3q",
            "board": "pineapple",
            "fingerprint": "fp",
            "serial_no": "sn",
            "imei": "123",
            "android_id": "abc",
            "mac_address": "02:42:ac:11:00:02",
            "width": 1440,
            "height": 3120,
            "dpi": 505,
            "fps": 120,
            "future_sensor_type": "lidar",
            "future_neural_engine": true
        },
        "proxy": {
            "enabled": false,
            "protocol": "http",
            "host": "",
            "port": 0,
            "username": null,
            "password": null,
            "extra_dns": "1.1.1.1"
        },
        "adb_port": 5555,
        "data_path": "/data",
        "binder_index": 0,
        "status": "Stopped",
        "unrecognized_future_field": [1, 2, 3]
    }"#;

    let parsed: Result<AndroidInstance, _> = serde_json::from_str(json_with_future_fields);
    assert!(parsed.is_ok(), "Domain models should ignore unknown future fields for forward compatibility");
    let instance = parsed.unwrap();
    assert_eq!(instance.name, "Future Instance");
    assert_eq!(instance.identity.model, "SM-S928B");
}

#[test]
fn test_high_volume_instances_persistence() {
    let temp_dir = ScopedTempDir::new("high_volume");
    let target = temp_dir.path().join("high_volume_instances.json");

    let count = 500;
    let mut instances = Vec::with_capacity(count);
    for i in 0..count {
        let (b1, b2, b3) = (i as u32 * 3 + 1, i as u32 * 3 + 2, i as u32 * 3 + 3);
        let inst = AndroidInstance {
            id: Uuid::new_v4(),
            name: format!("Instance {}", i),
            container_name: format!("cosmic-redroid-inst-{}", i),
            android_version: AndroidVersion::Android13,
            image_tag: "redroid/redroid:13.0.0-latest".to_string(),
            preset: DevicePreset::Pixel8Pro,
            identity: HardwareIdentity::pixel_8_pro(),
            proxy: ProxyConfig::default(),
            adb_port: 5555 + i as u16,
            data_path: PathBuf::from(format!("/instances/{}/data", i)),
            binder_index: i as u32,
            status: InstanceStatus::Stopped,
        };
        assert_eq!(inst.binder_indices(), (b1, b2, b3));
        instances.push(inst);
    }

    save_instances_to_path(&instances, &target).expect("save 500 instances");
    let loaded = load_instances_from_path(&target).expect("load 500 instances");
    assert_eq!(loaded.len(), count);
    assert_eq!(loaded[499].name, "Instance 499");
    assert_eq!(loaded[499].binder_index, 499);
}
