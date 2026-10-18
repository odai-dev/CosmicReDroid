// src/backend/host.rs
// Host environment capability probes (Docker, binderfs, DRI GPU, ADB, scrcpy, reflink)
// and asynchronous pkexec permission remediation helper.

#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::Path;

use crate::models::HostHealthReport;

/// Probes Docker daemon connectivity via bollard's local defaults.
pub async fn probe_docker() -> (bool, String) {
    match bollard::Docker::connect_with_local_defaults() {
        Ok(docker) => match docker.version().await {
            Ok(ver) => {
                let v = ver.version.unwrap_or_else(|| "unknown".to_string());
                let api = ver.api_version.unwrap_or_else(|| "unknown".to_string());
                (true, format!("Docker daemon active: version {} (API {})", v, api))
            }
            Err(err) => (false, format!("Docker daemon connection failed: {}", err)),
        },
        Err(err) => (false, format!("Failed to initialize Docker client: {}", err)),
    }
}

/// Probes Linux IPC binder device nodes (/dev/binderfs or /dev/binder).
pub fn probe_binder() -> (bool, String) {
    let binderfs_binder = Path::new("/dev/binderfs/binder");
    let dev_binder = Path::new("/dev/binder");

    if binderfs_binder.exists() {
        let writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(binderfs_binder)
            .is_ok();
        if writable {
            (
                true,
                "Binder IPC available via /dev/binderfs (read/write confirmed)".to_string(),
            )
        } else {
            (
                false,
                "Binder IPC found at /dev/binderfs but lacks write permissions (run pkexec remediation)".to_string(),
            )
        }
    } else if dev_binder.exists() {
        let writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dev_binder)
            .is_ok();
        if writable {
            (
                true,
                "Binder IPC available via /dev/binder (read/write confirmed)".to_string(),
            )
        } else {
            (
                false,
                "Binder IPC found at /dev/binder but lacks write permissions (run pkexec remediation)".to_string(),
            )
        }
    } else {
        (
            false,
            "Binder IPC nodes not found (/dev/binderfs or /dev/binder missing). Kernel module binder_linux required".to_string(),
        )
    }
}

/// Probes GPU DRI render nodes (/dev/dri/renderD128) for hardware acceleration.
pub fn probe_gpu_dri() -> (bool, String) {
    let primary_render = Path::new("/dev/dri/renderD128");

    if primary_render.exists() {
        let writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(primary_render)
            .is_ok();
        if writable {
            (
                true,
                "Direct Rendering Infrastructure (DRI) node /dev/dri/renderD128 accessible".to_string(),
            )
        } else {
            (
                false,
                "DRI node /dev/dri/renderD128 exists but lacks write permissions (add user to 'render' group or run remediation)".to_string(),
            )
        }
    } else if let Ok(entries) = std::fs::read_dir("/dev/dri") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("renderD") {
                    let writable = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&path)
                        .is_ok();
                    if writable {
                        return (
                            true,
                            format!(
                                "Direct Rendering Infrastructure (DRI) node {} accessible",
                                path.display()
                            ),
                        );
                    } else {
                        return (
                            false,
                            format!(
                                "DRI node {} exists but lacks write permissions (add user to 'render' group)",
                                path.display()
                            ),
                        );
                    }
                }
            }
        }
        (
            false,
            "No accessible GPU DRI render node found in /dev/dri (fallback to guest CPU rendering)".to_string(),
        )
    } else {
        (
            false,
            "No GPU DRI directory found at /dev/dri (hardware acceleration unavailable, guest CPU fallback)".to_string(),
        )
    }
}

/// Probes the existence and version of the ADB executable.
pub async fn probe_adb() -> (bool, String) {
    match tokio::process::Command::new("adb")
        .arg("version")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_line = stdout.lines().next().unwrap_or("ADB available").trim();
            (true, format!("ADB CLI detected: {}", first_line))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            (
                false,
                format!(
                    "ADB CLI execution failed with code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                ),
            )
        }
        Err(err) => (
            false,
            format!("ADB binary not found in PATH: {} (install android-tools-adb)", err),
        ),
    }
}

