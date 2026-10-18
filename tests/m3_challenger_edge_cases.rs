// tests/m3_challenger_edge_cases.rs
// Empirical Adversarial & Edge-Case Stress Testing Suite for Milestone 3
// Focus Areas: Error Recovery, Injection Safety, scrcpy Formatting, Atomic Writes, Host Probes
// Strict Zero-Emoji Compliance.

#[path = "../src/models.rs"]
pub mod models;

#[path = "../src/config.rs"]
pub mod config;

#[path = "../src/backend/mod.rs"]
pub mod backend;

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use backend::adb_scrcpy::{
    build_adb_connect_args, build_adb_disconnect_args, build_identity_commands_args,
    build_proxy_command_args, build_scrcpy_args, build_scrcpy_command,
};
use backend::host::{
    generate_remediation_script, probe_adb, probe_binder, probe_docker, probe_gpu_dri,
    probe_host_health, probe_reflink, probe_scrcpy, run_host_diagnostics,
};
use backend::spoofing::{
    populate_preset, write_local_prop, write_local_prop_async,
};
use backend::storage::{
    clone_instance, clone_instance_data, create_snapshot,
    delete_snapshot, delete_snapshot_by_id, find_next_adb_port, find_next_binder_index,
    restore_snapshot, ADB_PORT_BASE, ADB_PORT_STEP,
};
use models::{
    AndroidInstance, AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus, ProxyConfig,
    SnapshotInfo,
};

/// Scoped temporary directory that deletes itself upon drop.
struct ScopedTempDir {
    path: PathBuf,
}

impl ScopedTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_m3_edge_{}_{}", prefix, Uuid::new_v4()));
        fs::create_dir_all(&path).expect("failed to create temporary test directory");
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

// =========================================================================
// 1. ERROR RECOVERY PATHS
// =========================================================================

#[tokio::test]
async fn test_error_recovery_clone_nonexistent_source_directory() {
    let temp = ScopedTempDir::new("clone_nonexistent");
    let missing_src = temp.path().join("does_not_exist_source_dir");
    let target_dst = temp.path().join("target_dst_dir");

    // 1. Direct clone_instance_data call on missing source
    let result = clone_instance_data(&missing_src, &target_dst).await;
    assert!(result.is_err(), "Cloning missing source directory must fail");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Source directory does not exist"),
        "Error message must specify source does not exist: {}",
        err_msg
    );
    assert!(
        !target_dst.exists(),
        "Destination directory must not be created on failed clone"
    );

    // 2. High-level clone_instance when source instance data_path does not exist
    let source_instance = AndroidInstance {
        id: Uuid::new_v4(),
        name: "Phantom Source".to_string(),
        container_name: "phantom-source".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: "redroid/redroid:13.0.0-latest".to_string(),
        preset: DevicePreset::Pixel8Pro,
        identity: HardwareIdentity::pixel_8_pro(),
        proxy: ProxyConfig::default(),
        adb_port: 5555,
        data_path: missing_src.clone(),
        binder_index: 0,
        status: InstanceStatus::Stopped,
    };

    let cloned_res = clone_instance(&source_instance, &[source_instance.clone()], Some("Cloned Phantom")).await;
    assert!(
        cloned_res.is_ok(),
        "clone_instance must recover gracefully from missing source data_path by initializing empty directory"
    );
    let cloned = cloned_res.unwrap();
    assert_eq!(cloned.name, "Cloned Phantom");
    assert_eq!(cloned.adb_port, 5557);
    assert_eq!(cloned.binder_index, 1);
    assert!(cloned.data_path.exists(), "Cloned data_path must be created");
    assert!(
        cloned.data_path.join("local.prop").exists(),
        "Cloned data_path must contain initialized local.prop"
    );

    // Cleanup cloned dir
    let _ = fs::remove_dir_all(&cloned.data_path);
    if let Some(parent) = cloned.data_path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
}

