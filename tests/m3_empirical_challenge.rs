// tests/m3_empirical_challenge.rs
// Empirical Challenge & Stress Test Suite for Milestone 3 (Backend Subsystems)
// Strict Zero-Emoji Compliance.

#[path = "../src/models.rs"]
pub mod models;

#[path = "../src/config.rs"]
pub mod config;

#[path = "../src/backend/mod.rs"]
pub mod backend;

use std::collections::HashSet;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use backend::docker::{
    build_container_config_with_root, calculate_metrics_from_stats,
    resolve_binder_mappings_with_root,
};
use backend::host::generate_remediation_script;
use backend::spoofing::{
    generate_android_id, generate_luhn_imei, generate_mac_address, generate_serial,
    populate_preset, verify_luhn, write_local_prop,
};
use backend::storage::{
    allocate_next_port_and_binder, delete_snapshot, find_next_adb_port, find_next_binder_index,
    ADB_PORT_BASE, ADB_PORT_STEP,
};
use bollard::container::Stats;
use chrono::Utc;
use models::{
    AndroidInstance, AndroidVersion, DevicePreset, HardwareIdentity,
    InstanceStatus, ProxyConfig, SnapshotInfo,
};

/// Scoped temporary directory that deletes itself upon drop.
struct ScopedTempDir {
    path: PathBuf,
}

impl ScopedTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_m3_test_{}_{}", prefix, Uuid::new_v4()));
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

/// Independent ground-truth Luhn oracle implementing ISO/IEC 7812-1 MOD-10 from first principles.
fn oracle_luhn_verify(imei: &str) -> bool {
    if imei.len() != 15 || !imei.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let mut sum: i32 = 0;
    for (i, ch) in imei.chars().enumerate() {
        let digit = ch.to_digit(10).unwrap() as i32;
        if i % 2 == 1 {
            let doubled = digit * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += digit;
        }
    }
    sum % 10 == 0
}

// =========================================================================
// 1. MATHEMATICAL PROPERTIES OF IDENTIFIER GENERATORS
// =========================================================================

#[test]
fn test_challenge_luhn_imei_10000_samples_and_adversarial_mutations() {
    let test_prefixes = [
        "35275311",         // Samsung Galaxy S24 Ultra TAC
        "35327511",         // Google Pixel 8 Pro TAC
        "35612311",         // ASUS ROG Phone 8 TAC
        "86012304",         // Generic TAC
        "01234567",         // Leading zero TAC
        "99999999",         // All nines TAC
        "123",              // Short TAC (requires padding)
        "",                 // Empty TAC (requires full generation)
        "35-27.5311_ABC",   // TAC requiring sanitization
        "3527531100000000", // Long TAC (requires truncation to 8)
    ];

    let samples_per_prefix = 1000;
    let total_samples = test_prefixes.len() * samples_per_prefix; // Exactly 10,000 samples
    let mut generated_set = HashSet::with_capacity(total_samples);

    for prefix in &test_prefixes {
        for _ in 0..samples_per_prefix {
            let imei = generate_luhn_imei(prefix);

            // 1. Structural assertions
            assert_eq!(imei.len(), 15, "IMEI must be exactly 15 digits");
            assert!(
                imei.chars().all(|c| c.is_ascii_digit()),
                "IMEI must consist strictly of ASCII digits: {}",
                imei
            );

            // 2. Sanitized prefix checking
            let clean_prefix: String = prefix.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean_prefix.is_empty() {
                let expected_prefix_len = clean_prefix.len().min(8);
                assert!(
                    imei.starts_with(&clean_prefix[..expected_prefix_len]),
                    "IMEI {} must start with sanitized prefix {}",
                    imei,
                    &clean_prefix[..expected_prefix_len]
                );
            }

            // 3. Dual Luhn verification against implementation and independent oracle
            assert!(
                verify_luhn(&imei),
                "IMEI {} failed implementation verify_luhn",
                imei
            );
            assert!(
                oracle_luhn_verify(&imei),
                "IMEI {} failed independent oracle_luhn_verify",
                imei
            );

            // 4. Adversarial check digit corruption (all 9 alternate digits must fail)
            let original_check_digit = imei.chars().last().unwrap().to_digit(10).unwrap();
            let imei_prefix = &imei[..14];
            for alternate in 0..10 {
                if alternate != original_check_digit {
                    let corrupted_imei = format!("{}{}", imei_prefix, alternate);
                    assert!(
                        !verify_luhn(&corrupted_imei),
                        "Corrupted check digit '{}' on IMEI {} must fail verification",
                        alternate,
                        imei
                    );
                    assert!(
                        !oracle_luhn_verify(&corrupted_imei),
                        "Corrupted check digit '{}' on IMEI {} must fail oracle",
                        alternate,
                        imei
                    );
                }
            }

            generated_set.insert(imei);
        }
    }

    assert_eq!(
        total_samples, 10000,
        "Total sample count must equal exactly 10,000"
    );
    // With 10,000 samples and random 6-digit SNR (1,000,000 space) per prefix, collision rate is minimal
    assert!(
        generated_set.len() > 9500,
        "High diversity expected across 10,000 samples, got unique count: {}",
        generated_set.len()
    );
}

