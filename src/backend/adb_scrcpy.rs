// src/backend/adb_scrcpy.rs
// Subsystem: Asynchronous ADB Automation & Detached scrcpy Spawning
// Strict zero-emoji compliance.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::time::sleep;
use tracing::{debug, info, warn};

use crate::models::{AndroidInstance, HardwareIdentity, ProxyConfig};

/// Connects to a guest Android instance over TCP ADB.
pub async fn adb_connect(port: u16) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    info!("Executing adb connect {}", target);

    let output = tokio::process::Command::new("adb")
        .arg("connect")
        .arg(&target)
        .output()
        .await
        .with_context(|| format!("Failed to execute adb connect to {target}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    debug!("adb connect output: {} {}", stdout.trim(), stderr.trim());

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "adb connect failed for {target}: {}",
            stderr.trim()
        ));
    }
    Ok(())
}

/// Disconnects from an Android instance over TCP ADB.
pub async fn adb_disconnect(port: u16) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    debug!("Executing adb disconnect {}", target);

    let output = tokio::process::Command::new("adb")
        .arg("disconnect")
        .arg(&target)
        .output()
        .await
        .with_context(|| format!("Failed to execute adb disconnect from {target}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("adb disconnect returned non-zero status: {}", stderr.trim());
    }
    Ok(())
}

/// Polls `sys.boot_completed` property over ADB until it equals "1" or timeout is reached.
pub async fn wait_for_boot(port: u16, timeout_secs: u64) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    info!(
        "Waiting for guest boot on {} (timeout {}s)",
        target, timeout_secs
    );

    let start_time = tokio::time::Instant::now();
    let max_duration = Duration::from_secs(timeout_secs);
    let poll_interval = Duration::from_millis(500);

    // Initial connect attempt
    let _ = adb_connect(port).await;

    while start_time.elapsed() < max_duration {
        let output_res = tokio::process::Command::new("adb")
            .arg("-s")
            .arg(&target)
            .arg("shell")
            .arg("getprop")
            .arg("sys.boot_completed")
            .output()
            .await;

        match output_res {
            Ok(output) if output.status.success() => {
                let status_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if status_str == "1" {
                    info!(
                        "Guest on {} reported sys.boot_completed=1 in {:.2?}",
                        target,
                        start_time.elapsed()
                    );
                    return Ok(());
                }
            }
            Ok(_) => {
                // Device might still be offline or initializing ADB daemon
                let _ = adb_connect(port).await;
            }
            Err(e) => {
                debug!("ADB poll execution transient failure: {}", e);
            }
        }

        sleep(poll_interval).await;
    }

    Err(anyhow::anyhow!(
        "Timed out after {}s waiting for instance on port {} to complete boot",
        timeout_secs,
        port
    ))
}

/// Alias for `wait_for_boot` adhering to PROJECT.md interface contract.
pub async fn poll_boot_completed(port: u16, timeout_secs: u64) -> Result<()> {
    wait_for_boot(port, timeout_secs).await
}

/// Convenience function to poll boot completed with 45-second default timeout.
pub async fn poll_boot_completed_default(port: u16) -> Result<()> {
    wait_for_boot(port, 45).await
}

/// Injects identity identifiers into the guest Android environment via ADB shell commands.
pub async fn inject_identity(port: u16, identity: &HardwareIdentity) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    info!("Injecting hardware identity into {}", target);

    // 1. Android ID
    execute_adb_shell(
        &target,
        &["settings", "put", "secure", "android_id", &identity.android_id],
    )
    .await
    .with_context(|| format!("Failed to set android_id on {target}"))?;

    // 2. Radio IMEI properties
    execute_adb_shell(
        &target,
        &["setprop", "persist.radio.imei", &identity.imei],
    )
    .await
    .with_context(|| format!("Failed to set persist.radio.imei on {target}"))?;

    execute_adb_shell(
        &target,
        &["setprop", "ro.ril.oem.imei", &identity.imei],
    )
    .await
    .with_context(|| format!("Failed to set ro.ril.oem.imei on {target}"))?;

    info!("Hardware identity injected successfully into {}", target);
    Ok(())
}

/// Injects or clears global HTTP proxy settings in the guest Android environment.
pub async fn configure_proxy(port: u16, proxy: &ProxyConfig) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    if proxy.enabled && !proxy.host.is_empty() && proxy.port > 0 {
        let proxy_val = format!("{}:{}", proxy.host, proxy.port);
        info!("Applying global HTTP proxy '{}' to {}", proxy_val, target);
        execute_adb_shell(
            &target,
            &["settings", "put", "global", "http_proxy", &proxy_val],
        )
        .await
        .with_context(|| format!("Failed to set global http_proxy on {target}"))?;
    } else {
        debug!("Disabling global HTTP proxy on {}", target);
        execute_adb_shell(
            &target,
            &["settings", "put", "global", "http_proxy", ":0"],
        )
        .await
        .with_context(|| format!("Failed to clear global http_proxy on {target}"))?;
    }
    Ok(())
}