#[tokio::test]
async fn test_error_recovery_clone_invalid_destination_hierarchy() {
    let temp = ScopedTempDir::new("clone_invalid_dst");
    let src = temp.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("data.txt"), b"payload").unwrap();

    // Create a regular file where parent directory of dst should be
    let blocking_file = temp.path().join("blocker_file");
    fs::write(&blocking_file, b"im_a_file").unwrap();
    let illegal_dst = blocking_file.join("sub_dst");

    let result = clone_instance_data(&src, &illegal_dst).await;
    assert!(
        result.is_err(),
        "Cloning to illegal destination path hierarchy must return Err without panicking"
    );
}

#[tokio::test]
async fn test_error_recovery_restore_nonexistent_snapshot_preserves_target() {
    let temp = ScopedTempDir::new("restore_nonexistent");
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
    assert!(result.is_err(), "Restoring non-existent snapshot must fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("Snapshot archive does not exist"),
        "Error message must specify archive non-existence: {}",
        err_str
    );

    // CRITICAL SAFETY CHECK: Target data must NOT be purged when snapshot does not exist
    assert!(
        valuable_file.exists(),
        "Target files must be preserved when snapshot archive is missing"
    );
    let content = fs::read(&valuable_file).unwrap();
    assert_eq!(content, b"CRITICAL_USER_DATA_DO_NOT_DELETE");
}

#[tokio::test]
async fn test_error_recovery_restore_corrupted_archive_fails_cleanly() {
    let temp = ScopedTempDir::new("restore_corrupted");
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
    assert!(
        result.is_err(),
        "Restoring corrupt snapshot archive must fail cleanly with Err"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Failed to extract snapshot archive") || err_msg.contains("status"),
        "Error message must report extraction failure: {}",
        err_msg
    );
}