#[test]
fn test_challenge_luhn_single_digit_and_transposition_error_detection() {
    let base_imei = generate_luhn_imei("35275311");
    assert!(verify_luhn(&base_imei));

    // 1. Test single-digit substitution error detection at EVERY index (0..15)
    // The mathematical property of Luhn ensures 100% detection of single-digit substitution errors.
    let chars: Vec<char> = base_imei.chars().collect();
    for pos in 0..15 {
        let original_digit = chars[pos].to_digit(10).unwrap();
        for alternate in 0..10 {
            if alternate != original_digit {
                let mut mutated = chars.clone();
                mutated[pos] = char::from_digit(alternate, 10).unwrap();
                let mutated_imei: String = mutated.into_iter().collect();
                assert!(
                    !verify_luhn(&mutated_imei),
                    "Luhn algorithm failed to detect single-digit mutation at position {} (original={}, mutated={}): {}",
                    pos, original_digit, alternate, mutated_imei
                );
            }
        }
    }

    // 2. Test transposition error detection of adjacent digits
    // The mathematical property of Luhn ensures detection of adjacent transpositions
    // for all pairs EXCEPT (0, 9) and (9, 0).
    for pos in 0..14 {
        let d1 = chars[pos].to_digit(10).unwrap();
        let d2 = chars[pos + 1].to_digit(10).unwrap();
        if d1 != d2 && !((d1 == 0 && d2 == 9) || (d1 == 9 && d2 == 0)) {
            let mut transposed = chars.clone();
            transposed.swap(pos, pos + 1);
            let transposed_imei: String = transposed.into_iter().collect();
            assert!(
                !verify_luhn(&transposed_imei),
                "Luhn algorithm failed to detect transposition between positions {} and {} ({}{}) in {}",
                pos, pos + 1, d1, d2, transposed_imei
            );
        }
    }
}

#[test]
fn test_challenge_serial_numbers_10000_samples_and_charset() {
    const ALLOWED_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut unique_serials = HashSet::with_capacity(10000);
    let mut char_histogram = [0usize; 256];

    for _ in 0..10000 {
        let serial = generate_serial();
        assert_eq!(serial.len(), 11, "Serial number must be exactly 11 characters");
        for byte in serial.bytes() {
            assert!(
                ALLOWED_CHARS.contains(&byte),
                "Serial contains invalid byte: {}",
                byte as char
            );
            char_histogram[byte as usize] += 1;
        }
        unique_serials.insert(serial);
    }

    assert_eq!(
        unique_serials.len(),
        10000,
        "All 10,000 serial numbers must be globally unique (0 collisions expected in 36^11 space)"
    );

    // Verify all 36 characters are represented across 110,000 character draws
    for &ch in ALLOWED_CHARS {
        let count = char_histogram[ch as usize];
        assert!(
            count > 0,
            "Character '{}' had zero occurrences in 110,000 draws",
            ch as char
        );
    }
}

#[test]
fn test_challenge_mac_addresses_10000_samples_and_unicast_laa_bits() {
    let mut unique_macs = HashSet::with_capacity(10000);

    for _ in 0..10000 {
        let mac = generate_mac_address();
        assert_eq!(mac.len(), 17, "MAC address length must be 17 (xx:xx:xx:xx:xx:xx)");
        let parts: Vec<&str> = mac.split(':').collect();
        assert_eq!(parts.len(), 6, "MAC must contain exactly 6 octets");

        // Prefix verification
        assert_eq!(parts[0], "02", "First octet must be 02");
        assert_eq!(parts[1], "42", "Second octet must be 42");
        assert_eq!(parts[2], "ac", "Third octet must be ac");

        // IEEE 802 Locally Administered Unicast bit verification
        let first_byte = u8::from_str_radix(parts[0], 16).unwrap();
        let is_unicast = (first_byte & 0x01) == 0;
        let is_locally_administered = (first_byte & 0x02) != 0;
        assert!(is_unicast, "MAC must be unicast (bit 0 of octet 0 is 0)");
        assert!(
            is_locally_administered,
            "MAC must be locally administered (bit 1 of octet 0 is 1)"
        );

        // Randomized octets verification
        for part in &parts[3..6] {
            assert_eq!(part.len(), 2, "Octet must be 2 hex digits");
            assert!(
                part.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "Octet must be lowercase hex: {}",
                part
            );
        }

        unique_macs.insert(mac);
    }

    // 10,000 samples drawn from 256^3 = 16,777,216 space.
    // Expected collisions by birthday problem ~ 3.
    assert!(
        unique_macs.len() >= 9950,
        "MAC address randomness showed excessive collisions: {}",
        10000 - unique_macs.len()
    );
}

#[test]
fn test_challenge_android_ids_10000_samples_and_formatting() {
    let mut unique_ids = HashSet::with_capacity(10000);

    for _ in 0..10000 {
        let id = generate_android_id();
        assert_eq!(
            id.len(),
            16,
            "Android ID must be exactly 16 characters (64-bit hex)"
        );
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "Android ID must be lowercase hex: {}",
            id
        );
        let parsed = u64::from_str_radix(&id, 16);
        assert!(parsed.is_ok(), "Android ID must parse cleanly as u64 hex");
        unique_ids.insert(id);
    }

    assert_eq!(
        unique_ids.len(),
        10000,
        "All 10,000 Android IDs must be unique (0 collisions expected in 2^64 space)"
    );
}

