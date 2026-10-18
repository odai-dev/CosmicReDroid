#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Navigation destinations within the CosmicReDroid control center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NavPage {
    #[default]
    Instances,
    ImageFactory,
    Snapshots,
    HostHealth,
}

impl NavPage {
    pub const ALL: [NavPage; 4] = [
        NavPage::Instances,
        NavPage::ImageFactory,
        NavPage::Snapshots,
        NavPage::HostHealth,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            NavPage::Instances => "Instances",
            NavPage::ImageFactory => "Image Factory",
            NavPage::Snapshots => "Snapshots",
            NavPage::HostHealth => "Host Health",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            NavPage::Instances => "view-grid-symbolic",
            NavPage::ImageFactory => "applications-engineering-symbolic",
            NavPage::Snapshots => "camera-photo-symbolic",
            NavPage::HostHealth => "utilities-system-monitor-symbolic",
        }
    }
}

/// Supported ReDroid base Android versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AndroidVersion {
    Android11,
    #[default]
    Android13,
    Android14,
}

impl AndroidVersion {
    pub const ALL: [AndroidVersion; 3] = [
        AndroidVersion::Android11,
        AndroidVersion::Android13,
        AndroidVersion::Android14,
    ];

    pub const NAMES: &'static [&'static str] = &[
        "Android 11 (API 30 - RedBull)",
        "Android 13 (API 33 - Tiramisu)",
        "Android 14 (API 34 - Upside Down Cake)",
    ];

    pub fn display_label(&self) -> &'static str {
        match self {
            AndroidVersion::Android11 => "Android 11 (API 30 - RedBull)",
            AndroidVersion::Android13 => "Android 13 (API 33 - Tiramisu)",
            AndroidVersion::Android14 => "Android 14 (API 34 - Upside Down Cake)",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            AndroidVersion::Android11 => "Android 11",
            AndroidVersion::Android13 => "Android 13",
            AndroidVersion::Android14 => "Android 14",
        }
    }

    pub fn image_tag(&self) -> &'static str {
        match self {
            AndroidVersion::Android11 => "redroid/redroid:11.0.0-latest",
            AndroidVersion::Android13 => "redroid/redroid:13.0.0-latest",
            AndroidVersion::Android14 => "redroid/redroid:14.0.0-latest",
        }
    }

    pub fn default_image_tag(&self) -> &'static str {
        self.image_tag()
    }

    pub fn api_level(&self) -> u32 {
        match self {
            AndroidVersion::Android11 => 30,
            AndroidVersion::Android13 => 33,
            AndroidVersion::Android14 => 34,
        }
    }
}

/// Predefined hardware spoofing profiles for flagship devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DevicePreset {
    #[default]
    GalaxyS24Ultra,
    Pixel8Pro,
    AsusRogPhone8,
    Custom,
}

impl DevicePreset {
    pub const ALL: [DevicePreset; 4] = [
        DevicePreset::GalaxyS24Ultra,
        DevicePreset::Pixel8Pro,
        DevicePreset::AsusRogPhone8,
        DevicePreset::Custom,
    ];

    pub const NAMES: &'static [&'static str] = &[
        "Samsung Galaxy S24 Ultra",
        "Google Pixel 8 Pro",
        "ASUS ROG Phone 8",
        "Custom Device Profile",
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            DevicePreset::GalaxyS24Ultra => "Samsung Galaxy S24 Ultra",
            DevicePreset::Pixel8Pro => "Google Pixel 8 Pro",
            DevicePreset::AsusRogPhone8 => "ASUS ROG Phone 8",
            DevicePreset::Custom => "Custom Device Profile",
        }
    }

    pub fn tac_prefix(&self) -> &'static str {
        match self {
            DevicePreset::GalaxyS24Ultra => "35275311",
            DevicePreset::Pixel8Pro => "35327511",
            DevicePreset::AsusRogPhone8 => "35612311",
            DevicePreset::Custom => "86012304",
        }
    }

    pub fn default_identity(&self) -> HardwareIdentity {
        match self {
            DevicePreset::GalaxyS24Ultra => HardwareIdentity::galaxy_s24_ultra(),
            DevicePreset::Pixel8Pro => HardwareIdentity::pixel_8_pro(),
            DevicePreset::AsusRogPhone8 => HardwareIdentity::asus_rog_phone_8(),
            DevicePreset::Custom => HardwareIdentity::default(),
        }
    }
}