#[test]
fn test_error_recovery_delete_nonexistent_snapshot_by_id() {
    let inst_id = Uuid::new_v4();
    let snap_id = Uuid::new_v4();
    let result = delete_snapshot_by_id(&inst_id, &snap_id);
    assert!(
        result.is_err(),
        "Deleting non-existent snapshot by ID must return Err"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

// =========================================================================
// 2. SOCKET & PORT EXHAUSTION SAFETY
// =========================================================================

#[test]
fn test_socket_exhaustion_and_port_overflow_fallback() {
    // 1. Boundary condition: Candidate ports up to u16::MAX
    // If ports list exhausts candidate search near u16::MAX, checked_add must prevent overflow panic.
    let mut high_ports = Vec::new();
    let mut p = 65525u16;
    loop {
        high_ports.push(p);
        match p.checked_add(ADB_PORT_STEP) {
            Some(next) => p = next,
            None => break,
        }
    }

    // Verify checked_add stepping does not panic
    let allocated = find_next_adb_port(&high_ports);
    assert!(
        allocated >= ADB_PORT_BASE,
        "Allocated port must be valid u16 port >= 5555"
    );
    assert_eq!(allocated % 2, 1, "ADB ports must always be odd");

    // 2. Active network stack socket collision detection
    // Bind a TCP listener on a test port to make it unavailable at OS level
    let test_port = 5601u16;
    let listener = TcpListener::bind(("127.0.0.1", test_port));
    if let Ok(_l) = listener {
        // Even if test_port is omitted from existing_ports, find_next_adb_port must detect
        // the OS socket collision and skip it!
        let chosen = find_next_adb_port(&[5555, 5557, 5559, 5561, 5563, 5565, 5567, 5569, 5571, 5573, 5575, 5577, 5579, 5581, 5583, 5585, 5587, 5589, 5591, 5593, 5595, 5597, 5599]);
        assert_ne!(
            chosen, test_port,
            "Port allocator must never allocate a port currently bound by a live socket"
        );
    }
}

#[test]
fn test_binder_index_exhaustion_and_fragmentation_resilience() {
    // Highly fragmented array with sparse indices
    let mut indices: Vec<u32> = (0..5000).collect();
    // Remove index 1337 and 4242
    indices.retain(|&x| x != 1337 && x != 4242);

    let next1 = find_next_binder_index(&indices);
    assert_eq!(next1, 1337, "Must allocate the lowest available hole at 1337");

    indices.push(1337);
    let next2 = find_next_binder_index(&indices);
    assert_eq!(next2, 4242, "Must allocate the next hole at 4242");

    indices.push(4242);
    let next3 = find_next_binder_index(&indices);
    assert_eq!(next3, 5000, "Must allocate next contiguous index at 5000");
}

// =========================================================================
// 3. ADB COMMAND STRING & ARGUMENT INJECTION SAFETY
// =========================================================================

#[test]
fn test_adb_argument_injection_safety_vectors() {
    let injection_payloads = [
        "; rm -rf / ;",
        "\" && reboot && \"",
        "' || cat /etc/passwd || '",
        "$(whoami)",
        "`id`",
        "\nreboot\n",
        "\r\nsetprop persist.sys.evil 1\r\n",
        "| nc -l 4444 -e /bin/sh",
        "> /dev/null; echo pwned",
        "--eval",
        "--system",
        "../../../../../../etc/shadow",
    ];

    for payload in &injection_payloads {
        // 1. Identity injection command args
        let mut identity = HardwareIdentity::galaxy_s24_ultra();
        identity.android_id = payload.to_string();
        identity.imei = payload.to_string();

        let target = "127.0.0.1:5555";
        let commands = build_identity_commands_args(target, &identity);

        assert_eq!(commands.len(), 3);
        for cmd in &commands {
            // Verify structure: ["-s", target, "shell", ...]
            assert_eq!(cmd[0], "-s");
            assert_eq!(cmd[1], target);
            assert_eq!(cmd[2], "shell");

            // Verify that the payload is contained strictly as a single argument element,
            // never concatenated or interpolated into a single string.
            let contains_exact_payload = cmd.iter().any(|arg| arg == payload);
            assert!(
                contains_exact_payload,
                "Payload must be preserved exactly as an individual argv item without shell concatenation"
            );
        }

        // 2. Proxy command args injection
        let proxy = ProxyConfig {
            enabled: true,
            protocol: "http".to_string(),
            host: payload.to_string(),
            port: 8080,
            username: None,
            password: None,
        };

        let proxy_cmd = build_proxy_command_args(target, &proxy);
        assert_eq!(proxy_cmd[0], "-s");
        assert_eq!(proxy_cmd[1], target);
        assert_eq!(proxy_cmd[2], "shell");
        assert_eq!(proxy_cmd[3], "settings");
        assert_eq!(proxy_cmd[4], "put");
        assert_eq!(proxy_cmd[5], "global");
        assert_eq!(proxy_cmd[6], "http_proxy");
        assert_eq!(proxy_cmd[7], format!("{}:8080", payload));
    }
}

#[test]
fn test_adb_connect_and_disconnect_arg_isolation() {
    let connect_args = build_adb_connect_args(5555);
    assert_eq!(connect_args.len(), 2);
    assert_eq!(connect_args[0], "connect");
    assert_eq!(connect_args[1], "127.0.0.1:5555");

    let disconnect_args = build_adb_disconnect_args(5557);
    assert_eq!(disconnect_args.len(), 2);
    assert_eq!(disconnect_args[0], "disconnect");
    assert_eq!(disconnect_args[1], "127.0.0.1:5557");
}

// =========================================================================
// 4. SCRCPY COMMAND FORMATTING & MODIFIER FLAGS
// =========================================================================

#[test]
fn test_scrcpy_command_formatting_and_modifier_flags() {
    let test_cases = [
        (5555u16, "Galaxy S24 Ultra", 120u32, 16u32),
        (5557u16, "Pixel 8 Pro (Dev)", 90u32, 8u32),
        (5559u16, "ROG Phone --fast & 'quoted'", 165u32, 24u32),
        (5561u16, "Custom / Device ; test", 60u32, 4u32),
    ];

    for (port, title, max_fps, bitrate) in test_cases {
        let args = build_scrcpy_args(port, title, max_fps, bitrate);

        // Verification of parameter count and exact sequence
        assert_eq!(args.len(), 10, "scrcpy args must have exactly 10 parameters");
        assert_eq!(args[0], "-s");
        assert_eq!(args[1], format!("127.0.0.1:{port}"));
        assert_eq!(args[2], "--window-title");
        assert_eq!(args[3], title);
        assert_eq!(args[4], "--max-fps");
        assert_eq!(args[5], max_fps.to_string());
        assert_eq!(args[6], "--video-bit-rate");
        assert_eq!(args[7], format!("{bitrate}M"));
        assert_eq!(args[8], "--shortcut-mod");
        assert_eq!(args[9], "lalt", "Shortcut modifier must be strictly 'lalt'");

        // Command builder check
        let cmd = build_scrcpy_command(port, title, max_fps, bitrate);
        assert_eq!(cmd.get_program(), "scrcpy");

        let extracted_args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert_eq!(extracted_args, args);
    }
}

#[test]
fn test_scrcpy_detached_stdio_configuration() {
    let cmd = build_scrcpy_command(5555, "Detached Instance", 60, 16);
    // Program must be scrcpy
    assert_eq!(cmd.get_program(), "scrcpy");
    // Verify that arguments match
    let args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();
    assert_eq!(args[8], "--shortcut-mod");
    assert_eq!(args[9], "lalt");
}

#[test]
fn test_spawn_scrcpy_for_instance_preset_title_formatting() {
    let instance = AndroidInstance {
        id: Uuid::new_v4(),
        name: "My Gaming Rig".to_string(),
        container_name: "cosmic-gaming".to_string(),
        android_version: AndroidVersion::Android14,
        image_tag: "redroid/redroid:14.0.0-latest".to_string(),
        preset: DevicePreset::AsusRogPhone8,
        identity: HardwareIdentity::asus_rog_phone_8(),
        proxy: ProxyConfig::default(),
        adb_port: 5559,
        data_path: PathBuf::from("/tmp/game_data"),
        binder_index: 2,
        status: InstanceStatus::Running,
    };

    let cmd = build_scrcpy_command(
        instance.adb_port,
        &format!("{} ({})", instance.name, instance.preset.display_name()),
        instance.identity.fps,
        16,
    );

    let args: Vec<String> = cmd.get_args().map(|s| s.to_string_lossy().to_string()).collect();
    assert_eq!(args[1], "127.0.0.1:5559");
    assert_eq!(args[3], "My Gaming Rig (ASUS ROG Phone 8)");
    assert_eq!(args[5], "165");
    assert_eq!(args[7], "16M");
    assert_eq!(args[8], "--shortcut-mod");
    assert_eq!(args[9], "lalt");
}

// =========================================================================
// 5. ATOMIC LOCAL.PROP WRITING & TEMP FILE CLEANUP
// =========================================================================

#[test]
fn test_atomic_local_prop_write_leaves_no_temp_files() {
    let temp = ScopedTempDir::new("local_prop_cleanup");
    let data_path = temp.path().join("android_data");

    let identity = populate_preset(DevicePreset::GalaxyS24Ultra);

    // Initial write
    let res = write_local_prop(&data_path, &identity);
    assert!(res.is_ok(), "write_local_prop must succeed");

    let target_file = data_path.join("local.prop");
    assert!(target_file.exists(), "local.prop must exist");

    // Verify no temporary files remain in data_path
    let entries = fs::read_dir(&data_path).unwrap();
    let mut file_names = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        file_names.push(name);
    }

    assert_eq!(
        file_names,
        vec!["local.prop"],
        "Only local.prop must exist; no .tmp files may remain: {:?}",
        file_names
    );

    // Overwrite write
    let identity2 = populate_preset(DevicePreset::Pixel8Pro);
    let res2 = write_local_prop(&data_path, &identity2);
    assert!(res2.is_ok(), "Overwriting local.prop must succeed");

    let entries2 = fs::read_dir(&data_path).unwrap();
    let mut file_names2 = Vec::new();
    for entry in entries2.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        file_names2.push(name);
    }

    assert_eq!(
        file_names2,
        vec!["local.prop"],
        "Only local.prop must exist after overwrite: {:?}",
        file_names2
    );

    let content = fs::read_to_string(&target_file).unwrap();
    assert!(content.contains(&format!("ro.product.model={}", identity2.model)));
    assert!(!content.contains(&format!("ro.product.model={}", identity.model)));
}