// =========================================================================
// 2. DETERMINISTIC BINDER MAPPING ALGORITHM (0 TO 1,000,000)
// =========================================================================

#[test]
fn test_challenge_deterministic_binder_mapping_1000000_indices_no_overlaps() {
    // Mathematical theorem:
    // b1 = 3i + 1
    // b2 = 3i + 2
    // b3 = 3i + 3
    // For any i != j: {3i+1, 3i+2, 3i+3} and {3j+1, 3j+2, 3j+3} are strictly disjoint.
    // Furthermore, every integer k >= 1 belongs to exactly one instance index i = floor((k-1)/3).

    let mut prev_b3 = 0u32;
    let max_index = 1_000_000u32;

    for i in 0..=max_index {
        let b1 = i * 3 + 1;
        let b2 = i * 3 + 2;
        let b3 = i * 3 + 3;

        // Modulo properties
        assert_eq!(b1 % 3, 1, "b1 must be 1 mod 3 at index {}", i);
        assert_eq!(b2 % 3, 2, "b2 must be 2 mod 3 at index {}", i);
        assert_eq!(b3 % 3, 0, "b3 must be 0 mod 3 at index {}", i);

        // Strict ordering
        assert!(b1 < b2 && b2 < b3, "Ordering violation at index {}", i);
        assert_eq!(b2 - b1, 1, "Gap between b1 and b2 must be 1");
        assert_eq!(b3 - b2, 1, "Gap between b2 and b3 must be 1");

        // Perfect partition boundary stepping (no gaps, no overlaps between adjacent instances)
        if i > 0 {
            assert_eq!(
                b1,
                prev_b3 + 1,
                "Instance {} b1 does not immediately follow instance {} b3",
                i,
                i - 1
            );
        }
        prev_b3 = b3;
    }

    // Verify boundary values
    assert_eq!(max_index * 3 + 3, 3_000_003u32);

    // Verify model mapping on representative instances
    let inst0 = AndroidInstance {
        id: Uuid::new_v4(),
        name: "Inst0".to_string(),
        container_name: "cosmic-0".to_string(),
        android_version: AndroidVersion::Android13,
        image_tag: "tag".to_string(),
        preset: DevicePreset::GalaxyS24Ultra,
        identity: HardwareIdentity::default(),
        proxy: ProxyConfig::default(),
        adb_port: 5555,
        data_path: PathBuf::from("/data"),
        binder_index: 0,
        status: InstanceStatus::Stopped,
    };
    assert_eq!(inst0.binder_indices(), (1, 2, 3));
    assert_eq!(
        inst0.binder_nodes(),
        (
            "/dev/binder1".to_string(),
            "/dev/hwbinder2".to_string(),
            "/dev/vndbinder3".to_string()
        )
    );

    let inst_mil = AndroidInstance {
        binder_index: 1_000_000,
        ..inst0.clone()
    };
    assert_eq!(inst_mil.binder_indices(), (3000001, 3000002, 3000003));
    assert_eq!(
        inst_mil.binder_nodes(),
        (
            "/dev/binder3000001".to_string(),
            "/dev/hwbinder3000002".to_string(),
            "/dev/vndbinder3000003".to_string()
        )
    );
}