/// Combined post-boot setup: injects both hardware identity and proxy settings.
pub async fn inject_identity_and_proxy(
    port: u16,
    identity: &HardwareIdentity,
    proxy: &ProxyConfig,
) -> Result<()> {
    inject_identity(port, identity).await?;
    configure_proxy(port, proxy).await?;
    Ok(())
}

/// Helper executing an arbitrary ADB shell command on a specific device target.
async fn execute_adb_shell(target: &str, args: &[&str]) -> Result<()> {
    let mut cmd = tokio::process::Command::new("adb");
    cmd.arg("-s").arg(target).arg("shell");
    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd
        .output()
        .await
        .with_context(|| format!("Failed to run adb command on {target}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "ADB shell command failed on {target}: {}",
            stderr.trim()
        ));
    }
    Ok(())
}

/// Builds the argument list for adb connect.
pub fn build_adb_connect_args(port: u16) -> Vec<String> {
    vec!["connect".to_string(), format!("127.0.0.1:{port}")]
}

/// Builds the argument list for adb disconnect.
pub fn build_adb_disconnect_args(port: u16) -> Vec<String> {
    vec!["disconnect".to_string(), format!("127.0.0.1:{port}")]
}

/// Builds the command list for identity injection over ADB.
pub fn build_identity_commands_args(target: &str, identity: &HardwareIdentity) -> Vec<Vec<String>> {
    vec![
        vec![
            "-s".to_string(),
            target.to_string(),
            "shell".to_string(),
            "settings".to_string(),
            "put".to_string(),
            "secure".to_string(),
            "android_id".to_string(),
            identity.android_id.clone(),
        ],
        vec![
            "-s".to_string(),
            target.to_string(),
            "shell".to_string(),
            "setprop".to_string(),
            "persist.radio.imei".to_string(),
            identity.imei.clone(),
        ],
        vec![
            "-s".to_string(),
            target.to_string(),
            "shell".to_string(),
            "setprop".to_string(),
            "ro.ril.oem.imei".to_string(),
            identity.imei.clone(),
        ],
    ]
}

/// Builds the command argument list for proxy configuration over ADB.
pub fn build_proxy_command_args(target: &str, proxy: &ProxyConfig) -> Vec<String> {
    let proxy_arg = if proxy.enabled && !proxy.host.is_empty() && proxy.port > 0 {
        format!("{}:{}", proxy.host, proxy.port)
    } else {
        ":0".to_string()
    };

    vec![
        "-s".to_string(),
        target.to_string(),
        "shell".to_string(),
        "settings".to_string(),
        "put".to_string(),
        "global".to_string(),
        "http_proxy".to_string(),
        proxy_arg,
    ]
}

/// Builds the scrcpy command argument list.
pub fn build_scrcpy_args(
    port: u16,
    title: &str,
    max_fps: u32,
    bitrate_mbps: u32,
) -> Vec<String> {
    vec![
        "-s".to_string(),
        format!("127.0.0.1:{port}"),
        "--window-title".to_string(),
        title.to_string(),
        "--max-fps".to_string(),
        max_fps.to_string(),
        "--video-bit-rate".to_string(),
        format!("{bitrate_mbps}M"),
        "--shortcut-mod".to_string(),
        "lalt".to_string(),
    ]
}