#[tokio::test]
async fn test_atomic_local_prop_async_write_clean() {
    let temp = ScopedTempDir::new("local_prop_async");
    let data_path = temp.path().join("async_data");

    let identity = populate_preset(DevicePreset::AsusRogPhone8);
    let res = write_local_prop_async(&data_path, &identity).await;
    assert!(res.is_ok(), "write_local_prop_async must succeed");

    let target_file = data_path.join("local.prop");
    assert!(target_file.exists());
    let content = fs::read_to_string(&target_file).unwrap();
    assert!(content.contains("ro.product.model=ASUS_AI2401_D"));
}

#[tokio::test]
async fn test_snapshot_lifecycle_cleans_archive_and_metadata_files() {
    let temp = ScopedTempDir::new("snap_clean");
    let data_path = temp.path().join("data");
    fs::create_dir_all(&data_path).unwrap();
    fs::write(data_path.join("file.txt"), b"test").unwrap();

    let inst_id = Uuid::new_v4();
    let snapshot = create_snapshot(&inst_id, &data_path, "Cleanliness Test").await;
    assert!(snapshot.is_ok(), "Snapshot creation must succeed");
    let info = snapshot.unwrap();

    assert!(info.archive_path.exists(), "Archive must exist");
    // Also check standard sidecar path
    let sidecar_json = info.archive_path.parent().unwrap().join(format!("{}.json", info.id));
    assert!(
        sidecar_json.exists(),
        "Sidecar JSON metadata must exist at {}",
        sidecar_json.display()
    );

    // Delete snapshot
    let del_res = delete_snapshot(&info);
    assert!(del_res.is_ok(), "delete_snapshot must succeed");

    assert!(
        !info.archive_path.exists(),
        "Archive must be deleted from filesystem"
    );
    assert!(
        !sidecar_json.exists(),
        "Sidecar JSON metadata must be deleted from filesystem"
    );

    // Cleanup snapshots dir
    if let Ok(snap_dir) = crate::config::snapshots_dir() {
        let inst_snap_dir = snap_dir.join(inst_id.to_string());
        let _ = fs::remove_dir_all(&inst_snap_dir);
    }
}

