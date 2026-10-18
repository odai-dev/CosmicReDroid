#[path = "../src/models.rs"]
pub mod models;

#[path = "../src/config.rs"]
pub mod config;

use config::{
    load_config_from_path, load_instances_from_path, save_instances_to_path,
    seed_default_instances, AppConfig,
};
use models::{
    AndroidInstance, AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus, NavPage,
    ProxyConfig,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_m2_{}_{}", prefix, Uuid::new_v4()));
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

// =========================================================================
// 1. DEFAULT SEEDING LOGIC EMPIRICAL CHALLENGES
// =========================================================================

#[test]
fn challenge_default_seeding_exact_specifications() {
    let defaults = seed_default_instances();
    assert_eq!(defaults.len(), 2, "Must seed exactly 2 default instances");

    // --- Instance 1: Galaxy S24 Ultra ---
    let inst1 = &defaults[0];
    assert_eq!(
        inst1.name, "Instance 1 - Galaxy S24 Ultra",
        "Exact name mismatch for instance 1"
    );
    assert_eq!(
        inst1.container_name, "cosmic-redroid-instance-1",
        "Container name mismatch for instance 1"
    );
    assert_eq!(inst1.adb_port, 5555, "ADB port mismatch for instance 1");
    assert_eq!(
        inst1.binder_index, 0,
        "Binder index mismatch for instance 1"
    );
    assert_eq!(
        inst1.binder_indices(),
        (1, 2, 3),
        "Binder formula (3*0+1, 3*0+2, 3*0+3) mismatch for instance 1"
    );
    assert_eq!(
        inst1.binder_nodes(),
        (
            "/dev/binder1".to_string(),
            "/dev/hwbinder2".to_string(),
            "/dev/vndbinder3".to_string(),
        ),
        "Binder nodes mismatch for instance 1"
    );
    assert_eq!(
        inst1.preset,
        DevicePreset::GalaxyS24Ultra,
        "Device preset mismatch for instance 1"
    );
    assert_eq!(
        inst1.android_version,
        AndroidVersion::Android13,
        "Android version mismatch for instance 1"
    );
    assert_eq!(
        inst1.image_tag, "redroid/redroid:13.0.0-latest",
        "Image tag mismatch for instance 1"
    );
    assert_eq!(
        inst1.status,
        InstanceStatus::Stopped,
        "Initial status must be Stopped for instance 1"
    );
    assert_eq!(
        inst1.adb_address(),
        "127.0.0.1:5555",
        "ADB address mismatch for instance 1"
    );

    // Galaxy S24 Ultra hardware identity resolution specifications
    assert_eq!(
        inst1.identity.model, "SM-S928B",
        "Galaxy S24 Ultra model mismatch"
    );
    assert_eq!(
        inst1.identity.manufacturer, "samsung",
        "Galaxy S24 Ultra manufacturer mismatch"
    );
    assert_eq!(
        inst1.identity.brand, "samsung",
        "Galaxy S24 Ultra brand mismatch"
    );
    assert_eq!(
        inst1.identity.width, 1440,
        "Galaxy S24 Ultra width mismatch"
    );
    assert_eq!(
        inst1.identity.height, 3120,
        "Galaxy S24 Ultra height mismatch"
    );
    assert_eq!(inst1.identity.dpi, 505, "Galaxy S24 Ultra DPI mismatch");
    assert_eq!(inst1.identity.fps, 120, "Galaxy S24 Ultra FPS mismatch");
    assert_eq!(
        inst1.identity.resolution_display(),
        "1440x3120 @ 120 FPS (505 DPI)"
    );

    // --- Instance 2: Pixel 8 Pro ---
    let inst2 = &defaults[1];
    assert_eq!(
        inst2.name, "Instance 2 - Pixel 8 Pro",
        "Exact name mismatch for instance 2"
    );
    assert_eq!(
        inst2.container_name, "cosmic-redroid-instance-2",
        "Container name mismatch for instance 2"
    );
    assert_eq!(inst2.adb_port, 5557, "ADB port mismatch for instance 2");
    assert_eq!(
        inst2.binder_index, 1,
        "Binder index mismatch for instance 2"
    );
    assert_eq!(
        inst2.binder_indices(),
        (4, 5, 6),
        "Binder formula (3*1+1, 3*1+2, 3*1+3) mismatch for instance 2"
    );
    assert_eq!(
        inst2.binder_nodes(),
        (
            "/dev/binder4".to_string(),
            "/dev/hwbinder5".to_string(),
            "/dev/vndbinder6".to_string(),
        ),
        "Binder nodes mismatch for instance 2"
    );
    assert_eq!(
        inst2.preset,
        DevicePreset::Pixel8Pro,
        "Device preset mismatch for instance 2"
    );
    assert_eq!(
        inst2.android_version,
        AndroidVersion::Android13,
        "Android version mismatch for instance 2"
    );
    assert_eq!(
        inst2.image_tag, "redroid/redroid:13.0.0-latest",
        "Image tag mismatch for instance 2"
    );
    assert_eq!(
        inst2.status,
        InstanceStatus::Stopped,
        "Initial status must be Stopped for instance 2"
    );
    assert_eq!(
        inst2.adb_address(),
        "127.0.0.1:5557",
        "ADB address mismatch for instance 2"
    );

    // Pixel 8 Pro hardware identity resolution specifications
    assert_eq!(
        inst2.identity.model, "Pixel 8 Pro",
        "Pixel 8 Pro model mismatch"
    );
    assert_eq!(
        inst2.identity.manufacturer, "Google",
        "Pixel 8 Pro manufacturer mismatch"
    );
    assert_eq!(
        inst2.identity.brand, "google",
        "Pixel 8 Pro brand mismatch"
    );
    assert_eq!(inst2.identity.width, 1344, "Pixel 8 Pro width mismatch");
    assert_eq!(inst2.identity.height, 2992, "Pixel 8 Pro height mismatch");
    assert_eq!(inst2.identity.dpi, 489, "Pixel 8 Pro DPI mismatch");
    assert_eq!(inst2.identity.fps, 120, "Pixel 8 Pro FPS mismatch");
    assert_eq!(
        inst2.identity.resolution_display(),
        "1344x2992 @ 120 FPS (489 DPI)"
    );

    // Distinct UUIDs and distinct data directories
    assert_ne!(
        inst1.id, inst2.id,
        "Default instance IDs must be unique v4 UUIDs"
    );
    assert_ne!(
        inst1.data_path, inst2.data_path,
        "Default instance data paths must be isolated"
    );
    assert!(
        inst1
            .data_path
            .ends_with(format!("instances/{}/data", inst1.id)),
        "Instance 1 data path must conform to instances/<uuid>/data"
    );
    assert!(
        inst2
            .data_path
            .ends_with(format!("instances/{}/data", inst2.id)),
        "Instance 2 data path must conform to instances/<uuid>/data"
    );
}

#[test]
fn challenge_seeding_on_empty_and_whitespace_only_files() {
    let temp_dir = TestTempDir::new("seeding_stress");
    let nonexistent = temp_dir.path().join("missing.json");
    let empty_file = temp_dir.path().join("empty.json");
    let whitespace_file = temp_dir.path().join("whitespace.json");

    // 1. Nonexistent file seeds defaults and writes them out
    let loaded1 = load_instances_from_path(&nonexistent).expect("load nonexistent");
    assert_eq!(loaded1.len(), 2);
    assert!(
        nonexistent.exists(),
        "Nonexistent path must be created on disk"
    );

    // 2. 0-byte file seeds defaults
    fs::write(&empty_file, b"").expect("write 0-byte file");
    let loaded2 = load_instances_from_path(&empty_file).expect("load 0-byte");
    assert_eq!(loaded2.len(), 2);
    assert_eq!(loaded2[0].name, "Instance 1 - Galaxy S24 Ultra");

    // 3. File containing only spaces, tabs, and newlines seeds defaults
    fs::write(&whitespace_file, "   \n\t  \r\n   ").expect("write whitespace file");
    let loaded3 = load_instances_from_path(&whitespace_file).expect("load whitespace");
    assert_eq!(loaded3.len(), 2);
    assert_eq!(loaded3[1].name, "Instance 2 - Pixel 8 Pro");
}

// =========================================================================
// 2. BACKWARD COMPATIBILITY EMPIRICAL CHALLENGES
// =========================================================================

#[test]
fn challenge_backward_compatibility_matrix() {
    let temp_dir = TestTempDir::new("compat_stress");
    let defaults = seed_default_instances();

    // Format A: Raw JSON array `[...]`
    let raw_array_path = temp_dir.path().join("legacy_array.json");
    let raw_array_json = serde_json::to_string_pretty(&defaults).expect("serialize array");
    fs::write(&raw_array_path, &raw_array_json).expect("write raw array");

    // Format B: Structured AppConfig `{ "instances": [...] }`
    let structured_path = temp_dir.path().join("structured_config.json");
    let config_obj = AppConfig {
        instances: defaults.clone(),
        active_page: Some(NavPage::Snapshots),
    };
    let structured_json = serde_json::to_string_pretty(&config_obj).expect("serialize structured");
    fs::write(&structured_path, &structured_json).expect("write structured");

    // Format C: Structured AppConfig without active_page `{ "instances": [...] }`
    let minimal_structured_path = temp_dir.path().join("minimal_structured.json");
    let minimal_json = format!(
        "{{\"instances\": {}}}",
        serde_json::to_string(&defaults).unwrap()
    );
    fs::write(&minimal_structured_path, &minimal_json).expect("write minimal structured");

    // Format D: Structured AppConfig with forward-compatible unknown fields
    let future_structured_path = temp_dir.path().join("future_structured.json");
    let future_json = format!(
        "{{\"instances\": {}, \"active_page\": \"HostHealth\", \"unknown_future_field\": 42, \"dark_mode\": true}}",
        serde_json::to_string(&defaults).unwrap()
    );
    fs::write(&future_structured_path, &future_json).expect("write future structured");

    // --- Verify reading with `load_instances_from_path` ---
    let insts_from_a = load_instances_from_path(&raw_array_path).expect("read A as instances");
    assert_eq!(insts_from_a.len(), 2);
    assert_eq!(insts_from_a[0].adb_port, 5555);

    let insts_from_b = load_instances_from_path(&structured_path).expect("read B as instances");
    assert_eq!(insts_from_b.len(), 2);
    assert_eq!(insts_from_b[1].adb_port, 5557);

    let insts_from_c =
        load_instances_from_path(&minimal_structured_path).expect("read C as instances");
    assert_eq!(insts_from_c.len(), 2);

    let insts_from_d =
        load_instances_from_path(&future_structured_path).expect("read D as instances");
    assert_eq!(insts_from_d.len(), 2);

    // --- Verify reading with `load_config_from_path` ---
    let cfg_from_a = load_config_from_path(&raw_array_path).expect("read A as config");
    assert_eq!(cfg_from_a.instances.len(), 2);
    assert_eq!(
        cfg_from_a.active_page,
        Some(NavPage::Instances),
        "Legacy array format defaults active_page to Instances"
    );

    let cfg_from_b = load_config_from_path(&structured_path).expect("read B as config");
    assert_eq!(cfg_from_b.instances.len(), 2);
    assert_eq!(cfg_from_b.active_page, Some(NavPage::Snapshots));

    let cfg_from_c = load_config_from_path(&minimal_structured_path).expect("read C as config");
    assert_eq!(cfg_from_c.instances.len(), 2);
    assert_eq!(cfg_from_c.active_page, None);

    let cfg_from_d = load_config_from_path(&future_structured_path).expect("read D as config");
    assert_eq!(cfg_from_d.instances.len(), 2);
    assert_eq!(cfg_from_d.active_page, Some(NavPage::HostHealth));

    // --- Verify empty array behavior `[]` ---
    // If user explicitly deleted all instances, saving `[]` must not be re-seeded over
    let empty_array_path = temp_dir.path().join("empty_array.json");
    fs::write(&empty_array_path, "[]").expect("write empty array");

    let loaded_empty_insts =
        load_instances_from_path(&empty_array_path).expect("load empty array");
    assert_eq!(
        loaded_empty_insts.len(),
        0,
        "Explicit empty array '[]' must preserve empty collection"
    );

    let loaded_empty_cfg = load_config_from_path(&empty_array_path).expect("load empty as cfg");
    assert_eq!(loaded_empty_cfg.instances.len(), 0);

    // --- Verify corrupted JSON returns proper error and does not panic ---
    let corrupted_path = temp_dir.path().join("corrupted.json");
    fs::write(&corrupted_path, "{ broken_json: [ }").expect("write corrupted");
    assert!(
        load_instances_from_path(&corrupted_path).is_err(),
        "Corrupted JSON must return Err"
    );
    assert!(
        load_config_from_path(&corrupted_path).is_err(),
        "Corrupted JSON must return Err"
    );
}

// =========================================================================
// 3. ATOMIC WRITE CRASH SAFETY SIMULATION
// =========================================================================

#[test]
fn challenge_crash_safety_dangling_temp_files_isolation() {
    let temp_dir = TestTempDir::new("crash_safety_dangling");
    let target_file = temp_dir.path().join("instances.json");

    // 1. Initial valid write
    let initial_seeds = seed_default_instances();
    save_instances_to_path(&initial_seeds, &target_file).expect("initial save");
    assert!(target_file.exists());

    // 2. Simulate crashed processes that died mid-write leaving truncated or garbage temp files
    let stray_temp1 = temp_dir
        .path()
        .join(".instances.json.tmp.99999_ab12cd34ef56");
    let stray_temp2 = temp_dir
        .path()
        .join(".instances.json.tmp.88888_001122334455");
    fs::write(&stray_temp1, b"{\"instances\": [{\"name\": \"half-wri")
        .expect("write half-written temp");
    fs::write(&stray_temp2, b"CORRUPTED_SYSTEM_CRASH_GARBAGE_BYTES")
        .expect("write garbage temp");

    // 3. Load should read the intact target file and ignore stray temp files
    let loaded = load_instances_from_path(&target_file).expect("load during dangling temps");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "Instance 1 - Galaxy S24 Ultra");

    // 4. Save should succeed without being impeded by existing dangling temp files
    let mut updated = loaded.clone();
    updated[0].name = "Updated S24 Ultra".to_string();
    save_instances_to_path(&updated, &target_file).expect("save with dangling temps present");

    let reloaded = load_instances_from_path(&target_file).expect("reload");
    assert_eq!(reloaded[0].name, "Updated S24 Ultra");

    // Stray temp files remain untouched and isolated
    assert!(stray_temp1.exists());
    assert!(stray_temp2.exists());
}