/// Lifecycle state machine for an Android instance container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum InstanceStatus {
    #[default]
    Stopped,
    DownloadingImage(String),
    Starting,
    WaitingForBoot,
    Running,
    Stopping,
    Cloning,
    Snapshotting,
    Error(String),
}

impl InstanceStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, InstanceStatus::Running)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            InstanceStatus::DownloadingImage(_)
                | InstanceStatus::Starting
                | InstanceStatus::WaitingForBoot
                | InstanceStatus::Running
                | InstanceStatus::Stopping
        )
    }

    pub fn is_transitional(&self) -> bool {
        matches!(
            self,
            InstanceStatus::DownloadingImage(_)
                | InstanceStatus::Starting
                | InstanceStatus::WaitingForBoot
                | InstanceStatus::Stopping
                | InstanceStatus::Cloning
                | InstanceStatus::Snapshotting
        )
    }

    pub fn can_start(&self) -> bool {
        matches!(self, InstanceStatus::Stopped | InstanceStatus::Error(_))
    }

    pub fn can_stop(&self) -> bool {
        matches!(
            self,
            InstanceStatus::Running
                | InstanceStatus::Starting
                | InstanceStatus::WaitingForBoot
                | InstanceStatus::DownloadingImage(_)
        )
    }

    pub fn display_status(&self) -> String {
        match self {
            InstanceStatus::Stopped => "Stopped".to_string(),
            InstanceStatus::DownloadingImage(desc) => {
                if desc.is_empty() {
                    "Downloading image...".to_string()
                } else {
                    desc.clone()
                }
            }
            InstanceStatus::Starting => "Starting...".to_string(),
            InstanceStatus::WaitingForBoot => "Booting Android...".to_string(),
            InstanceStatus::Running => "Running".to_string(),
            InstanceStatus::Stopping => "Stopping...".to_string(),
            InstanceStatus::Cloning => "Cloning...".to_string(),
            InstanceStatus::Snapshotting => "Snapshotting...".to_string(),
            InstanceStatus::Error(err) => format!("Error: {err}"),
        }
    }

    pub fn display_label(&self) -> String {
        self.display_status()
    }

    pub fn status_icon_name(&self) -> &'static str {
        match self {
            InstanceStatus::Stopped => "media-playback-stop-symbolic",
            InstanceStatus::DownloadingImage(_) => "folder-download-symbolic",
            InstanceStatus::Starting | InstanceStatus::WaitingForBoot => "system-run-symbolic",
            InstanceStatus::Running => "media-playback-start-symbolic",
            InstanceStatus::Stopping => "process-stop-symbolic",
            InstanceStatus::Cloning => "edit-copy-symbolic",
            InstanceStatus::Snapshotting => "camera-photo-symbolic",
            InstanceStatus::Error(_) => "dialog-error-symbolic",
        }
    }
}

/// Progress telemetry for a container image download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageDownloadProgress {
    pub image_tag: String,
    pub status_text: String,
    pub current_bytes: u64,
    pub total_bytes: u64,
    pub progress_fraction: f32,
    pub is_complete: bool,
    pub error: Option<String>,
}

impl Default for ImageDownloadProgress {
    fn default() -> Self {
        Self {
            image_tag: String::new(),
            status_text: String::new(),
            current_bytes: 0,
            total_bytes: 0,
            progress_fraction: 0.0,
            is_complete: false,
            error: None,
        }
    }
}

impl ImageDownloadProgress {
    pub fn new(image_tag: impl Into<String>) -> Self {
        Self {
            image_tag: image_tag.into(),
            status_text: "Contacting registry...".to_string(),
            ..Default::default()
        }
    }

    pub fn formatted_progress(&self) -> String {
        if self.total_bytes > 0 {
            let current_mb = (self.current_bytes as f64) / (1024.0 * 1024.0);
            let total_mb = (self.total_bytes as f64) / (1024.0 * 1024.0);
            let percent = (self.progress_fraction * 100.0).clamp(0.0, 100.0) as u32;
            format!("{:.1} MB / {:.1} MB ({}%)", current_mb, total_mb, percent)
        } else if !self.status_text.is_empty() {
            self.status_text.clone()
        } else {
            "Downloading...".to_string()
        }
    }
}