// =========================================================================
// 6. HOST HEALTH DIAGNOSTIC PROBES RESILIENCE
// =========================================================================

#[tokio::test]
async fn test_host_probes_individual_resilience() {
    // 1. Docker probe
    let (_docker_ok, docker_msg) = probe_docker().await;
    assert!(!docker_msg.is_empty(), "probe_docker must return diagnostic string");

    // 2. Binder probe
    let (_binder_ok, binder_msg) = probe_binder();
    assert!(!binder_msg.is_empty(), "probe_binder must return diagnostic string");

    // 3. GPU DRI probe
    let (_gpu_ok, gpu_msg) = probe_gpu_dri();
    assert!(!gpu_msg.is_empty(), "probe_gpu_dri must return diagnostic string");

    // 4. ADB probe
    let (_adb_ok, adb_msg) = probe_adb().await;
    assert!(!adb_msg.is_empty(), "probe_adb must return diagnostic string");

    // 5. scrcpy probe
    let (_scrcpy_ok, scrcpy_msg) = probe_scrcpy().await;
    assert!(!scrcpy_msg.is_empty(), "probe_scrcpy must return diagnostic string");

    // 6. Reflink probe and temporary file cleanup verification
    let (_reflink_ok, reflink_msg) = probe_reflink().await;
    assert!(!reflink_msg.is_empty(), "probe_reflink must return diagnostic string");

    // Verify probe_reflink left no orphaned .reflink_test_* files in root config directory
    let config_dir = crate::config::config_dir().unwrap_or_else(|_| std::env::temp_dir().join("cosmic-redroid"));
    if config_dir.exists() {
        for entry in fs::read_dir(&config_dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with(".reflink_test_"),
                "probe_reflink must not leave temporary probe files in config root: {}",
                name
            );
        }
    }

    // Verify isolated probe directory does not accumulate stale orphaned files
    let probe_dir = config_dir.join(".probe_reflink");
    if probe_dir.exists() {
        for entry in fs::read_dir(&probe_dir).unwrap().flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        assert!(
                            elapsed.as_secs() < 10,
                            "probe_reflink must not leave stale orphaned files in probe_dir: {:?}",
                            entry.file_name()
                        );
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_host_health_report_aggregation_and_remediation_flag() {
    let report = probe_host_health().await;
    assert_eq!(report.details.len(), 6, "Report must aggregate all 6 diagnostic probes");

    // If binder is unavailable or docker is unavailable, remediation must be needed
    if !report.binder_available || !report.docker_available {
        assert!(
            report.remediation_needed,
            "remediation_needed must be true when binder or docker is missing"
        );
    }

    let alias_report = run_host_diagnostics().await;
    assert_eq!(alias_report.details.len(), 6);
    assert_eq!(alias_report.docker_available, report.docker_available);
    assert_eq!(alias_report.binder_available, report.binder_available);
}

#[test]
fn test_remediation_script_shell_syntax_and_commands() {
    let script = generate_remediation_script();
    assert!(
        script.starts_with("#!/bin/bash\nset -e\n"),
        "Script must start with shebang and set -e"
    );
    assert!(script.contains("modprobe binder_linux"));
    assert!(script.contains("mount -t binder binder /dev/binderfs"));
    assert!(script.contains("chmod 666 /dev/binder"));
    assert!(script.contains("chmod 666 /dev/dri/renderD*"));
    assert!(script.contains("usermod -aG docker"));
    assert!(script.contains("usermod -aG render"));
}

// =========================================================================
// 7. EXHAUSTIVE ZERO EMOJI COMPLIANCE AUDIT
// =========================================================================

#[test]
fn test_strict_zero_emojis_entire_codebase() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scan_dirs = ["src", "tests", "resources", "i18n"];

    let emoji_ranges: &[(u32, u32)] = &[
        (0x1F600, 0x1F64F), // Emoticons
        (0x1F300, 0x1F5FF), // Misc Symbols and Pictographs
        (0x1F680, 0x1F6FF), // Transport and Map
        (0x1F700, 0x1F77F), // Alchemical Symbols
        (0x1F780, 0x1F7FF), // Geometric Shapes Extended
        (0x1F800, 0x1F8FF), // Supplemental Arrows-C
        (0x1F900, 0x1F9FF), // Supplemental Symbols and Pictographs
        (0x1FA00, 0x1FA6F), // Chess Symbols
        (0x1FA70, 0x1FAFF), // Symbols and Pictographs Extended-A
        (0x2600, 0x26FF),   // Miscellaneous Symbols
        (0x2700, 0x27BF),   // Dingbats
    ];

    let mut scanned_files = 0;
    for dir in &scan_dirs {
        let dir_path = root.join(dir);
        if !dir_path.exists() {
            continue;
        }

        let mut queue = vec![dir_path];
        while let Some(current_dir) = queue.pop() {
            if let Ok(entries) = fs::read_dir(&current_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        queue.push(path);
                    } else if path.is_file() {
                        if let Ok(content) = fs::read_to_string(&path) {
                            scanned_files += 1;
                            for (line_no, line) in content.lines().enumerate() {
                                for ch in line.chars() {
                                    let cp = ch as u32;
                                    for &(start, end) in emoji_ranges {
                                        assert!(
                                            !(cp >= start && cp <= end),
                                            "Zero emoji violation in {}:{}: char '{}' (U+{:04X})",
                                            path.display(),
                                            line_no + 1,
                                            ch,
                                            cp
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    assert!(scanned_files >= 10, "Audited at least 10 project files for emojis");
}
