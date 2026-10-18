// src/backend/mod.rs
// Backend subsystems for CosmicReDroid.

pub mod adb_scrcpy;
pub mod docker;
pub mod host;
pub mod spoofing;
pub mod storage;

pub use adb_scrcpy::{
    adb_connect, adb_disconnect, build_adb_connect_args, build_adb_disconnect_args,
    build_identity_commands_args, build_proxy_command_args, build_scrcpy_args,
    build_scrcpy_command, configure_proxy, inject_identity, inject_identity_and_proxy,
    poll_boot_completed, poll_boot_completed_default, spawn_scrcpy, spawn_scrcpy_for_instance,
    wait_for_boot,
};
pub use docker::{
    build_container_boot_args, build_container_config, build_container_config_with_root,
    build_container_host_config, build_container_host_config_with_root,
    calculate_metrics_from_stats, create_container, create_device_mapping,
    create_instance_container, delete_container, get_container_metrics, get_container_stats,
    inspect_container, pull_image, resolve_binder_mappings, resolve_binder_mappings_with_root,
    start_container, stop_container, DockerEngine,
};
pub use host::{
    generate_remediation_script, probe_adb, probe_binder, probe_docker, probe_gpu_dri,
    probe_host_health, probe_reflink, probe_scrcpy, remediate_host_environment,
    run_host_diagnostics,
};
pub use spoofing::{
    build_redroid_boot_args, generate_android_id, generate_boot_params, generate_luhn_imei,
    generate_mac_address, generate_serial, populate_preset, preset_for_device, verify_luhn,
    write_local_prop, write_local_prop_async,
};
pub use storage::{
    allocate_next_port_and_binder, clone_instance, clone_instance_data, create_snapshot,
    delete_snapshot, delete_snapshot_by_id, find_next_adb_port, find_next_binder_index,
    list_snapshots, regenerate_instance_identity, restore_snapshot, ADB_PORT_BASE, ADB_PORT_STEP,
};
