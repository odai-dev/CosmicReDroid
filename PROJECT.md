# Project: CosmicReDroid

## Architecture
CosmicReDroid is a native Linux Android Multi-Instance Control Center built on System76's `libcosmic` toolkit and powered by ReDroid (Remote-Android) containers. The system is architected into clean, decoupled layers adhering to the official `cosmic-app-template` layout:

1. **Presentation Layer (`src/app.rs`, `src/ui/`)**:
   - Implements `cosmic::Application for AppModel` using `cosmic::app::Task<Message>` async execution.
   - Header with active container counter badge, refresh health trigger, and instance creation action.
   - 4-page navigation bar (`nav_bar::Model`) routing between Dashboard, Image Factory, Snapshots, and Host Health.
   - Dashboard (`src/ui/dashboard.rs`): Card grid with live CPU/RAM progress bars, status indicators, and lifecycle controls.
   - Context Drawer (`src/ui/drawer.rs`): Sliding side panel for device preset configuration, identity randomization, and proxy editing.
   - Image Factory (`src/ui/image_hub.rs`): Custom ReDroid Dockerfile generator and async build streamer.
   - Host Health & Snapshots (`src/ui/host_health.rs`): Diagnostic probe matrix with one-click `pkexec` remediation, plus snapshot archive manager.
   - Strict Zero-Emoji Rule: All actions, navigation items, and indicators use FreeDesktop XDG symbolic icons (`cosmic::widget::icon::from_name(...)`).

2. **Domain & Configuration Layer (`src/models.rs`, `src/config.rs`)**:
   - Pure domain models (`NavPage`, `AndroidVersion`, `DevicePreset`, `InstanceStatus`, `HardwareIdentity`, `ProxyConfig`, `AndroidInstance`, `ContainerMetrics`, `SnapshotInfo`, `HostHealthReport`).
   - JSON persistence at `~/.local/share/cosmic-redroid/instances.json` using atomic temporary file write and rename.
   - Default seeding on initial run: "Instance 1 - Galaxy S24 Ultra" (port 5555, binder 0) and "Instance 2 - Pixel 8 Pro" (port 5557, binder 1).

3. **Backend Subsystems Layer (`src/backend/`)**:
   - `spoofing.rs`: Hardware preset factory (Galaxy S24 Ultra, Pixel 8 Pro, ASUS ROG Phone 8), Luhn-valid 15-digit IMEI generator, 11-char alphanumeric serial, 16-char hex Android ID, locally administered MAC generator, `/data/local.prop` writer, and kernel boot arguments synthesizer.
   - `docker.rs`: Bollard 0.18 container lifecycle (pull, create, start, stop, delete, inspect, stats), deterministic multi-instance binder isolation ($b_1 = 3i+1, b_2 = 3i+2, b_3 = 3i+3$), GPU pass-through (`/dev/dri/renderD128`), privileged mode, and CPU/RAM telemetry.
   - `adb_scrcpy.rs`: Async ADB boot completion polling (`sys.boot_completed`), secure property injection, global HTTP proxy injection, and detached `scrcpy` invocation.
   - `storage.rs`: CoW cloning via `cp -a --reflink=auto`, available port and binder index allocators, `.tar.zst` snapshot archive creation and restoration.
   - `host.rs`: Host capability probes (Docker daemon, binder nodes, DRI permissions, ADB, scrcpy, reflink support) and `pkexec` remediation helper.