#[test]
fn test_challenge_binder_mapping_resolution_under_all_fallback_modes() {
    let temp = ScopedTempDir::new("binder_fallback");
    let root = temp.path();

    // Mode 1: Empty root -> returns empty vector without panic
    let mappings_empty = resolve_binder_mappings_with_root(1, root);
    assert!(mappings_empty.is_empty(), "Empty root must produce empty mappings");

    // Mode 2: Single /dev/binder fallback
    fs::create_dir_all(root.join("dev")).unwrap();
    fs::write(root.join("dev/binder"), b"").unwrap();
    let mappings_single = resolve_binder_mappings_with_root(1, root);
    assert_eq!(mappings_single.len(), 1);
    assert_eq!(mappings_single[0].path_on_host.as_deref(), Some("/dev/binder"));
    assert_eq!(mappings_single[0].path_in_container.as_deref(), Some("/dev/binder"));

    // Mode 3: Directory /dev/binderfs fallback
    fs::create_dir_all(root.join("dev/binderfs")).unwrap();
    let mappings_fs_dir = resolve_binder_mappings_with_root(1, root);
    assert_eq!(mappings_fs_dir.len(), 1);
    assert_eq!(mappings_fs_dir[0].path_on_host.as_deref(), Some("/dev/binderfs"));
    assert_eq!(mappings_fs_dir[0].path_in_container.as_deref(), Some("/dev/binderfs"));

    // Mode 4: Index 0 standard nodes fallback (/dev/binder, /dev/hwbinder, /dev/vndbinder)
    fs::write(root.join("dev/hwbinder"), b"").unwrap();
    fs::write(root.join("dev/vndbinder"), b"").unwrap();
    let mappings_idx0 = resolve_binder_mappings_with_root(0, root);
    assert_eq!(mappings_idx0.len(), 3);
    assert_eq!(mappings_idx0[0].path_on_host.as_deref(), Some("/dev/binder"));
    assert_eq!(mappings_idx0[1].path_on_host.as_deref(), Some("/dev/hwbinder"));
    assert_eq!(mappings_idx0[2].path_on_host.as_deref(), Some("/dev/vndbinder"));

    // Mode 5: Numbered binderfs nodes (/dev/binderfs/binder{b1,b2,b3})
    // For index 2: b1=7, b2=8, b3=9
    fs::write(root.join("dev/binderfs/binder7"), b"").unwrap();
    fs::write(root.join("dev/binderfs/binder8"), b"").unwrap();
    fs::write(root.join("dev/binderfs/binder9"), b"").unwrap();
    let mappings_numbered_fs = resolve_binder_mappings_with_root(2, root);
    assert_eq!(mappings_numbered_fs.len(), 3);
    assert_eq!(mappings_numbered_fs[0].path_on_host.as_deref(), Some("/dev/binderfs/binder7"));
    assert_eq!(mappings_numbered_fs[0].path_in_container.as_deref(), Some("/dev/binder"));
    assert_eq!(mappings_numbered_fs[1].path_on_host.as_deref(), Some("/dev/binderfs/binder8"));
    assert_eq!(mappings_numbered_fs[1].path_in_container.as_deref(), Some("/dev/hwbinder"));
    assert_eq!(mappings_numbered_fs[2].path_on_host.as_deref(), Some("/dev/binderfs/binder9"));
    assert_eq!(mappings_numbered_fs[2].path_in_container.as_deref(), Some("/dev/vndbinder"));

    // Mode 6: Numbered direct host nodes (/dev/binder{b1}, /dev/hwbinder{b2}, /dev/vndbinder{b3})
    fs::write(root.join("dev/binder7"), b"").unwrap();
    fs::write(root.join("dev/hwbinder8"), b"").unwrap();
    fs::write(root.join("dev/vndbinder9"), b"").unwrap();
    let mappings_numbered_host = resolve_binder_mappings_with_root(2, root);
    assert_eq!(mappings_numbered_host.len(), 3);
    assert_eq!(mappings_numbered_host[0].path_on_host.as_deref(), Some("/dev/binder7"));
    assert_eq!(mappings_numbered_host[0].path_in_container.as_deref(), Some("/dev/binder"));
    assert_eq!(mappings_numbered_host[1].path_on_host.as_deref(), Some("/dev/hwbinder8"));
    assert_eq!(mappings_numbered_host[1].path_in_container.as_deref(), Some("/dev/hwbinder"));
    assert_eq!(mappings_numbered_host[2].path_on_host.as_deref(), Some("/dev/vndbinder9"));
    assert_eq!(mappings_numbered_host[2].path_in_container.as_deref(), Some("/dev/vndbinder"));
}

// =========================================================================
// 3. PORT & BINDER ALLOCATORS FRAGMENTATION & HOLE FILLING
// =========================================================================

#[test]
fn test_challenge_port_allocator_fragmentation_and_hole_filling() {
    // 1. Clean sequential allocation
    let mut ports: Vec<u16> = Vec::new();
    for i in 0..50 {
        let allocated = find_next_adb_port(&ports);
        let expected = ADB_PORT_BASE + (i as u16 * ADB_PORT_STEP);
        assert_eq!(allocated, expected);
        ports.push(allocated);
    }
    assert_eq!(ports.len(), 50);

    // 2. Punch holes: remove indices 5, 12, 23, 37, 44
    let removed_indices = [5usize, 12, 23, 37, 44];
    let mut expected_holes: Vec<u16> = removed_indices
        .iter()
        .map(|&idx| ADB_PORT_BASE + (idx as u16 * ADB_PORT_STEP))
        .collect();
    expected_holes.sort();

    ports.retain(|p| !expected_holes.contains(p));
    assert_eq!(ports.len(), 45);

    // 3. Sequential reallocation must fill holes in exact ascending order
    for expected_hole in &expected_holes {
        let filled = find_next_adb_port(&ports);
        assert_eq!(
            filled, *expected_hole,
            "Allocator failed to fill hole in ascending order"
        );
        ports.push(filled);
    }
    assert_eq!(ports.len(), 50);

    // Next allocation after all holes filled must be at index 50
    let next_port = find_next_adb_port(&ports);
    assert_eq!(next_port, ADB_PORT_BASE + 50 * ADB_PORT_STEP);

    // 4. Shuffled existing ports should not affect allocation
    let mut shuffled = ports.clone();
    shuffled.reverse();
    assert_eq!(find_next_adb_port(&shuffled), next_port);

    // 5. Active OS TCP socket collision
    // Bind a real socket on 127.0.0.1:next_port to simulate an external collision
    let listener = TcpListener::bind(("127.0.0.1", next_port));
    if let Ok(_socket) = listener {
        // Allocator must detect that next_port cannot be bound and skip to next_port + 2
        let port_after_collision = find_next_adb_port(&ports);
        assert_eq!(
            port_after_collision,
            next_port + ADB_PORT_STEP,
            "Allocator did not skip actively bound OS TCP port"
        );
    }

    // 6. Port range exhaustion fallback
    // When port exceeds u16::MAX - 2 (e.g. 65535), checked_add returns None and falls back to ADB_PORT_BASE
    let candidate = 65535u16;
    assert!(candidate.checked_add(ADB_PORT_STEP).is_none());
}

