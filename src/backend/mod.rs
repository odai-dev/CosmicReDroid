// src/backend/mod.rs
// Backend subsystems for CosmicReDroid.

pub mod host;
pub mod spoofing;
pub mod storage;

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
