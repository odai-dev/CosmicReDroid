use cosmic_redroid::backend::adb_scrcpy::{
    build_adb_connect_args, build_adb_disconnect_args, build_identity_commands_args,
    build_proxy_command_args, build_scrcpy_args,
};
use cosmic_redroid::backend::docker::{
    build_container_config, build_container_host_config, resolve_binder_mappings,
};
use cosmic_redroid::backend::host::{generate_remediation_script, probe_host_health};
use cosmic_redroid::models::{HardwareIdentity, ProxyConfig};

#[tokio::test]
async fn test_host_health_probing() {
    let report = probe_host_health().await;
    // Health report fields should be valid booleans and contain status details
    assert!(!report.details.is_empty(), "Report details must not be empty");
}

#[test]
fn test_remediation_script_content_and_safety() {
    let script = generate_remediation_script();

    assert!(
        script.starts_with("#!/bin/bash"),
        "Remediation script must begin with bash shebang"
    );
    assert!(
        script.contains("set -e"),
        "Remediation script must execute with set -e for strict error halting"
    );
    assert!(
        script.contains("modprobe binder_linux") || script.contains("binderfs"),
        "Script must attempt kernel module or binderfs loading"
    );
    assert!(
        script.contains("mkdir -p /dev/binderfs"),
        "Script must create binderfs directory"
    );
}

#[test]
fn test_adb_argument_injection_safety() {
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
        let mut identity = HardwareIdentity::galaxy_s24_ultra();
        identity.android_id = payload.to_string();
        identity.imei = payload.to_string();

        let target = "127.0.0.1:5555";
        let commands = build_identity_commands_args(target, &identity);

        assert_eq!(commands.len(), 3);
        for cmd in &commands {
            assert_eq!(cmd[0], "-s");
            assert_eq!(cmd[1], target);
            assert_eq!(cmd[2], "shell");

            // Verify payload is kept as an isolated argv element, not executed in shell
            let contains_payload = cmd.iter().any(|arg| arg == payload);
            assert!(
                contains_payload,
                "Payload must be preserved exactly as an individual argv item"
            );
        }

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
        assert_eq!(proxy_cmd[7], format!("{}:8080", payload));
    }
}

#[test]
fn test_adb_connect_and_disconnect_command_construction() {
    let connect_args = build_adb_connect_args(5555);
    assert_eq!(connect_args, vec!["connect", "127.0.0.1:5555"]);

    let disconnect_args = build_adb_disconnect_args(5557);
    assert_eq!(disconnect_args, vec!["disconnect", "127.0.0.1:5557"]);
}

#[test]
fn test_scrcpy_command_formatting() {
    let args = build_scrcpy_args(5555, "Cosmic ReDroid - S24", 60, 8);
    assert_eq!(args[0], "-s");
    assert_eq!(args[1], "127.0.0.1:5555");
    assert_eq!(args[2], "--window-title");
    assert_eq!(args[3], "Cosmic ReDroid - S24");
    assert_eq!(args[4], "--max-fps");
    assert_eq!(args[5], "60");
    assert_eq!(args[6], "--video-bit-rate");
    assert_eq!(args[7], "8M");
}

#[test]
fn test_docker_container_config_generation() {
    let defaults = cosmic_redroid::config::seed_default_instances();
    let inst = defaults[0].clone();

    let host_config = build_container_host_config(&inst);
    assert_eq!(host_config.privileged, Some(true));

    let container_config = build_container_config(&inst);
    assert_eq!(container_config.image, Some(inst.image_tag.clone()));

    let binder_mappings = resolve_binder_mappings(inst.binder_index);
    assert!(!binder_mappings.is_empty());
}