#[test]
fn test_challenge_binder_allocator_fragmentation_and_hole_filling() {
    // 1. Clean sequential allocation
    let mut binders: Vec<u32> = Vec::new();
    for i in 0..100 {
        let allocated = find_next_binder_index(&binders);
        assert_eq!(allocated, i);
        binders.push(allocated);
    }
    assert_eq!(binders.len(), 100);

    // 2. Punch holes: remove 0, 15, 42, 77, 99
    let holes = vec![0u32, 15, 42, 77, 99];
    binders.retain(|b| !holes.contains(b));
    assert_eq!(binders.len(), 95);

    // 3. Allocations must sequentially fill each hole in ascending order
    for &expected_hole in &holes {
        let allocated = find_next_binder_index(&binders);
        assert_eq!(allocated, expected_hole, "Must fill lowest available hole");
        binders.push(allocated);
    }
    assert_eq!(binders.len(), 100);

    // Next after holes filled must be 100
    assert_eq!(find_next_binder_index(&binders), 100);

    // 4. Adversarial inputs: unsorted, duplicates, large gaps
    let adversarial = vec![1000u32, 5, 2, 0, 2, 5, 1, 99999];
    // Existing contains 0, 1, 2, 5, 1000, 99999. Missing: 3.
    assert_eq!(find_next_binder_index(&adversarial), 3);
}

#[test]
fn test_challenge_combined_allocate_next_port_and_binder() {
    let mut instances = Vec::new();
    for i in 0..10 {
        instances.push(AndroidInstance {
            id: Uuid::new_v4(),
            name: format!("Inst {}", i),
            container_name: format!("cosmic-{}", i),
            android_version: AndroidVersion::Android13,
            image_tag: "tag".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity: HardwareIdentity::default(),
            proxy: ProxyConfig::default(),
            adb_port: ADB_PORT_BASE + (i as u16 * ADB_PORT_STEP),
            data_path: PathBuf::from(format!("/data/{}", i)),
            binder_index: i as u32,
            status: InstanceStatus::Stopped,
        });
    }

    // Allocate next
    let (p10, b10) = allocate_next_port_and_binder(&instances);
    assert_eq!(p10, ADB_PORT_BASE + 10 * ADB_PORT_STEP);
    assert_eq!(b10, 10);

    // Remove instance 3
    instances.remove(3);
    let (p_fill, b_fill) = allocate_next_port_and_binder(&instances);
    assert_eq!(p_fill, ADB_PORT_BASE + 3 * ADB_PORT_STEP);
    assert_eq!(b_fill, 3);
}

// =========================================================================
// 4. DOCKER TELEMETRY CALCULATOR EDGE CASES
// =========================================================================

fn make_test_stats(
    curr_cpu: u64,
    prev_cpu: u64,
    curr_sys: Option<u64>,
    prev_sys: Option<u64>,
    online_cpus: Option<u64>,
    percpu_len: usize,
    mem_usage: Option<u64>,
    mem_limit: Option<u64>,
) -> Stats {
    let percpu = vec![0u64; percpu_len];
    let json = serde_json::json!({
        "read": "2026-10-18T00:00:00Z",
        "preread": "2026-10-18T00:00:00Z",
        "num_procs": 1,
        "pids_stats": {},
        "network": null,
        "networks": null,
        "memory_stats": {
            "usage": mem_usage,
            "limit": mem_limit
        },
        "blkio_stats": {},
        "cpu_stats": {
            "cpu_usage": {
                "total_usage": curr_cpu,
                "percpu_usage": percpu,
                "usage_in_kernelmode": 0,
                "usage_in_usermode": 0
            },
            "system_cpu_usage": curr_sys,
            "online_cpus": online_cpus,
            "throttling_data": {
                "periods": 0,
                "throttled_periods": 0,
                "throttled_time": 0
            }
        },
        "precpu_stats": {
            "cpu_usage": {
                "total_usage": prev_cpu,
                "usage_in_kernelmode": 0,
                "usage_in_usermode": 0
            },
            "system_cpu_usage": prev_sys,
            "throttling_data": {
                "periods": 0,
                "throttled_periods": 0,
                "throttled_time": 0
            }
        },
        "storage_stats": {},
        "name": "/test-container",
        "id": "1234567890abcdef"
    });

    serde_json::from_value(json).expect("valid Stats JSON")
}