/// Detailed hardware parameters for guest OS spoofing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareIdentity {
    pub manufacturer: String,
    pub brand: String,
    pub model: String,
    pub device: String,
    pub board: String,
    pub fingerprint: String,
    pub serial_no: String,
    pub imei: String,
    pub android_id: String,
    pub mac_address: String,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub fps: u32,
}

impl Default for HardwareIdentity {
    fn default() -> Self {
        Self::galaxy_s24_ultra()
    }
}

impl HardwareIdentity {
    pub fn galaxy_s24_ultra() -> Self {
        Self {
            manufacturer: "samsung".to_string(),
            brand: "samsung".to_string(),
            model: "SM-S928B".to_string(),
            device: "e3q".to_string(),
            board: "pineapple".to_string(),
            fingerprint: "samsung/e3qxxx/e3q:14/UP1A.231005.007/S928BXXU1AXB5:user/release-keys".to_string(),
            serial_no: "R5CW10ABCDE".to_string(),
            imei: "352753110123456".to_string(),
            android_id: "7a8b9c0d1e2f3a4b".to_string(),
            mac_address: "02:42:ac:11:00:02".to_string(),
            width: 1440,
            height: 3120,
            dpi: 505,
            fps: 120,
        }
    }

    pub fn pixel_8_pro() -> Self {
        Self {
            manufacturer: "Google".to_string(),
            brand: "google".to_string(),
            model: "Pixel 8 Pro".to_string(),
            device: "husky".to_string(),
            board: "ripcurrent".to_string(),
            fingerprint: "google/husky/husky:14/UD1A.231105.004/11038385:user/release-keys".to_string(),
            serial_no: "39021FQDV00012".to_string(),
            imei: "353275110987654".to_string(),
            android_id: "8b9c0d1e2f3a4b5c".to_string(),
            mac_address: "02:42:ac:11:00:03".to_string(),
            width: 1344,
            height: 2992,
            dpi: 489,
            fps: 120,
        }
    }

    pub fn asus_rog_phone_8() -> Self {
        Self {
            manufacturer: "asus".to_string(),
            brand: "asus".to_string(),
            model: "ASUS_AI2401_D".to_string(),
            device: "AI2401".to_string(),
            board: "pineapple".to_string(),
            fingerprint: "asus/WW_Phone/ASUS_AI2401_D:14/UKQ1.231003.002/34.1420.1420.316-0:user/release-keys".to_string(),
            serial_no: "K5AT20A0012".to_string(),
            imei: "356123110123456".to_string(),
            android_id: "9c0d1e2f3a4b5c6d".to_string(),
            mac_address: "02:42:ac:11:00:04".to_string(),
            width: 1080,
            height: 2400,
            dpi: 395,
            fps: 165,
        }
    }

    pub fn resolution_display(&self) -> String {
        format!("{}x{} @ {} FPS ({} DPI)", self.width, self.height, self.fps, self.dpi)
    }

    pub fn to_local_prop_lines(&self) -> Vec<String> {
        vec![
            format!("ro.product.manufacturer={}", self.manufacturer),
            format!("ro.product.brand={}", self.brand),
            format!("ro.product.model={}", self.model),
            format!("ro.product.device={}", self.device),
            format!("ro.product.board={}", self.board),
            format!("ro.product.name={}", self.device),
            format!("ro.build.fingerprint={}", self.fingerprint),
            format!("ro.serialno={}", self.serial_no),
            format!("ro.boot.serialno={}", self.serial_no),
            format!("net.hostname={}-{}", self.brand, self.model),
            format!("ro.sf.lcd_density={}", self.dpi),
            format!("persist.radio.imei={}", self.imei),
        ]
    }
}

/// Network proxy settings for guest traffic redirection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn host_port_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn summary_display(&self) -> String {
        if self.enabled && !self.host.is_empty() && self.port > 0 {
            format!("{}://{}:{}", self.protocol, self.host, self.port)
        } else {
            "Disabled".to_string()
        }
    }
}

/// Primary entity representing a managed ReDroid container instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AndroidInstance {
    pub id: Uuid,
    pub name: String,
    pub container_name: String,
    pub android_version: AndroidVersion,
    pub image_tag: String,
    pub preset: DevicePreset,
    pub identity: HardwareIdentity,
    pub proxy: ProxyConfig,
    pub adb_port: u16,
    pub data_path: PathBuf,
    pub binder_index: u32,
    pub status: InstanceStatus,
}