#[test]
fn challenge_crash_safety_nested_directory_creation() {
    let temp_dir = TestTempDir::new("nested_dir_creation");
    let deep_nested = temp_dir
        .path()
        .join("sub1")
        .join("sub2")
        .join("sub3")
        .join("instances.json");

    assert!(!deep_nested.parent().unwrap().exists());

    let instances = seed_default_instances();
    save_instances_to_path(&instances, &deep_nested).expect("save to deep nested path");
    assert!(
        deep_nested.exists(),
        "Target file must exist in created directory structure"
    );

    let loaded = load_instances_from_path(&deep_nested).expect("load from deep nested path");
    assert_eq!(loaded.len(), 2);
}

#[test]
fn challenge_concurrent_read_write_atomic_integrity() {
    let temp_dir = TestTempDir::new("concurrent_stress");
    let target_path = Arc::new(temp_dir.path().join("instances_concurrent.json"));

    // Seed file
    let base_instances = seed_default_instances();
    save_instances_to_path(&base_instances, &target_path).expect("initial save");

    let num_writer_threads = 8;
    let num_reader_threads = 12;
    let iterations_per_thread = 50;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let read_successes = Arc::new(AtomicUsize::new(0));
    let write_successes = Arc::new(AtomicUsize::new(0));
    let read_corruptions = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    // Spawn writers
    for writer_id in 0..num_writer_threads {
        let path_clone = Arc::clone(&target_path);
        let write_ok_clone = Arc::clone(&write_successes);
        let stop_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            for i in 0..iterations_per_thread {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }
                let port = 6000 + (writer_id * 100 + i) as u16;
                let test_instances = vec![AndroidInstance {
                    id: Uuid::new_v4(),
                    name: format!("Concurrent Instance {writer_id}-{i}"),
                    container_name: format!("cosmic-test-{writer_id}-{i}"),
                    android_version: AndroidVersion::Android13,
                    image_tag: "redroid/redroid:13.0.0-latest".to_string(),
                    preset: DevicePreset::Pixel8Pro,
                    identity: HardwareIdentity::pixel_8_pro(),
                    proxy: ProxyConfig::default(),
                    adb_port: port,
                    data_path: PathBuf::from(format!("/tmp/data_{writer_id}_{i}")),
                    binder_index: (writer_id as u32 % 10),
                    status: InstanceStatus::Running,
                }];

                match save_instances_to_path(&test_instances, &path_clone) {
                    Ok(_) => {
                        write_ok_clone.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("Writer {writer_id} failed: {e:?}");
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        handles.push(handle);
    }

    // Spawn readers
    for reader_id in 0..num_reader_threads {
        let path_clone = Arc::clone(&target_path);
        let read_ok_clone = Arc::clone(&read_successes);
        let corruptions_clone = Arc::clone(&read_corruptions);
        let stop_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                match load_instances_from_path(&path_clone) {
                    Ok(instances) => {
                        // In an atomic system, any successfully read instance list must be valid
                        if instances.is_empty() {
                            corruptions_clone.fetch_add(1, Ordering::SeqCst);
                            eprintln!("Reader {reader_id} observed empty instances list!");
                        } else {
                            read_ok_clone.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(err) => {
                        // A torn read would manifest as a JSON parsing error
                        corruptions_clone.fetch_add(1, Ordering::SeqCst);
                        eprintln!(
                            "CRITICAL: Reader {reader_id} observed torn read/corruption: {err:?}"
                        );
                    }
                }
                thread::sleep(Duration::from_millis(1));
            }
        });
        handles.push(handle);
    }

    // Wait for writers to complete
    for handle in handles.drain(..num_writer_threads) {
        handle.join().expect("writer thread panicked");
    }

    // Signal readers to finish and wait for them
    stop_flag.store(true, Ordering::SeqCst);
    for handle in handles {
        handle.join().expect("reader thread panicked");
    }

    let final_writes = write_successes.load(Ordering::SeqCst);
    let final_reads = read_successes.load(Ordering::SeqCst);
    let final_corruptions = read_corruptions.load(Ordering::SeqCst);

    println!(
        "Concurrent stress test completed: {} writes, {} reads, {} corruptions",
        final_writes, final_reads, final_corruptions
    );

    assert_eq!(
        final_corruptions, 0,
        "Atomic rename guarantee violated: readers observed torn reads or corruption!"
    );
    assert!(
        final_writes >= (num_writer_threads * iterations_per_thread) * 9 / 10,
        "At least 90% of writes should succeed under heavy concurrency"
    );
    assert!(
        final_reads > 100,
        "Readers should complete many successful concurrent reads"
    );
}

// =========================================================================
// 4. ZERO EMOJI COMPLIANCE CHALLENGE
// =========================================================================

#[test]
fn challenge_zero_emojis_across_entire_repo() {
    // Unicode ranges covering emoticons, dingbats, transport, supplemental symbols, etc.
    let emoji_ranges: &[(u32, u32)] = &[
        (0x1F600, 0x1F64F), // Emoticons
        (0x1F300, 0x1F5FF), // Misc Symbols and Pictographs
        (0x1F680, 0x1F6FF), // Transport and Map
        (0x1F1E0, 0x1F1FF), // Regional Indicator Symbols (flags)
        (0x2702, 0x27B0),   // Dingbats
        (0x1F900, 0x1F9FF), // Supplemental Symbols and Pictographs
        (0x1FA70, 0x1FAFF), // Symbols and Pictographs Extended-A
        (0x2600, 0x26FF),   // Misc Symbols (sun, umbrella, warning, etc.)
    ];

    let check_emoji = |ch: char| -> bool {
        let code = ch as u32;
        emoji_ranges
            .iter()
            .any(|(start, end)| code >= *start && code <= *end)
    };

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files_to_scan = Vec::new();

    // Scan src, resources, i18n, Cargo.toml, justfile, i18n.toml
    let scan_dirs = [
        manifest_dir.join("src"),
        manifest_dir.join("resources"),
        manifest_dir.join("i18n"),
    ];

    for dir in &scan_dirs {
        if dir.exists() {
            for entry in walkdir(dir) {
                if entry.is_file() {
                    files_to_scan.push(entry);
                }
            }
        }
    }

    files_to_scan.push(manifest_dir.join("Cargo.toml"));
    files_to_scan.push(manifest_dir.join("justfile"));
    files_to_scan.push(manifest_dir.join("i18n.toml"));

    let mut violations = Vec::new();

    for file_path in &files_to_scan {
        if !file_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue, // binary files (e.g. icons, fonts) are skipped
        };

        for (line_idx, line) in content.lines().enumerate() {
            for ch in line.chars() {
                if check_emoji(ch) {
                    violations.push(format!(
                        "{}:{}: Found prohibited emoji U+{:04X} ('{}')",
                        file_path.display(),
                        line_idx + 1,
                        ch as u32,
                        ch
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Zero-emoji violation found!\n{}",
        violations.join("\n")
    );
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}