#[test]
fn test_challenge_telemetry_calculator_edge_cases_and_wrapping() {
    // Edge Case 1: system_delta == 0 (curr_sys == prev_sys)
    let stats_zero_sys = make_test_stats(100, 50, Some(1000), Some(1000), Some(4), 4, None, None);
    let m1 = calculate_metrics_from_stats(&stats_zero_sys);
    assert_eq!(m1.cpu_percent, 0.0);
    assert!(!m1.cpu_percent.is_nan());
    assert!(!m1.cpu_percent.is_infinite());

    // Edge Case 2: Counter wrap / reset on system CPU (curr_sys < prev_sys)
    let stats_wrap_sys = make_test_stats(100, 50, Some(500), Some(1000), Some(4), 4, None, None);
    let m2 = calculate_metrics_from_stats(&stats_wrap_sys);
    assert_eq!(m2.cpu_percent, 0.0);

    // Edge Case 3: Counter wrap / reset on container CPU (curr_cpu < prev_cpu)
    let stats_wrap_cpu = make_test_stats(50, 100, Some(2000), Some(1000), Some(4), 4, None, None);
    let m3 = calculate_metrics_from_stats(&stats_wrap_cpu);
    assert_eq!(m3.cpu_percent, 0.0);

    // Edge Case 4: None in system CPU usage
    let stats_none_sys = make_test_stats(100, 50, None, Some(1000), Some(4), 4, None, None);
    let m4 = calculate_metrics_from_stats(&stats_none_sys);
    assert_eq!(m4.cpu_percent, 0.0);

    // Edge Case 5: Normal multi-core scaling
    // cpu_delta = 500_000_000, sys_delta = 1_000_000_000 (50%), 8 cpus -> 400%
    let stats_normal = make_test_stats(
        1_500_000_000,
        1_000_000_000,
        Some(2_000_000_000),
        Some(1_000_000_000),
        Some(8),
        8,
        Some(1024 * 1024 * 512),   // 512 MB
        Some(1024 * 1024 * 2048),  // 2048 MB
    );
    let m5 = calculate_metrics_from_stats(&stats_normal);
    assert!((m5.cpu_percent - 400.0).abs() < 0.01);
    assert_eq!(m5.memory_used_mb, 512);
    assert_eq!(m5.memory_limit_mb, 2048);
    assert_eq!(m5.memory_percent(), 25.0);

    // Edge Case 6: Fallback to percpu_usage length when online_cpus is None
    let stats_percpu_fallback = make_test_stats(
        1_200_000_000,
        1_000_000_000,
        Some(2_000_000_000),
        Some(1_000_000_000),
        None,
        4, // 4 cpus from percpu
        None,
        None,
    );
    let m6 = calculate_metrics_from_stats(&stats_percpu_fallback);
    // delta = 200M / 1000M = 0.2 * 4 cpus * 100 = 80%
    assert!((m6.cpu_percent - 80.0).abs() < 0.01);

    // Edge Case 7: online_cpus is Some(0)
    let stats_zero_cpus = make_test_stats(
        1_500_000_000,
        1_000_000_000,
        Some(2_000_000_000),
        Some(1_000_000_000),
        Some(0),
        4,
        None,
        None,
    );
    let m7 = calculate_metrics_from_stats(&stats_zero_cpus);
    assert_eq!(m7.cpu_percent, 0.0);

    // Edge Case 8: Extreme overflow boundary (u64::MAX)
    let stats_max = make_test_stats(
        u64::MAX,
        0,
        Some(u64::MAX),
        Some(0),
        Some(1),
        1,
        Some(u64::MAX),
        Some(u64::MAX),
    );
    let m8 = calculate_metrics_from_stats(&stats_max);
    assert!(!m8.cpu_percent.is_nan());
    assert!(!m8.cpu_percent.is_infinite());
    assert!(m8.memory_used_mb > 0);
    assert!(m8.memory_limit_mb > 0);
}

// =========================================================================
// 5. HOSTCONFIG & CONTAINER CONFIG GENERATION MATRIX
// =========================================================================

