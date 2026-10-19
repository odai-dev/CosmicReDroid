use cosmic_redroid::backend::spoofing::{
    build_redroid_boot_args, generate_android_id, generate_boot_params, generate_luhn_imei,
    generate_mac_address, generate_serial, populate_preset, write_local_prop,
    write_local_prop_async,
};
use cosmic_redroid::models::{DevicePreset, HardwareIdentity};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

struct TestTempDir {
    path: PathBuf,
}

impl TestTempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cosmic_spoof_{}_{}", prefix, Uuid::new_v4()));
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

fn verify_luhn(imei: &str) -> bool {
    if imei.len() != 15 {
        return false;
    }
    let digits: Vec<u32> = match imei.chars().map(|c| c.to_digit(10).ok_or(())).collect() {
        Ok(v) => v,
        Err(_) => return false,
    };

    let mut sum = 0;
    for (i, &digit) in digits.iter().rev().enumerate() {
        if i % 2 == 1 {
            let doubled = digit * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += digit;
        }
    }
    sum % 10 == 0
}

#[test]
fn test_luhn_imei_generation_and_validation() {
    let tac_prefixes = ["35275311", "35327511", "35612311", "86012304", "01234567"];

    for tac in &tac_prefixes {
        for _ in 0..100 {
            let imei = generate_luhn_imei(tac);
            assert_eq!(imei.len(), 15, "IMEI must be 15 digits");
            assert!(imei.starts_with(tac), "IMEI must start with TAC prefix");
            assert!(
                verify_luhn(&imei),
                "Generated IMEI {imei} must pass Luhn checksum"
            );
        }
    }
}

#[test]
fn test_luhn_single_digit_and_transposition_error_detection() {
    let valid_imei = generate_luhn_imei("35275311");
    assert!(verify_luhn(&valid_imei));

    let chars: Vec<char> = valid_imei.chars().collect();

    // 1. Single-digit mutation test
    for i in 0..15 {
        let original_digit = chars[i].to_digit(10).unwrap();
        for new_digit in 0..10 {
            if new_digit == original_digit {
                continue;
            }
            let mut mutated = chars.clone();
            mutated[i] = std::char::from_digit(new_digit, 10).unwrap();
            let mutated_imei: String = mutated.into_iter().collect();
            assert!(
                !verify_luhn(&mutated_imei),
                "Single-digit alteration at index {i} must invalidate checksum: {mutated_imei}"
            );
        }
    }

    // 2. Transposition error test
    for i in 0..14 {
        if chars[i] != chars[i + 1] {
            let mut transposed = chars.clone();
            transposed.swap(i, i + 1);
            let transposed_imei: String = transposed.into_iter().collect();
            // In standard Luhn, swaps between 0 and 9 can produce equivalent sums,
            // but all other pairwise transpositions must fail.
            let a = chars[i].to_digit(10).unwrap();
            let b = chars[i + 1].to_digit(10).unwrap();
            if !((a == 0 && b == 9) || (a == 9 && b == 0)) {
                assert!(
                    !verify_luhn(&transposed_imei),
                    "Transposition between indices {i} and {} must be detected",
                    i + 1
                );
            }
        }
    }
}

#[test]
fn test_serial_numbers_format_and_charset() {
    let charset = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

    for _ in 0..200 {
        let serial = generate_serial();
        assert_eq!(serial.len(), 11, "Serial number must be 11 characters");
        assert!(
            serial.chars().all(|c| charset.contains(c)),
            "Serial must contain uppercase alphanumeric characters only: {serial}"
        );
    }
}

#[test]
fn test_mac_addresses_format_and_unicast_laa() {
    for _ in 0..200 {
        let mac = generate_mac_address();
        let octets: Vec<&str> = mac.split(':').collect();
        assert_eq!(octets.len(), 6, "MAC must consist of 6 octets");

        for octet in &octets {
            assert_eq!(octet.len(), 2);
            assert!(
                octet.chars().all(|c| c.is_ascii_hexdigit()),
                "Octet {octet} must be hex"
            );
        }

        // Verify Locally Administered Address (LAA) bit and Unicast bit
        let first_byte = u8::from_str_radix(octets[0], 16).expect("parse first octet");
        assert_eq!(
            first_byte & 0x01,
            0x00,
            "Least significant bit of first octet must be 0 for Unicast: {mac}"
        );
        assert_eq!(
            first_byte & 0x02,
            0x02,
            "Second least significant bit of first octet must be 1 for LAA: {mac}"
        );
    }
}