impl AndroidInstance {
    pub fn adb_address(&self) -> String {
        format!("127.0.0.1:{}", self.adb_port)
    }

    pub fn binder_indices(&self) -> (u32, u32, u32) {
        let b1 = self.binder_index * 3 + 1;
        let b2 = self.binder_index * 3 + 2;
        let b3 = self.binder_index * 3 + 3;
        (b1, b2, b3)
    }

    pub fn binder_nodes(&self) -> (String, String, String) {
        let (b1, b2, b3) = self.binder_indices();
        (
            format!("/dev/binder{}", b1),
            format!("/dev/hwbinder{}", b2),
            format!("/dev/vndbinder{}", b3),
        )
    }
}

/// Runtime performance telemetry queried from Docker container stats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ContainerMetrics {
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_limit_mb: u64,
}

impl ContainerMetrics {
    pub fn memory_percent(&self) -> f32 {
        if self.memory_limit_mb > 0 {
            (self.memory_used_mb as f32 / self.memory_limit_mb as f32) * 100.0
        } else {
            0.0
        }
    }

    pub fn cpu_display(&self) -> String {
        format!("{:.1}%", self.cpu_percent)
    }

    pub fn memory_display(&self) -> String {
        format!(
            "{} / {} MB ({:.1}%)",
            self.memory_used_mb,
            self.memory_limit_mb,
            self.memory_percent()
        )
    }
}

/// Metadata record for an instance data volume snapshot archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotInfo {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub archive_path: PathBuf,
    pub size_bytes: u64,
}

impl SnapshotInfo {
    pub fn size_display(&self) -> String {
        let mb = self.size_bytes as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.2} GB", mb / 1024.0)
        } else {
            format!("{:.1} MB", mb)
        }
    }

    pub fn date_display(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }

    pub fn file_path(&self) -> &Path {
        &self.archive_path
    }
}

/// Comprehensive diagnostic probe result of host prerequisites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HostHealthReport {
    pub docker_available: bool,
    pub binder_available: bool,
    pub gpu_available: bool,
    pub adb_available: bool,
    pub scrcpy_available: bool,
    pub reflink_supported: bool,
    pub remediation_needed: bool,
    pub details: Vec<String>,
}

impl HostHealthReport {
    pub fn all_ok(&self) -> bool {
        self.docker_available
            && self.binder_available
            && self.gpu_available
            && self.adb_available
            && self.scrcpy_available
            && self.reflink_supported
            && !self.remediation_needed
    }

    pub fn needs_remediation(&self) -> bool {
        self.remediation_needed
            || !self.binder_available
            || !self.docker_available
            || !self.gpu_available
    }

    pub fn is_healthy(&self) -> bool {
        self.all_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nav_page_properties() {
        assert_eq!(NavPage::ALL.len(), 4);
        assert_eq!(NavPage::Instances.title(), "Instances");
        assert_eq!(NavPage::ImageFactory.title(), "Image Factory");
        assert_eq!(NavPage::Snapshots.title(), "Snapshots");
        assert_eq!(NavPage::HostHealth.title(), "Host Health");

        assert_eq!(NavPage::Instances.icon_name(), "view-grid-symbolic");
        assert_eq!(
            NavPage::ImageFactory.icon_name(),
            "applications-engineering-symbolic"
        );
        assert_eq!(NavPage::Snapshots.icon_name(), "camera-photo-symbolic");
        assert_eq!(
            NavPage::HostHealth.icon_name(),
            "utilities-system-monitor-symbolic"
        );

        let serialized = serde_json::to_string(&NavPage::Instances).expect("serialize");
        let deserialized: NavPage = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, NavPage::Instances);
    }