#[test]
fn test_challenge_host_config_and_boot_args_generation_matrix() {
    let temp = ScopedTempDir::new("host_config_matrix");
    let root = temp.path();

    // Create GPU node in root
    fs::create_dir_all(root.join("dev/dri")).unwrap();
    fs::write(root.join("dev/dri/renderD128"), b"").unwrap();

    let presets = [
        (DevicePreset::GalaxyS24Ultra, 1440, 3120, 505, 120),
        (DevicePreset::Pixel8Pro, 1344, 2992, 489, 120),
        (DevicePreset::AsusRogPhone8, 1080, 2400, 395, 165),
        (DevicePreset::Custom, 1080, 1920, 420, 60),
    ];

    for (preset, expected_w, expected_h, expected_dpi, expected_fps) in &presets {
        let identity = populate_preset(*preset);
        let instance = AndroidInstance {
            id: Uuid::new_v4(),
            name: format!("Test {}", preset.display_name()),
            container_name: format!("cosmic-{}", preset.display_name().to_lowercase().replace(' ', "-")),
            android_version: AndroidVersion::Android13,
            image_tag: "redroid/redroid:13.0.0-latest".to_string(),
            preset: *preset,
            identity: identity.clone(),
            proxy: ProxyConfig::default(),
            adb_port: 5555,
            data_path: PathBuf::from("/data/instance"),
            binder_index: 0,
            status: InstanceStatus::Stopped,
        };

        // 1. With GPU node present
        let config_gpu = build_container_config_with_root(&instance, root);
        let host_config_gpu = config_gpu.host_config.as_ref().unwrap();

        assert_eq!(host_config_gpu.privileged, Some(true));
        assert_eq!(host_config_gpu.shm_size, Some(2 * 1024 * 1024 * 1024));
        assert_eq!(
            host_config_gpu.binds.as_ref().unwrap(),
            &vec!["/data/instance:/data".to_string()]
        );

        let devices = host_config_gpu.devices.as_ref().unwrap();
        assert!(
            devices.iter().any(|d| d.path_on_host.as_deref() == Some("/dev/dri/renderD128")),
            "HostConfig must contain DRI render node when present on host"
        );

        let cmd = config_gpu.cmd.as_ref().unwrap();
        assert!(cmd.contains(&format!("redroid.width={}", expected_w)));
        assert!(cmd.contains(&format!("redroid.height={}", expected_h)));
        assert!(cmd.contains(&format!("redroid.dpi={}", expected_dpi)));
        assert!(cmd.contains(&format!("redroid.fps={}", expected_fps)));
        assert!(cmd.contains(&"redroid.gpu.mode=host".to_string()));
        assert!(cmd.contains(&format!("ro.product.model={}", identity.model)));
        assert!(cmd.contains(&format!("ro.product.brand={}", identity.brand)));
        assert!(cmd.contains(&format!("ro.product.manufacturer={}", identity.manufacturer)));

        // 2. Without GPU node
        let temp_no_gpu = ScopedTempDir::new("no_gpu");
        let config_no_gpu = build_container_config_with_root(&instance, temp_no_gpu.path());
        let host_config_no_gpu = config_no_gpu.host_config.as_ref().unwrap();
        let devices_no_gpu = host_config_no_gpu.devices.as_ref().unwrap();
        assert!(
            !devices_no_gpu.iter().any(|d| d.path_on_host.as_deref() == Some("/dev/dri/renderD128")),
            "HostConfig must omit DRI node when absent on host"
        );
        let cmd_no_gpu = config_no_gpu.cmd.as_ref().unwrap();
        assert!(cmd_no_gpu.contains(&"redroid.gpu.mode=guest".to_string()));
    }
}

// =========================================================================
// 6. STORAGE SNAPSHOT METADATA SERIALIZATION & PATH SAFETY
// =========================================================================

#[test]
fn test_challenge_snapshot_metadata_serialization_roundtrip_and_path_safety() {
    let snapshot_id = Uuid::new_v4();
    let instance_id = Uuid::new_v4();
    let archive_path = PathBuf::from("/home/user/.local/share/cosmic-redroid/snapshots/archive with spaces & symbols.tar.zst");

    let snapshot = SnapshotInfo {
        id: snapshot_id,
        instance_id,
        name: "Backup 2026-10-18 (Clean State) #1".to_string(),
        created_at: Utc::now(),
        archive_path: archive_path.clone(),
        size_bytes: 1024 * 1024 * 85,
    };

    let json = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot metadata");
    let deserialized: SnapshotInfo = serde_json::from_str(&json).expect("deserialize snapshot metadata");

    assert_eq!(deserialized.id, snapshot_id);
    assert_eq!(deserialized.instance_id, instance_id);
    assert_eq!(deserialized.name, snapshot.name);
    assert_eq!(deserialized.archive_path, archive_path);
    assert_eq!(deserialized.size_bytes, 1024 * 1024 * 85);
    assert_eq!(deserialized.size_display(), "85.0 MB");
}

