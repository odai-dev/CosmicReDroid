//! Hardware spoofing, device presets, identifier generators,
//! and redroid boot arguments.

#![allow(dead_code)]

use anyhow::{Context, Result};
use rand::Rng;
use std::io::Write;
use std::path::Path;

use crate::models::{DevicePreset, HardwareIdentity};

/// Character set for generating uppercase alphanumeric Android serial numbers.
const SERIAL_CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Generates a valid 15-digit IMEI adhering to the Luhn algorithm (ISO/IEC 7812-1).
///
/// Format: 8-digit TAC + 6-digit SNR + 1-digit Luhn Checksum.
pub fn generate_luhn_imei(tac_prefix: &str) -> String {
    let mut rng = rand::thread_rng();

    // Sanitize TAC prefix to digits only
    let clean_tac: String = tac_prefix.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut imei14 = String::with_capacity(15);

    if clean_tac.len() >= 8 {
        imei14.push_str(&clean_tac[..8]);
    } else {
        imei14.push_str(&clean_tac);
        while imei14.len() < 8 {
            let digit: u8 = rng.gen_range(0..=9);
            imei14.push_str(&digit.to_string());
        }
    }

    // Append 6 random serial number (SNR) digits
    while imei14.len() < 14 {
        let digit: u8 = rng.gen_range(0..=9);
        imei14.push_str(&digit.to_string());
    }
    imei14.truncate(14);

    // Calculate Luhn check digit over the 14 digits
    let mut sum: i32 = 0;
    for (i, ch) in imei14.chars().enumerate() {
        let digit = ch.to_digit(10).unwrap_or(0) as i32;
        if i % 2 == 1 {
            let doubled = digit * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += digit;
        }
    }

    let check_digit = (10 - (sum % 10)) % 10;
    format!("{}{}", imei14, check_digit)
}

/// Verifies whether a given 15-digit IMEI satisfies the Luhn algorithm.
pub fn verify_luhn(imei: &str) -> bool {
    if imei.len() != 15 || !imei.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    let mut sum: i32 = 0;
    for (i, ch) in imei.chars().enumerate() {
        let digit = match ch.to_digit(10) {
            Some(d) => d as i32,
            None => return false,
        };
        if i % 2 == 1 {
            let doubled = digit * 2;
            sum += if doubled > 9 { doubled - 9 } else { doubled };
        } else {
            sum += digit;
        }
    }

    sum % 10 == 0
}

/// Generates an 11-character uppercase alphanumeric serial number.
pub fn generate_serial() -> String {
    let mut rng = rand::thread_rng();
    (0..11)
        .map(|_| {
            let idx = rng.gen_range(0..SERIAL_CHARSET.len());
            SERIAL_CHARSET[idx] as char
        })
        .collect()
}

/// Generates a 16-character lowercase hexadecimal Android ID (Settings.Secure.ANDROID_ID).
pub fn generate_android_id() -> String {
    let mut rng = rand::thread_rng();
    let id_num: u64 = rng.gen();
    format!("{:016x}", id_num)
}

/// Generates a locally administered unicast MAC address (format: 02:42:ac:xx:xx:xx).
pub fn generate_mac_address() -> String {
    let mut rng = rand::thread_rng();
    format!(
        "02:42:ac:{:02x}:{:02x}:{:02x}",
        rng.gen::<u8>(),
        rng.gen::<u8>(),
        rng.gen::<u8>()
    )
}

/// Synthesizes a new hardware identity for a given preset with randomized identifiers.
pub fn populate_preset(preset: DevicePreset) -> HardwareIdentity {
    let mut identity = match preset {
        DevicePreset::GalaxyS24Ultra => HardwareIdentity::galaxy_s24_ultra(),
        DevicePreset::Pixel8Pro => HardwareIdentity::pixel_8_pro(),
        DevicePreset::AsusRogPhone8 => HardwareIdentity::asus_rog_phone_8(),
        DevicePreset::Custom => HardwareIdentity {
            manufacturer: "Generic".to_string(),
            brand: "generic".to_string(),
            model: "Android Phone".to_string(),
            device: "generic_arm64".to_string(),
            board: "unknown".to_string(),
            fingerprint: "generic/generic_arm64/generic_arm64:14/UD1A.230803.041/10808477:user/release-keys".to_string(),
            serial_no: String::new(),
            imei: String::new(),
            android_id: String::new(),
            mac_address: String::new(),
            width: 1080,
            height: 1920,
            dpi: 420,
            fps: 60,
        },
    };

    identity.imei = generate_luhn_imei(preset.tac_prefix());
    identity.serial_no = generate_serial();
    identity.android_id = generate_android_id();
    identity.mac_address = generate_mac_address();

    identity
}