/// Probes the existence and version of the scrcpy display mirroring binary.
pub async fn probe_scrcpy() -> (bool, String) {
    match tokio::process::Command::new("scrcpy")
        .arg("--version")
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_line = stdout.lines().next().unwrap_or("scrcpy available").trim();
            (true, format!("scrcpy display engine detected: {}", first_line))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            (
                false,
                format!(
                    "scrcpy check failed with code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                ),
            )
        }
        Err(err) => (
            false,
            format!("scrcpy binary not found in PATH: {} (install scrcpy)", err),
        ),
    }
}

/// Probes whether the filesystem hosting instance storage supports instant Copy-on-Write (reflink) cloning.
pub async fn probe_reflink() -> (bool, String) {
    let base_dir = crate::config::config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("cosmic-redroid"));
    let probe_dir = base_dir.join(".probe_reflink");
    let _ = tokio::fs::create_dir_all(&probe_dir).await;

    let test_id = uuid::Uuid::new_v4();
    let src_file = probe_dir.join(format!(".reflink_test_src_{}.tmp", test_id));
    let dst_file = probe_dir.join(format!(".reflink_test_dst_{}.tmp", test_id));

    if let Err(err) = tokio::fs::write(&src_file, b"cosmic-redroid-reflink-probe").await {
        let _ = tokio::fs::remove_dir(&probe_dir).await;
        return (
            false,
            format!("Failed to create temporary file for reflink probe: {}", err),
        );
    }

    let result = tokio::process::Command::new("cp")
        .arg("--reflink=always")
        .arg(&src_file)
        .arg(&dst_file)
        .output()
        .await;

    let _ = tokio::fs::remove_file(&src_file).await;
    let _ = tokio::fs::remove_file(&dst_file).await;
    let _ = tokio::fs::remove_dir(&probe_dir).await;

    match result {
        Ok(output) if output.status.success() => (
            true,
            "Filesystem supports instant Copy-on-Write (reflink) cloning".to_string(),
        ),
        Ok(_) => (
            false,
            "Filesystem does not support CoW reflink (ext4 or unsupported fs, fallback to byte copying)".to_string(),
        ),
        Err(err) => (
            false,
            format!("Failed to invoke cp binary for reflink check: {}", err),
        ),
    }
}

/// Executes all diagnostic probes in parallel/sequence and compiles a comprehensive HostHealthReport.
pub async fn probe_host_health() -> HostHealthReport {
    let (docker_available, docker_msg) = probe_docker().await;
    let (binder_available, binder_msg) = probe_binder();
    let (gpu_available, gpu_msg) = probe_gpu_dri();
    let (adb_available, adb_msg) = probe_adb().await;
    let (scrcpy_available, scrcpy_msg) = probe_scrcpy().await;
    let (reflink_supported, reflink_msg) = probe_reflink().await;

    let remediation_needed = !binder_available
        || !docker_available
        || (!gpu_available && Path::new("/dev/dri/renderD128").exists());

    let details = vec![
        docker_msg,
        binder_msg,
        gpu_msg,
        adb_msg,
        scrcpy_msg,
        reflink_msg,
    ];

    HostHealthReport {
        docker_available,
        binder_available,
        gpu_available,
        adb_available,
        scrcpy_available,
        reflink_supported,
        remediation_needed,
        details,
    }
}

/// Alias for `probe_host_health()` matching survey_backend.md exports.
pub async fn run_host_diagnostics() -> HostHealthReport {
    probe_host_health().await
}

/// Returns the shell remediation script executed with elevated privileges via pkexec.
pub fn generate_remediation_script() -> &'static str {
    r#"#!/bin/bash
set -e
# 1. Load binder kernel module if available
modprobe binder_linux 2>/dev/null || true

# 2. Mount binderfs if not already mounted
mkdir -p /dev/binderfs
mount -t binder binder /dev/binderfs -o stats=global 2>/dev/null || true