    #[test]
    fn test_android_version_properties() {
        assert_eq!(AndroidVersion::ALL.len(), 3);
        assert_eq!(AndroidVersion::default(), AndroidVersion::Android13);

        assert_eq!(AndroidVersion::Android11.api_level(), 30);
        assert_eq!(AndroidVersion::Android13.api_level(), 33);
        assert_eq!(AndroidVersion::Android14.api_level(), 34);

        assert_eq!(
            AndroidVersion::Android11.image_tag(),
            "redroid/redroid:11.0.0-latest"
        );
        assert_eq!(
            AndroidVersion::Android13.image_tag(),
            "redroid/redroid:13.0.0-latest"
        );
        assert_eq!(
            AndroidVersion::Android14.image_tag(),
            "redroid/redroid:14.0.0-latest"
        );
        assert_eq!(
            AndroidVersion::Android14.default_image_tag(),
            "redroid/redroid:14.0.0-latest"
        );

        assert!(AndroidVersion::Android11.display_label().contains("API 30"));
        assert_eq!(AndroidVersion::Android13.short_name(), "Android 13");

        let serialized = serde_json::to_string(&AndroidVersion::Android14).expect("serialize");
        let deserialized: AndroidVersion = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, AndroidVersion::Android14);
    }

    #[test]
    fn test_device_preset_properties() {
        assert_eq!(DevicePreset::ALL.len(), 4);
        assert_eq!(DevicePreset::default(), DevicePreset::GalaxyS24Ultra);

        assert_eq!(
            DevicePreset::GalaxyS24Ultra.display_name(),
            "Samsung Galaxy S24 Ultra"
        );
        assert_eq!(
            DevicePreset::Pixel8Pro.display_name(),
            "Google Pixel 8 Pro"
        );
        assert_eq!(
            DevicePreset::AsusRogPhone8.display_name(),
            "ASUS ROG Phone 8"
        );
        assert_eq!(
            DevicePreset::Custom.display_name(),
            "Custom Device Profile"
        );

        assert_eq!(DevicePreset::GalaxyS24Ultra.tac_prefix(), "35275311");
        assert_eq!(DevicePreset::Pixel8Pro.tac_prefix(), "35327511");
        assert_eq!(DevicePreset::AsusRogPhone8.tac_prefix(), "35612311");
        assert_eq!(DevicePreset::Custom.tac_prefix(), "86012304");

        let s24_ident = DevicePreset::GalaxyS24Ultra.default_identity();
        assert_eq!(s24_ident.model, "SM-S928B");

        let p8_ident = DevicePreset::Pixel8Pro.default_identity();
        assert_eq!(p8_ident.model, "Pixel 8 Pro");

        let rog_ident = DevicePreset::AsusRogPhone8.default_identity();
        assert_eq!(rog_ident.model, "ASUS_AI2401_D");
    }

    #[test]
    fn test_instance_status_transitions_and_labels() {
        assert_eq!(InstanceStatus::default(), InstanceStatus::Stopped);

        assert!(!InstanceStatus::Stopped.is_running());
        assert!(InstanceStatus::Running.is_running());

        assert!(InstanceStatus::Starting.is_active());
        assert!(InstanceStatus::WaitingForBoot.is_active());
        assert!(InstanceStatus::Running.is_active());
        assert!(InstanceStatus::Stopping.is_active());
        assert!(!InstanceStatus::Stopped.is_active());

        assert!(InstanceStatus::Starting.is_transitional());
        assert!(InstanceStatus::Snapshotting.is_transitional());
        assert!(InstanceStatus::Cloning.is_transitional());
        assert!(!InstanceStatus::Running.is_transitional());

        assert!(InstanceStatus::Stopped.can_start());
        assert!(InstanceStatus::Error("crash".to_string()).can_start());
        assert!(!InstanceStatus::Running.can_start());

        assert!(InstanceStatus::Running.can_stop());
        assert!(InstanceStatus::Starting.can_stop());
        assert!(!InstanceStatus::Stopped.can_stop());

        assert_eq!(InstanceStatus::Stopped.display_status(), "Stopped");
        assert_eq!(InstanceStatus::Starting.display_status(), "Starting...");
        assert_eq!(
            InstanceStatus::WaitingForBoot.display_status(),
            "Booting Android..."
        );
        assert_eq!(InstanceStatus::Running.display_status(), "Running");
        assert_eq!(
            InstanceStatus::Error("Failed to start".to_string()).display_status(),
            "Error: Failed to start"
        );

        assert_eq!(
            InstanceStatus::Stopped.status_icon_name(),
            "media-playback-stop-symbolic"
        );
        assert_eq!(
            InstanceStatus::Running.status_icon_name(),
            "media-playback-start-symbolic"
        );
        assert_eq!(
            InstanceStatus::Error("err".to_string()).status_icon_name(),
            "dialog-error-symbolic"
        );

        let dl = InstanceStatus::DownloadingImage("45.2 MB / 838.4 MB (5%)".to_string());
        assert!(dl.is_transitional());
        assert!(dl.is_active());
        assert!(!dl.can_start());
        assert!(dl.can_stop());
        assert_eq!(dl.display_status(), "45.2 MB / 838.4 MB (5%)");
        assert_eq!(dl.status_icon_name(), "folder-download-symbolic");

        let err_status = InstanceStatus::Error("Disk full".to_string());
        let serialized = serde_json::to_string(&err_status).expect("serialize");
        let deserialized: InstanceStatus = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, err_status);
    }

    #[test]
    fn test_image_download_progress_formatting_and_lifecycle() {
        let mut prog = ImageDownloadProgress::new("redroid/redroid:13.0.0-latest");
        assert_eq!(prog.image_tag, "redroid/redroid:13.0.0-latest");
        assert_eq!(prog.current_bytes, 0);
        assert_eq!(prog.total_bytes, 0);
        assert_eq!(prog.progress_fraction, 0.0);
        assert!(!prog.is_complete);
        assert_eq!(prog.error, None);
        assert_eq!(prog.formatted_progress(), "Contacting registry...");

        prog.current_bytes = 104857600; // 100 MB
        prog.total_bytes = 524288000;   // 500 MB
        prog.progress_fraction = 0.2;
        assert_eq!(prog.current_bytes, 104857600);
        assert_eq!(prog.total_bytes, 524288000);
        assert_eq!(prog.formatted_progress(), "100.0 MB / 500.0 MB (20%)");

        prog.is_complete = true;
        prog.status_text = "Download complete".to_string();
        prog.progress_fraction = 1.0;
        prog.current_bytes = 524288000;
        assert_eq!(prog.formatted_progress(), "500.0 MB / 500.0 MB (100%)");

        let serialized = serde_json::to_string(&prog).expect("serialize");
        let deserialized: ImageDownloadProgress =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.image_tag, prog.image_tag);
        assert_eq!(deserialized.progress_fraction, prog.progress_fraction);
        assert!(deserialized.is_complete);
    }

    #[test]
    fn test_hardware_identity_props_and_resolutions() {
        let ident = HardwareIdentity::galaxy_s24_ultra();
        assert_eq!(
            ident.resolution_display(),
            "1440x3120 @ 120 FPS (505 DPI)"
        );

        let props = ident.to_local_prop_lines();
        assert!(props.contains(&"ro.product.manufacturer=samsung".to_string()));
        assert!(props.contains(&"ro.product.model=SM-S928B".to_string()));
        assert!(props.contains(&"ro.sf.lcd_density=505".to_string()));
        assert!(props.contains(&"persist.radio.imei=352753110123456".to_string()));
    }

    #[test]
    fn test_proxy_config_methods() {
        let disabled_proxy = ProxyConfig::default();
        assert_eq!(disabled_proxy.summary_display(), "Disabled");

        let active_proxy = ProxyConfig {
            enabled: true,
            protocol: "socks5".to_string(),
            host: "192.168.1.100".to_string(),
            port: 1080,
            username: None,
            password: None,
        };
        assert_eq!(active_proxy.host_port_string(), "192.168.1.100:1080");
        assert_eq!(
            active_proxy.summary_display(),
            "socks5://192.168.1.100:1080"
        );
    }

    #[test]
    fn test_android_instance_binder_formula() {
        let instance_0 = AndroidInstance {
            id: Uuid::new_v4(),
            name: "Test 0".to_string(),
            container_name: "test-0".to_string(),
            android_version: AndroidVersion::Android13,
            image_tag: "redroid/redroid:13.0.0-latest".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity: HardwareIdentity::default(),
            proxy: ProxyConfig::default(),
            adb_port: 5555,
            data_path: PathBuf::from("/tmp/test0"),
            binder_index: 0,
            status: InstanceStatus::Stopped,
        };

        assert_eq!(instance_0.adb_address(), "127.0.0.1:5555");
        assert_eq!(instance_0.binder_indices(), (1, 2, 3));
        assert_eq!(
            instance_0.binder_nodes(),
            (
                "/dev/binder1".to_string(),
                "/dev/hwbinder2".to_string(),
                "/dev/vndbinder3".to_string(),
            )
        );

        let instance_1 = AndroidInstance {
            binder_index: 1,
            ..instance_0.clone()
        };
        assert_eq!(instance_1.binder_indices(), (4, 5, 6));
        assert_eq!(
            instance_1.binder_nodes(),
            (
                "/dev/binder4".to_string(),
                "/dev/hwbinder5".to_string(),
                "/dev/vndbinder6".to_string(),
            )
        );

        let instance_2 = AndroidInstance {
            binder_index: 2,
            ..instance_0
        };
        assert_eq!(instance_2.binder_indices(), (7, 8, 9));
        assert_eq!(
            instance_2.binder_nodes(),
            (
                "/dev/binder7".to_string(),
                "/dev/hwbinder8".to_string(),
                "/dev/vndbinder9".to_string(),
            )
        );
    }

    #[test]
    fn test_container_metrics_calculations() {
        let metrics = ContainerMetrics {
            cpu_percent: 15.42,
            memory_used_mb: 2048,
            memory_limit_mb: 4096,
        };

        assert_eq!(metrics.cpu_display(), "15.4%");
        assert_eq!(metrics.memory_percent(), 50.0);
        assert_eq!(metrics.memory_display(), "2048 / 4096 MB (50.0%)");

        let zero_limit = ContainerMetrics {
            cpu_percent: 0.0,
            memory_used_mb: 0,
            memory_limit_mb: 0,
        };
        assert_eq!(zero_limit.memory_percent(), 0.0);
    }

    #[test]
    fn test_snapshot_info_display() {
        let now = Utc::now();
        let snapshot = SnapshotInfo {
            id: Uuid::new_v4(),
            instance_id: Uuid::new_v4(),
            name: "Initial Backup".to_string(),
            created_at: now,
            archive_path: PathBuf::from("/tmp/snapshot.tar.zst"),
            size_bytes: 500 * 1024 * 1024,
        };

        assert_eq!(snapshot.size_display(), "500.0 MB");
        assert_eq!(snapshot.file_path(), Path::new("/tmp/snapshot.tar.zst"));
        assert!(snapshot.date_display().contains("UTC"));

        let large_snapshot = SnapshotInfo {
            size_bytes: 2500 * 1024 * 1024,
            ..snapshot
        };
        assert_eq!(large_snapshot.size_display(), "2.44 GB");
    }

    #[test]
    fn test_host_health_report_evaluation() {
        let perfect = HostHealthReport {
            docker_available: true,
            binder_available: true,
            gpu_available: true,
            adb_available: true,
            scrcpy_available: true,
            reflink_supported: true,
            remediation_needed: false,
            details: vec!["All probes succeeded".to_string()],
        };

        assert!(perfect.all_ok());
        assert!(perfect.is_healthy());
        assert!(!perfect.needs_remediation());

        let missing_binder = HostHealthReport {
            binder_available: false,
            remediation_needed: true,
            ..perfect
        };

        assert!(!missing_binder.all_ok());
        assert!(!missing_binder.is_healthy());
        assert!(missing_binder.needs_remediation());
    }

    #[test]
    fn test_full_roundtrip_serde() {
        let instance = AndroidInstance {
            id: Uuid::new_v4(),
            name: "Roundtrip Instance".to_string(),
            container_name: "cosmic-redroid-test".to_string(),
            android_version: AndroidVersion::Android14,
            image_tag: "redroid/redroid:14.0.0-latest".to_string(),
            preset: DevicePreset::AsusRogPhone8,
            identity: HardwareIdentity::asus_rog_phone_8(),
            proxy: ProxyConfig {
                enabled: true,
                protocol: "http".to_string(),
                host: "10.0.0.1".to_string(),
                port: 8080,
                username: Some("user".to_string()),
                password: Some("secret".to_string()),
            },
            adb_port: 5559,
            data_path: PathBuf::from("/data/instance_test"),
            binder_index: 3,
            status: InstanceStatus::Running,
        };

        let json = serde_json::to_string_pretty(&instance).expect("serialize instance");
        let decoded: AndroidInstance = serde_json::from_str(&json).expect("deserialize instance");
        assert_eq!(instance, decoded);
    }
}