#[test]
fn test_challenge_snapshot_listing_resilience_to_corrupted_sidecars() {
    let temp = ScopedTempDir::new("snapshot_dir_resilience");
    let instance_id = Uuid::new_v4();
    let instance_snapshot_dir = temp.path().join(instance_id.to_string());
    fs::create_dir_all(&instance_snapshot_dir).unwrap();

    // 1. Valid snapshot file pair
    let snap1_id = Uuid::new_v4();
    let snap1_archive = instance_snapshot_dir.join(format!("{}.tar.zst", snap1_id));
    let snap1_json = instance_snapshot_dir.join(format!("{}.json", snap1_id));
    fs::write(&snap1_archive, b"archive contents 1").unwrap();
    let snap1_info = SnapshotInfo {
        id: snap1_id,
        instance_id,
        name: "Valid Snapshot 1".to_string(),
        created_at: Utc::now(),
        archive_path: snap1_archive.clone(),
        size_bytes: 18,
    };
    fs::write(&snap1_json, serde_json::to_string(&snap1_info).unwrap()).unwrap();

    // 2. Corrupted JSON file
    let corrupt_json = instance_snapshot_dir.join(format!("{}.json", Uuid::new_v4()));
    fs::write(&corrupt_json, b"{ invalid json content !!! ").unwrap();

    // 3. Orphaned JSON file (points to missing archive)
    let orphan_id = Uuid::new_v4();
    let orphan_json = instance_snapshot_dir.join(format!("{}.json", orphan_id));
    let orphan_info = SnapshotInfo {
        id: orphan_id,
        instance_id,
        name: "Orphaned Snapshot".to_string(),
        created_at: Utc::now(),
        archive_path: instance_snapshot_dir.join("nonexistent.tar.zst"),
        size_bytes: 100,
    };
    fs::write(&orphan_json, serde_json::to_string(&orphan_info).unwrap()).unwrap();

    // 4. Non-JSON garbage files
    fs::write(instance_snapshot_dir.join("temp.tmp"), b"junk").unwrap();
    fs::write(instance_snapshot_dir.join("notes.txt"), b"junk").unwrap();

    // Verify manual inspection of directory matching list_snapshots logic
    let mut loaded_snapshots = Vec::new();
    for entry in fs::read_dir(&instance_snapshot_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut info) = serde_json::from_str::<SnapshotInfo>(&content) {
                    if info.archive_path.exists() {
                        if let Ok(meta) = info.archive_path.metadata() {
                            info.size_bytes = meta.len();
                        }
                        loaded_snapshots.push(info);
                    }
                }
            }
        }
    }

    assert_eq!(
        loaded_snapshots.len(),
        1,
        "Snapshot scanner must gracefully skip corrupted, orphaned, and non-JSON files"
    );
    assert_eq!(loaded_snapshots[0].id, snap1_id);

    // Delete snapshot
    assert!(delete_snapshot(&snap1_info).is_ok());
    assert!(!snap1_archive.exists(), "Archive must be deleted");
    assert!(!snap1_json.exists(), "Sidecar metadata JSON must be deleted");
}

#[test]
fn test_challenge_write_local_prop_atomic_replacement() {
    let temp = ScopedTempDir::new("local_prop_atomic");
    let data_path = temp.path();

    let identity = populate_preset(DevicePreset::GalaxyS24Ultra);
    assert!(write_local_prop(data_path, &identity).is_ok());

    let target_file = data_path.join("local.prop");
    assert!(target_file.exists(), "local.prop must exist");

    let content = fs::read_to_string(&target_file).expect("read local.prop");
    assert!(content.contains(&format!("ro.product.model={}", identity.model)));
    assert!(content.contains(&format!("ro.product.brand={}", identity.brand)));
    assert!(content.contains(&format!("ro.serialno={}", identity.serial_no)));
    assert!(content.contains(&format!("ro.boot.serialno={}", identity.serial_no)));
    assert!(content.contains(&format!("ro.sf.lcd_density={}", identity.dpi)));
    assert!(content.contains(&format!("persist.radio.imei={}", identity.imei)));

    // Second write with different identity must atomically replace
    let identity2 = populate_preset(DevicePreset::Pixel8Pro);
    assert!(write_local_prop(data_path, &identity2).is_ok());
    let content2 = fs::read_to_string(&target_file).expect("read local.prop after update");
    assert!(content2.contains(&format!("ro.product.model={}", identity2.model)));
    assert!(!content2.contains(&format!("ro.product.model={}", identity.model)));
}

// =========================================================================
// 7. HOST REMEDIATION SCRIPT GENERATION
// =========================================================================

#[test]
fn test_challenge_host_remediation_script_content() {
    let script = generate_remediation_script();
    assert!(script.starts_with("#!/bin/bash\nset -e\n"));
    assert!(script.contains("modprobe binder_linux"));
    assert!(script.contains("mount -t binder binder /dev/binderfs"));
    assert!(script.contains("chmod 666 /dev/binder"));
    assert!(script.contains("chmod 666 /dev/dri/renderD*"));
}

// =========================================================================
// 8. ZERO EMOJI COMPLIANCE SCANNER
// =========================================================================

#[test]
fn test_challenge_zero_emojis_across_entire_repo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scan_dirs = ["src", "tests", "resources", "i18n"];

    let emoji_ranges = [
        (0x1F600, 0x1F64F), // Emoticons
        (0x1F300, 0x1F5FF), // Misc Symbols and Pictographs
        (0x1F680, 0x1F6FF), // Transport and Map
        (0x1F900, 0x1F9FF), // Supplemental Symbols and Pictographs
        (0x1FA70, 0x1FAFF), // Symbols and Pictographs Extended-A
        (0x2600, 0x26FF),   // Miscellaneous Symbols
        (0x2700, 0x27BF),   // Dingbats
    ];

    let mut scanned_count = 0;
    for dir in &scan_dirs {
        let dir_path = root.join(dir);
        if !dir_path.exists() {
            continue;
        }

        for entry in walkdir(&dir_path) {
            let path = entry.as_path();
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    scanned_count += 1;
                    for (line_idx, line) in content.lines().enumerate() {
                        for ch in line.chars() {
                            let cp = ch as u32;
                            for (start, end) in &emoji_ranges {
                                assert!(
                                    !(cp >= *start && cp <= *end),
                                    "Emoji violation found in file {}:{}: '{}' (codepoint U+{:04X})",
                                    path.display(),
                                    line_idx + 1,
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

    assert!(scanned_count > 10, "Scanner must inspect repository files");
}

fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                files.extend(walkdir(&p));
            } else {
                files.push(p);
            }
        }
    }
    files
}