4. **Scaffolding & Packaging Layer**:
   - Conforms strictly to `cosmic-app-template`: `Cargo.toml`, `justfile`, `i18n.toml`, `i18n/en/cosmic_redroid.ftl`, `resources/app.desktop`, `resources/app.metainfo.xml`, and SVG icon.
   - Atomic Conventional Commits (`chore(scaffold)`, `feat(models)`, `feat(backend)`, `feat(ui)`, `fix(build)`).

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Repository Scaffolding & Dependencies | `cosmic-app-template` layout, exact Cargo.toml dependencies with libcosmic git features | M1 | R1 |
| 2 | Desktop & AppStream Metadata | `com.system76.CosmicRedroid.desktop`, metainfo XML, and scalable SVG icon | M1 | R1 |
| 3 | Task Automation & Justfile | Build, run, check, test, and i18n recipes in `justfile` | M1 | R1 |
| 4 | Localization System | Fluent i18n configuration, `i18n.toml`, and English dictionary `cosmic_redroid.ftl` | M1 | R1 |
| 5 | Domain Models Definition | Full structs and enums in `src/models.rs` with Serde derive | M2 | R2 |
| 6 | Persistent Configuration | JSON storage in `~/.local/share/cosmic-redroid/instances.json` with atomic rename | M2 | R2 |
| 7 | Default Instance Seeding | Auto-generation of Galaxy S24 Ultra (5555, b0) and Pixel 8 Pro (5557, b1) on first launch | M2 | R2 |
| 8 | Hardware Identity Presets | Factory presets for Galaxy S24 Ultra, Pixel 8 Pro, and ASUS ROG Phone 8 | M3 | R3 |
| 9 | Mathematical Identifier Generators | Luhn 15-digit IMEI, 11-char serial, 16-char hex Android ID, MAC address generator | M3 | R3 |
| 10 | ReDroid Boot Argument & Prop Synthesis | Generating `/data/local.prop` and ReDroid kernel boot arguments | M3 | R3 |
| 11 | Docker Container Engine (Bollard) | Image pulling, container create/start/stop/delete/inspect via Bollard 0.18 | M3 | R3 |
| 12 | Deterministic Binder Isolation | Mapping binder nodes per instance index ($b_1 = 3i+1, b_2 = 3i+2, b_3 = 3i+3$) | M3 | R3 |
| 13 | Container Telemetry | Async CPU % and RAM MB telemetry from Docker stats | M3 | R3 |
| 14 | ADB Polling & Boot Verification | Async ADB connect with 45-second polling loop for `sys.boot_completed` | M3 | R3 |
| 15 | Android Identity & Proxy Injection | In-guest injection of android_id, imei, and global HTTP proxy settings | M3 | R3 |
| 16 | scrcpy Detached Spawning | Spawning scrcpy with title, max-fps, 16M bitrate, and `lalt` shortcut modifier | M3 | R3 |
| 17 | CoW Instance Cloning | Instant cloning via `cp -a --reflink=auto` with port and binder re-allocation | M3 | R3 |
| 18 | Zstandard Snapshot Engine | Multi-threaded `.tar.zst` creation and restoration in `snapshots/<id>/` | M3 | R3 |
| 19 | Host Diagnostics & Probes | Verification of Docker daemon, binder nodes, DRI permissions, adb, scrcpy, reflink | M3 | R3 |
| 20 | Host Permission Remediation | Async `pkexec` script to load binder modules and set permissions | M3 | R3 |
| 21 | libcosmic Application Shell | `cosmic::Application for AppModel` with Task async loop and single-instance support | M4 | R4 |
| 22 | Navigation Bar & Header | 4-page `nav_bar::Model` and header with active counter badge and action triggers | M4 | R4 |
| 23 | Instances Dashboard Grid | Card grid with status badges, CPU/RAM progress bars, and action controls | M4 | R4 |
| 24 | Context Drawer Panel | Right-sliding panel for preset selection, identity randomizer, and proxy toggle | M4 | R4 |
| 25 | Image Factory View | Version selection, feature toggles (libndk, GApps, Magisk), and build streamer | M4 | R4 |
| 26 | Snapshots & Host Health View | Snapshot archive management table and host diagnostic checklist with fix button | M4 | R4 |
| 27 | Strict Zero-Emoji Rule | Complete replacement of glyphs with FreeDesktop XDG symbolic icons | M4 | R4 |
| 28 | Atomic Git Commit Sequence | Clean Conventional Commits corresponding to each architectural stage | M1-M5 | R5 |
| 29 | Comprehensive Test Suite & Stubs Elimination | Pass `cargo check` and `cargo test` with zero `todo!()` or `unimplemented!()` stubs | M5 | Acceptance |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Scaffolding & Packaging | `cosmic-app-template` layout, Cargo.toml, justfile, i18n, desktop, icons, initial entry point | None | PLANNED |
| M2 | Domain Models & Config | `src/models.rs`, `src/config.rs`, tests, default seeding | M1 | PLANNED |
| M3 | Backend Subsystems | `src/backend/` (spoofing, docker, adb_scrcpy, storage, host), backend tests | M2 | PLANNED |
| M4 | UI Architecture & Views | `src/ui/` (dashboard, drawer, image_hub, host_health), `src/app.rs`, zero emojis | M2, M3 | PLANNED |
| M5 | E2E Testing & Acceptance | Verification of cargo check, cargo test, zero stubs, zero emojis, clean git log | M1, M2, M3, M4 | PLANNED |

---

## Interface Contracts