/// Generates a hardware identity populated from a device preset.
pub fn preset_for_device(preset: DevicePreset) -> HardwareIdentity {
    populate_preset(preset)
}

/// Synchronously writes the `/data/local.prop` property file using atomic write + rename.
pub fn write_local_prop(data_path: &Path, identity: &HardwareIdentity) -> Result<()> {
    std::fs::create_dir_all(data_path)
        .with_context(|| format!("Failed to create data directory at '{}'", data_path.display()))?;

    let target_file = data_path.join("local.prop");
    let temp_name = format!(
        ".local.prop.tmp.{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let temp_file = data_path.join(temp_name);

    let lines = identity.to_local_prop_lines();
    let content = lines.join("\n") + "\n";

    {
        let mut file = std::fs::File::create(&temp_file).with_context(|| {
            format!("Failed to create temporary prop file '{}'", temp_file.display())
        })?;
        file.write_all(content.as_bytes()).with_context(|| {
            format!("Failed to write to temporary prop file '{}'", temp_file.display())
        })?;
        file.sync_all().with_context(|| {
            format!("Failed to sync temporary prop file '{}'", temp_file.display())
        })?;
    }

    std::fs::rename(&temp_file, &target_file).with_context(|| {
        format!(
            "Failed to atomically rename '{}' to '{}'",
            temp_file.display(),
            target_file.display()
        )
    })?;

    Ok(())
}

/// Asynchronously writes the `/data/local.prop` property file.
pub async fn write_local_prop_async(data_path: &Path, identity: &HardwareIdentity) -> Result<()> {
    tokio::fs::create_dir_all(data_path)
        .await
        .with_context(|| format!("Failed to create data directory at '{}'", data_path.display()))?;

    let target_file = data_path.join("local.prop");
    let lines = identity.to_local_prop_lines();
    let content = lines.join("\n") + "\n";

    tokio::fs::write(&target_file, content.as_bytes())
        .await
        .with_context(|| format!("Failed to write local.prop at '{}'", target_file.display()))?;

    Ok(())
}

/// Builds the array of kernel boot command-line arguments for ReDroid container init.
pub fn build_redroid_boot_args(identity: &HardwareIdentity, gpu_enabled: bool) -> Vec<String> {
    vec![
        "androidboot.hardware=redroid".to_string(),
        "ro.secure=0".to_string(),
        "ro.boot.hwcomposer=ranchu".to_string(),
        format!("redroid.width={}", identity.width),
        format!("redroid.height={}", identity.height),
        format!("redroid.dpi={}", identity.dpi),
        format!("redroid.fps={}", identity.fps),
        if gpu_enabled {
            "redroid.gpu.mode=host".to_string()
        } else {
            "redroid.gpu.mode=guest".to_string()
        },
        format!("ro.product.model={}", identity.model),
        format!("ro.product.brand={}", identity.brand),
        format!("ro.product.manufacturer={}", identity.manufacturer),
        format!("ro.serialno={}", identity.serial_no),
        format!("ro.boot.serialno={}", identity.serial_no),
    ]
}

/// Generates redroid boot parameter strings for container startup.
pub fn generate_boot_params(identity: &HardwareIdentity) -> Vec<String> {
    build_redroid_boot_args(identity, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luhn_imei_validity_thousand_samples() {
        let tac_prefixes = ["35275311", "35327511", "35612311", "86012304", "01234567"];
        for _ in 0..1000 {
            for prefix in &tac_prefixes {
                let imei = generate_luhn_imei(prefix);
                assert_eq!(imei.len(), 15, "IMEI length must be exactly 15");
                assert!(
                    imei.starts_with(prefix),
                    "IMEI must start with requested TAC prefix"
                );
                assert!(verify_luhn(&imei), "Generated IMEI must pass Luhn verification");
            }
        }
    }

    #[test]
    fn test_luhn_verification_detects_corrupt_digit() {
        let valid_imei = generate_luhn_imei("35275311");
        assert!(verify_luhn(&valid_imei));

        // Corrupt check digit
        let mut chars: Vec<char> = valid_imei.chars().collect();
        let last_idx = chars.len() - 1;
        let original_last = chars[last_idx];
        let mutated_last = if original_last == '0' { '1' } else { '0' };
        chars[last_idx] = mutated_last;
        let corrupted: String = chars.into_iter().collect();

        assert!(!verify_luhn(&corrupted), "Corrupted IMEI must fail Luhn verification");
    }

    #[test]
    fn test_luhn_imei_odd_tac_prefix_sanitization() {
        // TAC shorter than 8 digits
        let short_tac = "35275";
        let imei = generate_luhn_imei(short_tac);
        assert_eq!(imei.len(), 15);
        assert!(verify_luhn(&imei));

        // TAC with non-digits
        let dirty_tac = "35-27.5311";
        let clean_imei = generate_luhn_imei(dirty_tac);
        assert_eq!(clean_imei.len(), 15);
        assert!(clean_imei.starts_with("35275311"));
        assert!(verify_luhn(&clean_imei));
    }

    #[test]
    fn test_serial_format_and_charset() {
        for _ in 0..500 {
            let serial = generate_serial();
            assert_eq!(serial.len(), 11, "Serial must be exactly 11 characters");
            assert!(
                serial.chars().all(|c| c.is_ascii_alphanumeric() && !c.is_ascii_lowercase()),
                "Serial must contain only uppercase ASCII alphanumeric characters: {}",
                serial
            );
        }
    }

    #[test]
    fn test_android_id_format_and_hex() {
        for _ in 0..500 {
            let android_id = generate_android_id();
            assert_eq!(android_id.len(), 16, "Android ID must be exactly 16 characters");
            assert!(
                android_id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "Android ID must be lowercase hexadecimal: {}",
                android_id
            );
        }
    }

    #[test]
    fn test_mac_address_locally_administered_unicast() {
        for _ in 0..500 {
            let mac = generate_mac_address();
            assert_eq!(mac.len(), 17, "MAC must be exactly 17 characters");
            assert!(mac.starts_with("02:42:ac:"), "MAC must start with 02:42:ac:");

            let parts: Vec<&str> = mac.split(':').collect();
            assert_eq!(parts.len(), 6, "MAC must contain 6 octets");
            for part in &parts {
                assert_eq!(part.len(), 2, "Each MAC octet must be 2 characters");
                assert!(u8::from_str_radix(part, 16).is_ok(), "Each octet must be valid hex");
            }
        }
    }

    #[test]
    fn test_populate_preset_all_variants() {
        for preset in DevicePreset::ALL {
            let identity = populate_preset(preset);
            assert_eq!(identity.imei.len(), 15);
            assert!(verify_luhn(&identity.imei));
            assert_eq!(identity.serial_no.len(), 11);
            assert_eq!(identity.android_id.len(), 16);
            assert_eq!(identity.mac_address.len(), 17);
            assert!(identity.width > 0);
            assert!(identity.height > 0);
            assert!(identity.dpi > 0);
            assert!(identity.fps > 0);
            assert!(!identity.fingerprint.is_empty());
        }
    }

    #[test]
    fn test_preset_for_device_matches_populate_preset() {
        let s24 = preset_for_device(DevicePreset::GalaxyS24Ultra);
        assert_eq!(s24.model, "SM-S928B");
        assert_eq!(s24.manufacturer, "samsung");
    }

    #[test]
    fn test_write_local_prop_atomic() {
        let temp_dir = std::env::temp_dir().join(format!("test_local_prop_{}", uuid::Uuid::new_v4()));
        let identity = populate_preset(DevicePreset::GalaxyS24Ultra);

        let result = write_local_prop(&temp_dir, &identity);
        assert!(result.is_ok(), "write_local_prop must succeed");

        let file_path = temp_dir.join("local.prop");
        assert!(file_path.exists(), "local.prop must exist");

        let content = std::fs::read_to_string(&file_path).expect("read local.prop");
        assert!(content.contains("ro.product.model=SM-S928B"));
        assert!(content.contains(&format!("ro.serialno={}", identity.serial_no)));
        assert!(content.contains(&format!("persist.radio.imei={}", identity.imei)));
        assert!(content.contains(&format!("ro.sf.lcd_density={}", identity.dpi)));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_build_redroid_boot_args_modes() {
        let identity = populate_preset(DevicePreset::Pixel8Pro);

        let host_args = build_redroid_boot_args(&identity, true);
        assert!(host_args.contains(&"redroid.gpu.mode=host".to_string()));
        assert!(host_args.contains(&"androidboot.hardware=redroid".to_string()));
        assert!(host_args.contains(&format!("redroid.width={}", identity.width)));
        assert!(host_args.contains(&format!("redroid.height={}", identity.height)));
        assert!(host_args.contains(&format!("redroid.fps={}", identity.fps)));
        assert!(host_args.contains(&format!("ro.product.model={}", identity.model)));

        let guest_args = build_redroid_boot_args(&identity, false);
        assert!(guest_args.contains(&"redroid.gpu.mode=guest".to_string()));

        let default_params = generate_boot_params(&identity);
        assert_eq!(default_params, host_args);
    }
}