#[test]
fn test_android_id_format() {
    for _ in 0..200 {
        let android_id = generate_android_id();
        assert_eq!(android_id.len(), 16, "Android ID must be 16 characters");
        assert!(
            android_id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "Android ID must be lowercase hex: {android_id}"
        );
    }
}

#[test]
fn test_write_local_prop_atomic() {
    let temp = TestTempDir::new("local_prop");
    let identity = HardwareIdentity::galaxy_s24_ultra();

    // Initial write
    write_local_prop(temp.path(), &identity).expect("write local.prop");
    let prop_file = temp.path().join("local.prop");
    assert!(prop_file.exists());

    let content = fs::read_to_string(&prop_file).expect("read local.prop");
    assert!(content.contains(&format!("ro.product.model={}", identity.model)));
    assert!(content.contains(&format!("ro.product.brand={}", identity.brand)));
    assert!(content.contains(&format!("ro.serialno={}", identity.serial_no)));

    // Verify atomic replacement leaves no dangling temp files
    let mut updated_identity = identity.clone();
    updated_identity.model = "SM-S928B-MODIFIED".to_string();
    write_local_prop(temp.path(), &updated_identity).expect("overwrite local.prop");

    let updated_content = fs::read_to_string(&prop_file).expect("read updated local.prop");
    assert!(updated_content.contains("ro.product.model=SM-S928B-MODIFIED"));

    let stray_temps: Vec<_> = fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(".local.prop.tmp"))
        .collect();
    assert_eq!(stray_temps.len(), 0, "No temporary files should be left behind");
}

#[tokio::test]
async fn test_write_local_prop_async() {
    let temp = TestTempDir::new("local_prop_async");
    let identity = HardwareIdentity::pixel_8_pro();

    write_local_prop_async(temp.path(), &identity)
        .await
        .expect("write async local.prop");

    let prop_file = temp.path().join("local.prop");
    assert!(prop_file.exists());

    let content = fs::read_to_string(&prop_file).expect("read async local.prop");
    assert!(content.contains(&format!("ro.product.model={}", identity.model)));
}

#[test]
fn test_redroid_boot_args_generation() {
    let identity = HardwareIdentity::galaxy_s24_ultra();
    let boot_args = build_redroid_boot_args(&identity, true);

    assert!(boot_args.iter().any(|arg| arg.starts_with("redroid.width=")));
    assert!(boot_args.iter().any(|arg| arg.starts_with("redroid.height=")));
    assert!(boot_args.iter().any(|arg| arg.starts_with("redroid.dpi=")));
    assert!(boot_args.iter().any(|arg| arg.starts_with("redroid.fps=")));
    assert!(boot_args.iter().any(|arg| arg == "redroid.gpu.mode=host"));

    let no_gpu_args = build_redroid_boot_args(&identity, false);
    assert!(no_gpu_args.iter().any(|arg| arg == "redroid.gpu.mode=guest"));

    let params = generate_boot_params(&identity);
    assert!(!params.is_empty());
}

#[test]
fn test_device_preset_definitions() {
    let s24 = populate_preset(DevicePreset::GalaxyS24Ultra);
    assert_eq!(s24.model, "SM-S928B");
    assert_eq!(s24.brand, "samsung");
    assert_eq!(s24.manufacturer, "samsung");
    assert_eq!(s24.width, 1440);
    assert_eq!(s24.height, 3120);
    assert_eq!(s24.dpi, 505);
    assert_eq!(s24.fps, 120);

    let p8 = populate_preset(DevicePreset::Pixel8Pro);
    assert_eq!(p8.model, "Pixel 8 Pro");
    assert_eq!(p8.brand, "google");
    assert_eq!(p8.manufacturer, "Google");
    assert_eq!(p8.width, 1344);
    assert_eq!(p8.height, 2992);
    assert_eq!(p8.dpi, 489);
    assert_eq!(p8.fps, 120);

    let rog = populate_preset(DevicePreset::AsusRogPhone8);
    assert_eq!(rog.model, "ASUS_AI2401_D");
    assert_eq!(rog.brand, "asus");
    assert_eq!(rog.manufacturer, "asus");
    assert_eq!(rog.width, 1080);
    assert_eq!(rog.height, 2400);
    assert_eq!(rog.dpi, 395);
    assert_eq!(rog.fps, 165);
}