### `src/models.rs` ↔ `src/config.rs`
```rust
pub fn config_file_path() -> Result<std::path::PathBuf, anyhow::Error>;
pub fn load_instances() -> Result<Vec<AndroidInstance>, anyhow::Error>;
pub fn save_instances(instances: &[AndroidInstance]) -> Result<(), anyhow::Error>;
pub fn seed_default_instances() -> Vec<AndroidInstance>;
```

### `src/models.rs` ↔ `src/backend/spoofing.rs`
```rust
pub fn preset_for_device(preset: DevicePreset) -> HardwareIdentity;
pub fn generate_luhn_imei(tac_prefix: &str) -> String;
pub fn generate_serial() -> String;
pub fn generate_android_id() -> String;
pub fn generate_mac_address() -> String;
pub fn write_local_prop(data_path: &std::path::Path, identity: &HardwareIdentity) -> Result<(), anyhow::Error>;
pub fn generate_boot_params(identity: &HardwareIdentity) -> Vec<String>;
```

### `src/models.rs` ↔ `src/backend/docker.rs`
```rust
pub struct DockerEngine { /* bollard client */ }
impl DockerEngine {
    pub async fn new() -> Result<Self, anyhow::Error>;
    pub async fn pull_image(&self, image: &str) -> Result<(), anyhow::Error>;
    pub async fn create_instance_container(&self, instance: &AndroidInstance) -> Result<String, anyhow::Error>;
    pub async fn start_container(&self, container_name: &str) -> Result<(), anyhow::Error>;
    pub async fn stop_container(&self, container_name: &str) -> Result<(), anyhow::Error>;
    pub async fn delete_container(&self, container_name: &str) -> Result<(), anyhow::Error>;
    pub async fn inspect_container(&self, container_name: &str) -> Result<bollard::models::ContainerInspectResponse, anyhow::Error>;
    pub async fn get_container_metrics(&self, container_name: &str) -> Result<ContainerMetrics, anyhow::Error>;
}
```

### `src/models.rs` ↔ `src/backend/adb_scrcpy.rs`
```rust
pub async fn poll_boot_completed(port: u16, timeout_secs: u64) -> Result<(), anyhow::Error>;
pub async fn inject_identity_and_proxy(port: u16, identity: &HardwareIdentity, proxy: &ProxyConfig) -> Result<(), anyhow::Error>;
pub fn spawn_scrcpy(port: u16, title: &str, max_fps: u32, bitrate_mbps: u32) -> Result<std::process::Child, anyhow::Error>;
```

### `src/models.rs` ↔ `src/backend/storage.rs`
```rust
pub fn allocate_next_port_and_binder(instances: &[AndroidInstance]) -> (u16, u32);
pub async fn clone_instance_data(src_path: &std::path::Path, dst_path: &std::path::Path) -> Result<(), anyhow::Error>;
pub async fn create_snapshot(instance_id: &uuid::Uuid, data_path: &std::path::Path, snapshot_name: &str) -> Result<SnapshotInfo, anyhow::Error>;
pub async fn restore_snapshot(snapshot: &SnapshotInfo, data_path: &std::path::Path) -> Result<(), anyhow::Error>;
pub fn list_snapshots(instance_id: &uuid::Uuid) -> Result<Vec<SnapshotInfo>, anyhow::Error>;
```

### `src/backend/host.rs` ↔ UI / Health
```rust
pub async fn probe_host_health() -> HostHealthReport;
pub async fn remediate_host_environment() -> Result<(), anyhow::Error>;
```

---

## Code Layout
```
/home/odai/Documents/Coding/CosmicReDroid/
├── Cargo.toml
├── justfile
├── i18n.toml
├── i18n/
│   └── en/
│       └── cosmic_redroid.ftl
├── resources/
│   ├── app.desktop
│   ├── com.system76.CosmicRedroid.desktop -> app.desktop
│   ├── app.metainfo.xml
│   └── icons/
│       └── hicolor/
│           └── scalable/
│               └── apps/
│                   └── icon.svg
├── src/
│   ├── main.rs
│   ├── app.rs
│   ├── config.rs
│   ├── i18n.rs
│   ├── models.rs
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── spoofing.rs
│   │   ├── docker.rs
│   │   ├── adb_scrcpy.rs
│   │   ├── storage.rs
│   │   └── host.rs
│   └── ui/
│       ├── mod.rs
│       ├── dashboard.rs
│       ├── drawer.rs
│       ├── image_hub.rs
│       └── host_health.rs
└── tests/
    └── e2e_integration.rs
```