# 3. Symlink standard nodes if present in binderfs
[ -e /dev/binderfs/binder ] && ln -sf /dev/binderfs/binder /dev/binder
[ -e /dev/binderfs/hwbinder ] && ln -sf /dev/binderfs/hwbinder /dev/hwbinder
[ -e /dev/binderfs/vndbinder ] && ln -sf /dev/binderfs/vndbinder /dev/vndbinder

# 4. Ensure permissive access for unprivileged containers
chmod 755 /dev/binderfs 2>/dev/null || true
chmod 666 /dev/binder /dev/hwbinder /dev/vndbinder /dev/binderfs/* 2>/dev/null || true
chmod 666 /dev/dri/renderD* 2>/dev/null || true

# 5. Ensure user groups for docker and render
TARGET_USER="${SUDO_USER:-$USER}"
if [ -n "$TARGET_USER" ] && [ "$TARGET_USER" != "root" ]; then
    getent group docker >/dev/null && usermod -aG docker "$TARGET_USER" || true
    getent group render >/dev/null && usermod -aG render "$TARGET_USER" || true
fi
"#
}

/// Invokes pkexec to execute the host permission and binder remediation script.
pub async fn remediate_host_environment() -> Result<()> {
    let script = generate_remediation_script();
    let output = tokio::process::Command::new("pkexec")
        .arg("bash")
        .arg("-c")
        .arg(script)
        .output()
        .await
        .context("Failed to spawn pkexec process for host remediation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "pkexec remediation failed with exit code {:?}: {}\n{}",
            output.status.code(),
            stderr.trim(),
            stdout.trim()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_remediation_script_content() {
        let script = generate_remediation_script();
        assert!(script.starts_with("#!/bin/bash\nset -e\n"));
        assert!(script.contains("modprobe binder_linux"));
        assert!(script.contains("mount -t binder binder /dev/binderfs"));
        assert!(script.contains("chmod 666 /dev/binder"));
        assert!(script.contains("usermod -aG docker"));
        assert!(script.contains("usermod -aG render"));
    }

    #[tokio::test]
    async fn test_probe_docker_returns_non_empty_status() {
        let (_ok, msg) = probe_docker().await;
        assert!(!msg.is_empty(), "Docker probe message must not be empty");
    }

    #[test]
    fn test_probe_binder_returns_valid_message() {
        let (_ok, msg) = probe_binder();
        assert!(!msg.is_empty(), "Binder probe message must not be empty");
    }

    #[test]
    fn test_probe_gpu_dri_returns_valid_message() {
        let (_ok, msg) = probe_gpu_dri();
        assert!(!msg.is_empty(), "GPU DRI probe message must not be empty");
    }

    #[tokio::test]
    async fn test_probe_adb_returns_valid_message() {
        let (_ok, msg) = probe_adb().await;
        assert!(!msg.is_empty(), "ADB probe message must not be empty");
    }

    #[tokio::test]
    async fn test_probe_scrcpy_returns_valid_message() {
        let (_ok, msg) = probe_scrcpy().await;
        assert!(!msg.is_empty(), "scrcpy probe message must not be empty");
    }

    #[tokio::test]
    async fn test_probe_reflink_executes_safely() {
        let (_supported, msg) = probe_reflink().await;
        assert!(!msg.is_empty(), "Reflink probe message must not be empty");
    }

    #[tokio::test]
    async fn test_probe_host_health_aggregates_all_six_probes() {
        let report = probe_host_health().await;
        assert_eq!(
            report.details.len(),
            6,
            "HostHealthReport must contain exactly 6 diagnostic probe details"
        );
        for detail in &report.details {
            assert!(!detail.is_empty(), "Each detail item must be non-empty");
        }
    }

    #[tokio::test]
    async fn test_run_host_diagnostics_alias_matches_probe_host_health() {
        let report = run_host_diagnostics().await;
        assert_eq!(report.details.len(), 6);
    }
}