/// Constructs the scrcpy command with all required arguments and detached stdio.
pub fn build_scrcpy_command(
    port: u16,
    title: &str,
    max_fps: u32,
    bitrate_mbps: u32,
) -> Command {
    let mut cmd = Command::new("scrcpy");
    let args = build_scrcpy_args(port, title, max_fps, bitrate_mbps);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

/// Spawns a detached scrcpy process mirroring the guest container display.
pub fn spawn_scrcpy(
    port: u16,
    title: &str,
    max_fps: u32,
    bitrate_mbps: u32,
) -> Result<Child> {
    info!(
        "Spawning detached scrcpy for port {} (title='{}', max_fps={}, bitrate={}M)",
        port, title, max_fps, bitrate_mbps
    );
    let mut cmd = build_scrcpy_command(port, title, max_fps, bitrate_mbps);

    let child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn scrcpy process for instance on port {port}"))?;

    info!("scrcpy spawned successfully with PID {}", child.id());
    Ok(child)
}

/// Spawns a detached scrcpy process configured from an AndroidInstance domain model.
pub fn spawn_scrcpy_for_instance(instance: &AndroidInstance) -> Result<Child> {
    let title = format!("{} ({})", instance.name, instance.preset.display_name());
    spawn_scrcpy(instance.adb_port, &title, instance.identity.fps, 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn mock_instance(adb_port: u16, fps: u32) -> AndroidInstance {
        let mut identity = HardwareIdentity::galaxy_s24_ultra();
        identity.fps = fps;
        AndroidInstance {
            id: Uuid::new_v4(),
            name: "Test Galaxy S24".to_string(),
            container_name: "cosmic-test-1".to_string(),
            android_version: AndroidVersion::Android14,
            image_tag: "redroid/redroid:14.0.0-latest".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity,
            proxy: ProxyConfig::default(),
            adb_port,
            data_path: PathBuf::from("/tmp/cosmic_test"),
            binder_index: 0,
            status: InstanceStatus::Running,
        }
    }

    #[test]
    fn test_build_scrcpy_command_arguments() {
        let cmd = build_scrcpy_command(5555, "Test Device", 120, 16);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert_eq!(
            args,
            vec![
                "-s",
                "127.0.0.1:5555",
                "--window-title",
                "Test Device",
                "--max-fps",
                "120",
                "--video-bit-rate",
                "16M",
                "--shortcut-mod",
                "lalt"
            ]
        );
    }

    #[test]
    fn test_scrcpy_command_program_and_args() {
        let cmd = build_scrcpy_command(5557, "Another Device", 60, 8);
        assert_eq!(cmd.get_program(), "scrcpy");

        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert_eq!(args[1], "127.0.0.1:5557");
        assert_eq!(args[3], "Another Device");
        assert_eq!(args[5], "60");
        assert_eq!(args[7], "8M");
        assert_eq!(args[9], "lalt");
    }

    #[test]
    fn test_build_scrcpy_args_direct() {
        let args = build_scrcpy_args(5559, "ROG Phone", 165, 24);
        assert_eq!(args[1], "127.0.0.1:5559");
        assert_eq!(args[3], "ROG Phone");
        assert_eq!(args[5], "165");
        assert_eq!(args[7], "24M");
        assert_eq!(args[9], "lalt");
    }

    #[test]
    fn test_adb_connect_and_disconnect_args() {
        let connect_args = build_adb_connect_args(5555);
        assert_eq!(connect_args, vec!["connect", "127.0.0.1:5555"]);

        let disconnect_args = build_adb_disconnect_args(5555);
        assert_eq!(disconnect_args, vec!["disconnect", "127.0.0.1:5555"]);
    }

    #[test]
    fn test_build_identity_commands_structure() {
        let identity = HardwareIdentity::pixel_8_pro();
        let target = "127.0.0.1:5557";
        let commands = build_identity_commands_args(target, &identity);

        assert_eq!(commands.len(), 3);

        // 1. Android ID command
        assert_eq!(
            commands[0],
            vec![
                "-s",
                target,
                "shell",
                "settings",
                "put",
                "secure",
                "android_id",
                &identity.android_id
            ]
        );

        // 2. persist.radio.imei command
        assert_eq!(
            commands[1],
            vec![
                "-s",
                target,
                "shell",
                "setprop",
                "persist.radio.imei",
                &identity.imei
            ]
        );

        // 3. ro.ril.oem.imei command
        assert_eq!(
            commands[2],
            vec![
                "-s",
                target,
                "shell",
                "setprop",
                "ro.ril.oem.imei",
                &identity.imei
            ]
        );
    }

    #[test]
    fn test_build_proxy_command_enabled() {
        let proxy = ProxyConfig {
            enabled: true,
            protocol: "http".to_string(),
            host: "192.168.1.100".to_string(),
            port: 8888,
            username: None,
            password: None,
        };
        let target = "127.0.0.1:5555";
        let cmd = build_proxy_command_args(target, &proxy);

        assert_eq!(
            cmd,
            vec![
                "-s",
                target,
                "shell",
                "settings",
                "put",
                "global",
                "http_proxy",
                "192.168.1.100:8888"
            ]
        );
    }

    #[test]
    fn test_build_proxy_command_disabled_clears_proxy() {
        let proxy = ProxyConfig::default();
        let target = "127.0.0.1:5555";
        let cmd = build_proxy_command_args(target, &proxy);

        assert_eq!(
            cmd,
            vec![
                "-s",
                target,
                "shell",
                "settings",
                "put",
                "global",
                "http_proxy",
                ":0"
            ]
        );
    }

    #[test]
    fn test_build_proxy_command_empty_host_clears_proxy() {
        let proxy = ProxyConfig {
            enabled: true,
            protocol: "http".to_string(),
            host: "".to_string(),
            port: 8080,
            username: None,
            password: None,
        };
        let target = "127.0.0.1:5555";
        let cmd = build_proxy_command_args(target, &proxy);
        assert_eq!(cmd[7], ":0");
    }

    #[test]
    fn test_spawn_scrcpy_for_instance_title_and_fps() {
        let instance = mock_instance(5555, 120);
        let cmd = build_scrcpy_command(
            instance.adb_port,
            &format!("{} ({})", instance.name, instance.preset.display_name()),
            instance.identity.fps,
            16,
        );
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        assert_eq!(args[3], "Test Galaxy S24 (Samsung Galaxy S24 Ultra)");
        assert_eq!(args[5], "120");
        assert_eq!(args[7], "16M");
    }
}
