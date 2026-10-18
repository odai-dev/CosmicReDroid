// src/backend/docker.rs
// Subsystem: Docker Container Engine via Bollard 0.18
// Strict zero-emoji compliance.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions,
    StartContainerOptions, Stats, StatsOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{ContainerInspectResponse, DeviceMapping, HostConfig, PortBinding, PortMap};
use bollard::Docker;
use futures_util::StreamExt;
use tracing::{debug, info, warn};

use crate::models::{AndroidInstance, ContainerMetrics};

/// High-level wrapper around the Bollard Docker client.
#[derive(Clone, Debug)]
pub struct DockerEngine {
    client: Docker,
}

impl DockerEngine {
    /// Connects to the local Docker daemon using standard OS socket defaults.
    pub fn new() -> Result<Self> {
        let client = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon with local defaults")?;
        Ok(Self { client })
    }

    /// Asynchronously connects to the local Docker daemon and validates connection via ping.
    pub async fn connect() -> Result<Self> {
        let engine = Self::new()?;
        engine
            .client
            .ping()
            .await
            .context("Failed to ping Docker daemon")?;
        Ok(engine)
    }

    /// Wraps an existing Bollard Docker client instance.
    pub fn from_client(client: Docker) -> Self {
        Self { client }
    }

    /// Returns a reference to the underlying Bollard Docker client.
    pub fn client(&self) -> &Docker {
        &self.client
    }

    /// Pulls a container image, streaming progress until completion.
    pub async fn pull_image(&self, image: &str) -> Result<()> {
        pull_image(&self.client, image).await
    }

    /// Creates a ReDroid container configured according to the provided instance domain model.
    pub async fn create_instance_container(&self, instance: &AndroidInstance) -> Result<String> {
        create_container(&self.client, instance).await
    }

    /// Alias for create_instance_container.
    pub async fn create_container(&self, instance: &AndroidInstance) -> Result<String> {
        create_container(&self.client, instance).await
    }

    /// Starts an existing container by name.
    pub async fn start_container(&self, container_name: &str) -> Result<()> {
        start_container(&self.client, container_name).await
    }

    /// Stops a running container by name with default 10-second timeout.
    pub async fn stop_container(&self, container_name: &str) -> Result<()> {
        stop_container(&self.client, container_name, Some(10)).await
    }

    /// Stops a running container by name with a graceful timeout.
    pub async fn stop_container_with_timeout(
        &self,
        container_name: &str,
        timeout_secs: Option<i64>,
    ) -> Result<()> {
        stop_container(&self.client, container_name, timeout_secs).await
    }

    /// Forcefully deletes a container by name.
    pub async fn delete_container(&self, container_name: &str) -> Result<()> {
        delete_container(&self.client, container_name, true).await
    }

    /// Deletes a container by name with configurable force flag.
    pub async fn delete_container_opts(&self, container_name: &str, force: bool) -> Result<()> {
        delete_container(&self.client, container_name, force).await
    }

    /// Inspects a container to retrieve low-level runtime state.
    pub async fn inspect_container(&self, container_name: &str) -> Result<ContainerInspectResponse> {
        inspect_container(&self.client, container_name).await
    }

    /// Queries real-time container performance telemetry.
    pub async fn get_container_metrics(&self, container_name: &str) -> Result<ContainerMetrics> {
        get_container_stats(&self.client, container_name).await
    }

    /// Alias for get_container_metrics.
    pub async fn get_container_stats(&self, container_name: &str) -> Result<ContainerMetrics> {
        get_container_stats(&self.client, container_name).await
    }
}

/// Helper to create a Bollard DeviceMapping struct with full read-write-mknod permissions.
pub fn create_device_mapping(host_path: &str, container_path: &str) -> DeviceMapping {
    DeviceMapping {
        path_on_host: Some(host_path.to_string()),
        path_in_container: Some(container_path.to_string()),
        cgroup_permissions: Some("rwm".to_string()),
    }
}

/// Resolves the deterministic host binder device nodes to container device mappings against a custom root path.
///
/// Formula:
/// b1 = 3 * index + 1 -> /dev/binder
/// b2 = 3 * index + 2 -> /dev/hwbinder
/// b3 = 3 * index + 3 -> /dev/vndbinder
///
/// Fallback Hierarchy:
/// 1. Numbered /dev/binder{b1,b2,b3} nodes.
/// 2. Numbered /dev/binderfs/binder{b1,b2,b3} nodes.
/// 3. Default base fallback:
///    - If index == 0 and standard /dev/binder, /dev/hwbinder, /dev/vndbinder exist.
///    - If index == 0 and /dev/binderfs/{binder,hwbinder,vndbinder} exist.
///    - If /dev/binderfs directory exists, pass through /dev/binderfs.
///    - If only /dev/binder exists, map /dev/binder to /dev/binder.
pub fn resolve_binder_mappings_with_root(binder_index: u32, root: &Path) -> Vec<DeviceMapping> {
    let b1 = binder_index * 3 + 1;
    let b2 = binder_index * 3 + 2;
    let b3 = binder_index * 3 + 3;

    let p_b1_rel = format!("dev/binder{}", b1);
    let p_b2_rel = format!("dev/hwbinder{}", b2);
    let p_b3_rel = format!("dev/vndbinder{}", b3);

    let fs_b1_rel = format!("dev/binderfs/binder{}", b1);
    let fs_b2_rel = format!("dev/binderfs/binder{}", b2);
    let fs_b3_rel = format!("dev/binderfs/binder{}", b3);

    let p_b1 = root.join(&p_b1_rel);
    let p_b2 = root.join(&p_b2_rel);
    let p_b3 = root.join(&p_b3_rel);

    let fs_b1 = root.join(&fs_b1_rel);
    let fs_b2 = root.join(&fs_b2_rel);
    let fs_b3 = root.join(&fs_b3_rel);

    let target_b1 = format!("/dev/binder{}", b1);
    let target_b2 = format!("/dev/hwbinder{}", b2);
    let target_b3 = format!("/dev/vndbinder{}", b3);

    let target_fs_b1 = format!("/dev/binderfs/binder{}", b1);
    let target_fs_b2 = format!("/dev/binderfs/binder{}", b2);
    let target_fs_b3 = format!("/dev/binderfs/binder{}", b3);

    if p_b1.exists() && p_b2.exists() && p_b3.exists() {
        debug!(
            "Using numbered host binder nodes: {}, {}, {}",
            target_b1, target_b2, target_b3
        );
        vec![
            create_device_mapping(&target_b1, "/dev/binder"),
            create_device_mapping(&target_b2, "/dev/hwbinder"),
            create_device_mapping(&target_b3, "/dev/vndbinder"),
        ]
    } else if fs_b1.exists() && fs_b2.exists() && fs_b3.exists() {
        debug!(
            "Using numbered binderfs nodes: {}, {}, {}",
            target_fs_b1, target_fs_b2, target_fs_b3
        );
        vec![
            create_device_mapping(&target_fs_b1, "/dev/binder"),
            create_device_mapping(&target_fs_b2, "/dev/hwbinder"),
            create_device_mapping(&target_fs_b3, "/dev/vndbinder"),
        ]
    } else if binder_index == 0
        && root.join("dev/binder").exists()
        && root.join("dev/hwbinder").exists()
        && root.join("dev/vndbinder").exists()
    {
        debug!("Index 0 base fallback: using standard /dev/binder nodes");
        vec![
            create_device_mapping("/dev/binder", "/dev/binder"),
            create_device_mapping("/dev/hwbinder", "/dev/hwbinder"),
            create_device_mapping("/dev/vndbinder", "/dev/vndbinder"),
        ]
    } else if binder_index == 0
        && root.join("dev/binderfs/binder").exists()
        && root.join("dev/binderfs/hwbinder").exists()
        && root.join("dev/binderfs/vndbinder").exists()
    {
        debug!("Index 0 base fallback: using /dev/binderfs standard nodes");
        vec![
            create_device_mapping("/dev/binderfs/binder", "/dev/binder"),
            create_device_mapping("/dev/binderfs/hwbinder", "/dev/hwbinder"),
            create_device_mapping("/dev/binderfs/vndbinder", "/dev/vndbinder"),
        ]
    } else if root.join("dev/binderfs").exists() {
        debug!("Binderfs directory fallback: passing /dev/binderfs");
        vec![create_device_mapping("/dev/binderfs", "/dev/binderfs")]
    } else if root.join("dev/binder").exists() {
        warn!("Single binder fallback: only /dev/binder is present on host");
        vec![create_device_mapping("/dev/binder", "/dev/binder")]
    } else {
        warn!("No binder device nodes detected on host; container may fail to initialize IPC");
        Vec::new()
    }
}

/// Resolves the deterministic host binder device nodes to container device mappings using system root.
pub fn resolve_binder_mappings(binder_index: u32) -> Vec<DeviceMapping> {
    resolve_binder_mappings_with_root(binder_index, Path::new("/"))
}

/// Constructs the Bollard HostConfig according to CosmicReDroid specifications with configurable host root.
pub fn build_container_host_config_with_root(
    instance: &AndroidInstance,
    root: &Path,
) -> HostConfig {
    let mut devices = resolve_binder_mappings_with_root(instance.binder_index, root);

    let render_node = "/dev/dri/renderD128";
    if root.join("dev/dri/renderD128").exists() {
        debug!("DRI GPU render node detected, passing through {}", render_node);
        devices.push(create_device_mapping(render_node, render_node));
    } else {
        debug!("No DRI GPU render node detected at {}", render_node);
    }

    let mut port_bindings: PortMap = HashMap::new();
    port_bindings.insert(
        "5555/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some(instance.adb_port.to_string()),
        }]),
    );

    let binds = vec![format!("{}:/data", instance.data_path.display())];
    let shm_size = 2 * 1024 * 1024 * 1024; // 2 GB

    HostConfig {
        privileged: Some(true),
        shm_size: Some(shm_size),
        binds: Some(binds),
        devices: Some(devices),
        port_bindings: Some(port_bindings),
        ..Default::default()
    }
}

/// Constructs the Bollard HostConfig according to CosmicReDroid specifications.
pub fn build_container_host_config(instance: &AndroidInstance) -> HostConfig {
    build_container_host_config_with_root(instance, Path::new("/"))
}

/// Builds ReDroid command-line boot arguments for an AndroidInstance.
pub fn build_container_boot_args(instance: &AndroidInstance, gpu_available: bool) -> Vec<String> {
    let gpu_mode = if gpu_available { "host" } else { "guest" };
    vec![
        "androidboot.hardware=redroid".to_string(),
        "ro.secure=0".to_string(),
        "ro.boot.hwcomposer=ranchu".to_string(),
        format!("redroid.width={}", instance.identity.width),
        format!("redroid.height={}", instance.identity.height),
        format!("redroid.dpi={}", instance.identity.dpi),
        format!("redroid.fps={}", instance.identity.fps),
        format!("redroid.gpu.mode={}", gpu_mode),
        format!("ro.product.model={}", instance.identity.model),
        format!("ro.product.brand={}", instance.identity.brand),
        format!("ro.product.manufacturer={}", instance.identity.manufacturer),
        format!("ro.serialno={}", instance.identity.serial_no),
        format!("ro.boot.serialno={}", instance.identity.serial_no),
    ]
}

/// Alias for build_container_boot_args.
pub fn build_redroid_boot_args(instance: &AndroidInstance, gpu_available: bool) -> Vec<String> {
    build_container_boot_args(instance, gpu_available)
}

/// Constructs the complete Bollard container Config for container creation with configurable host root.
pub fn build_container_config_with_root(
    instance: &AndroidInstance,
    root: &Path,
) -> Config<String> {
    let gpu_available = root.join("dev/dri/renderD128").exists();
    let boot_args = build_container_boot_args(instance, gpu_available);
    let host_config = build_container_host_config_with_root(instance, root);

    let mut exposed_ports: HashMap<String, HashMap<(), ()>> = HashMap::new();
    exposed_ports.insert("5555/tcp".to_string(), HashMap::new());

    let mut labels = HashMap::new();
    labels.insert("app".to_string(), "cosmic-redroid".to_string());
    labels.insert("instance_id".to_string(), instance.id.to_string());
    labels.insert("instance_name".to_string(), instance.name.clone());

    Config {
        image: Some(instance.image_tag.clone()),
        exposed_ports: Some(exposed_ports),
        cmd: Some(boot_args),
        host_config: Some(host_config),
        labels: Some(labels),
        ..Default::default()
    }
}

/// Constructs the complete Bollard container Config for container creation.
pub fn build_container_config(instance: &AndroidInstance) -> Config<String> {
    build_container_config_with_root(instance, Path::new("/"))
}

/// Pulls a Docker image using Bollard and drives the progress stream until completion.
pub async fn pull_image(docker: &Docker, image: &str) -> Result<()> {
    info!("Pulling Docker image: {}", image);
    let options = Some(CreateImageOptions {
        from_image: image.to_string(),
        ..Default::default()
    });

    let mut stream = docker.create_image(options, None, None);
    while let Some(result) = stream.next().await {
        let info = result.with_context(|| format!("Error while pulling Docker image '{image}'"))?;
        if let Some(err) = info.error {
            return Err(anyhow::anyhow!("Docker pull failed for '{image}': {err}"));
        }
    }
    info!("Successfully pulled Docker image: {}", image);
    Ok(())
}

/// Creates a container for the given Android instance.
pub async fn create_container(docker: &Docker, instance: &AndroidInstance) -> Result<String> {
    info!(
        "Creating container '{}' from image '{}'",
        instance.container_name, instance.image_tag
    );
    let options = Some(CreateContainerOptions {
        name: instance.container_name.clone(),
        platform: None,
    });
    let config = build_container_config(instance);

    let response = docker
        .create_container(options, config)
        .await
        .with_context(|| {
            format!(
                "Failed to create Docker container '{}'",
                instance.container_name
            )
        })?;

    info!("Container created successfully with ID: {}", response.id);
    Ok(response.id)
}

/// Alias for create_container matching PROJECT.md interface contract.
pub async fn create_instance_container(
    docker: &Docker,
    instance: &AndroidInstance,
) -> Result<String> {
    create_container(docker, instance).await
}

/// Starts an existing container.
pub async fn start_container(docker: &Docker, container_name: &str) -> Result<()> {
    info!("Starting container '{}'", container_name);
    docker
        .start_container(container_name, None::<StartContainerOptions<String>>)
        .await
        .with_context(|| format!("Failed to start container '{container_name}'"))?;
    info!("Container '{}' started successfully", container_name);
    Ok(())
}

/// Stops a running container with a specified timeout (default 10s).
pub async fn stop_container(
    docker: &Docker,
    container_name: &str,
    timeout_secs: Option<i64>,
) -> Result<()> {
    let t = timeout_secs.unwrap_or(10);
    info!("Stopping container '{}' with timeout {}s", container_name, t);
    docker
        .stop_container(container_name, Some(StopContainerOptions { t }))
        .await
        .with_context(|| format!("Failed to stop container '{container_name}'"))?;
    info!("Container '{}' stopped successfully", container_name);
    Ok(())
}

/// Forcefully removes a container.
pub async fn delete_container(docker: &Docker, container_name: &str, force: bool) -> Result<()> {
    info!("Deleting container '{}' (force={})", container_name, force);
    let options = Some(RemoveContainerOptions {
        force,
        v: false,
        link: false,
    });
    docker
        .remove_container(container_name, options)
        .await
        .with_context(|| format!("Failed to remove container '{container_name}'"))?;
    info!("Container '{}' deleted successfully", container_name);
    Ok(())
}

/// Inspects a container and returns detailed inspect data.
pub async fn inspect_container(
    docker: &Docker,
    container_name: &str,
) -> Result<ContainerInspectResponse> {
    docker
        .inspect_container(container_name, None::<InspectContainerOptions>)
        .await
        .with_context(|| format!("Failed to inspect container '{container_name}'"))
}

/// Computes telemetry metrics from a Bollard Stats object.
pub fn calculate_metrics_from_stats(stats: &Stats) -> ContainerMetrics {
    let cpu_delta = stats
        .cpu_stats
        .cpu_usage
        .total_usage
        .saturating_sub(stats.precpu_stats.cpu_usage.total_usage) as f32;

    let system_delta = match (
        stats.cpu_stats.system_cpu_usage,
        stats.precpu_stats.system_cpu_usage,
    ) {
        (Some(curr), Some(prev)) => curr.saturating_sub(prev) as f32,
        _ => 0.0,
    };

    let online_cpus = stats.cpu_stats.online_cpus.unwrap_or_else(|| {
        stats
            .cpu_stats
            .cpu_usage
            .percpu_usage
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(1)
    }) as f32;

    let cpu_percent = if system_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / system_delta) * online_cpus * 100.0
    } else {
        0.0
    };

    let memory_used_mb = stats.memory_stats.usage.unwrap_or(0) / (1024 * 1024);
    let memory_limit_mb = stats.memory_stats.limit.unwrap_or(0) / (1024 * 1024);

    ContainerMetrics {
        cpu_percent,
        memory_used_mb,
        memory_limit_mb,
    }
}

/// Queries one-shot container statistics and computes real-time telemetry.
pub async fn get_container_stats(docker: &Docker, container_name: &str) -> Result<ContainerMetrics> {
    let options = Some(StatsOptions {
        stream: false,
        one_shot: true,
    });

    let mut stream = docker.stats(container_name, options);
    match stream.next().await {
        Some(Ok(stats)) => Ok(calculate_metrics_from_stats(&stats)),
        Some(Err(e)) => Err(anyhow::anyhow!(
            "Error fetching stats for container '{container_name}': {e}"
        )),
        None => Err(anyhow::anyhow!(
            "Stats stream ended unexpectedly for container '{container_name}'"
        )),
    }
}

/// Alias for get_container_stats.
pub async fn get_container_metrics(docker: &Docker, container_name: &str) -> Result<ContainerMetrics> {
    get_container_stats(docker, container_name).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AndroidVersion, DevicePreset, HardwareIdentity, InstanceStatus, ProxyConfig};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn mock_instance(binder_index: u32, adb_port: u16) -> AndroidInstance {
        AndroidInstance {
            id: Uuid::new_v4(),
            name: format!("Test Instance {}", binder_index),
            container_name: format!("cosmic-test-{}", binder_index),
            android_version: AndroidVersion::Android13,
            image_tag: "redroid/redroid:13.0.0-latest".to_string(),
            preset: DevicePreset::GalaxyS24Ultra,
            identity: HardwareIdentity::galaxy_s24_ultra(),
            proxy: ProxyConfig::default(),
            adb_port,
            data_path: PathBuf::from(format!("/tmp/cosmic_test_{}", binder_index)),
            binder_index,
            status: InstanceStatus::Stopped,
        }
    }

    fn create_mock_stats(
        total_usage: u64,
        prev_total_usage: u64,
        system_usage: Option<u64>,
        prev_system_usage: Option<u64>,
        online_cpus: Option<u64>,
        percpu_count: usize,
        mem_usage: Option<u64>,
        mem_limit: Option<u64>,
    ) -> Stats {
        let percpu: Vec<u64> = vec![0; percpu_count];
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
                    "total_usage": total_usage,
                    "percpu_usage": percpu,
                    "usage_in_kernelmode": 0,
                    "usage_in_usermode": 0
                },
                "system_cpu_usage": system_usage,
                "online_cpus": online_cpus,
                "throttling_data": {
                    "periods": 0,
                    "throttled_periods": 0,
                    "throttled_time": 0
                }
            },
            "precpu_stats": {
                "cpu_usage": {
                    "total_usage": prev_total_usage,
                    "usage_in_kernelmode": 0,
                    "usage_in_usermode": 0
                },
                "system_cpu_usage": prev_system_usage,
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
        serde_json::from_value(json).expect("valid Stats JSON deserialization")
    }

    #[test]
    fn test_deterministic_binder_formula_values() {
        let i0 = mock_instance(0, 5555);
        assert_eq!(i0.binder_indices(), (1, 2, 3));

        let i1 = mock_instance(1, 5557);
        assert_eq!(i1.binder_indices(), (4, 5, 6));

        let i2 = mock_instance(2, 5559);
        assert_eq!(i2.binder_indices(), (7, 8, 9));

        let i10 = mock_instance(10, 5575);
        assert_eq!(i10.binder_indices(), (31, 32, 33));

        let i100 = mock_instance(100, 5755);
        assert_eq!(i100.binder_indices(), (301, 302, 303));
    }

    #[test]
    fn test_resolve_binder_mappings_fallback_hierarchy() {
        let mappings = resolve_binder_mappings(0);
        for mapping in &mappings {
            assert_eq!(mapping.cgroup_permissions, Some("rwm".to_string()));
            assert!(mapping.path_on_host.is_some());
            assert!(mapping.path_in_container.is_some());
        }
    }

    struct TempTestDir {
        path: PathBuf,
    }

    impl TempTestDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4()));
            let _ = fs::create_dir_all(&path);
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_resolve_binder_mappings_mock_numbered_nodes() {
        let temp_dir = TempTestDir::new("test_numbered_nodes");
        let dev = temp_dir.path().join("dev");
        fs::create_dir_all(&dev).expect("mkdir dev");

        fs::write(dev.join("binder4"), b"").expect("write binder4");
        fs::write(dev.join("hwbinder5"), b"").expect("write hwbinder5");
        fs::write(dev.join("vndbinder6"), b"").expect("write vndbinder6");

        let mappings = resolve_binder_mappings_with_root(1, temp_dir.path());
        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0].path_on_host, Some("/dev/binder4".to_string()));
        assert_eq!(mappings[0].path_in_container, Some("/dev/binder".to_string()));
        assert_eq!(mappings[1].path_on_host, Some("/dev/hwbinder5".to_string()));
        assert_eq!(mappings[1].path_in_container, Some("/dev/hwbinder".to_string()));
        assert_eq!(mappings[2].path_on_host, Some("/dev/vndbinder6".to_string()));
        assert_eq!(mappings[2].path_in_container, Some("/dev/vndbinder".to_string()));
    }

    #[test]
    fn test_resolve_binder_mappings_mock_binderfs_numbered() {
        let temp_dir = TempTestDir::new("test_binderfs_numbered");
        let binderfs = temp_dir.path().join("dev/binderfs");
        fs::create_dir_all(&binderfs).expect("mkdir binderfs");

        fs::write(binderfs.join("binder7"), b"").expect("write binder7");
        fs::write(binderfs.join("binder8"), b"").expect("write binder8");
        fs::write(binderfs.join("binder9"), b"").expect("write binder9");

        let mappings = resolve_binder_mappings_with_root(2, temp_dir.path());
        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0].path_on_host, Some("/dev/binderfs/binder7".to_string()));
        assert_eq!(mappings[0].path_in_container, Some("/dev/binder".to_string()));
    }

    #[test]
    fn test_resolve_binder_mappings_mock_index_zero_fallback() {
        let temp_dir = TempTestDir::new("test_index_zero");
        let dev = temp_dir.path().join("dev");
        fs::create_dir_all(&dev).expect("mkdir dev");

        fs::write(dev.join("binder"), b"").expect("write binder");
        fs::write(dev.join("hwbinder"), b"").expect("write hwbinder");
        fs::write(dev.join("vndbinder"), b"").expect("write vndbinder");

        let mappings = resolve_binder_mappings_with_root(0, temp_dir.path());
        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0].path_on_host, Some("/dev/binder".to_string()));
        assert_eq!(mappings[1].path_on_host, Some("/dev/hwbinder".to_string()));
        assert_eq!(mappings[2].path_on_host, Some("/dev/vndbinder".to_string()));
    }

    #[test]
    fn test_resolve_binder_mappings_mock_binderfs_directory_fallback() {
        let temp_dir = TempTestDir::new("test_binderfs_dir");
        let binderfs = temp_dir.path().join("dev/binderfs");
        fs::create_dir_all(&binderfs).expect("mkdir binderfs");

        let mappings = resolve_binder_mappings_with_root(5, temp_dir.path());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].path_on_host, Some("/dev/binderfs".to_string()));
        assert_eq!(mappings[0].path_in_container, Some("/dev/binderfs".to_string()));
    }

    #[test]
    fn test_resolve_binder_mappings_mock_single_binder_fallback() {
        let temp_dir = TempTestDir::new("test_single_binder");
        let dev = temp_dir.path().join("dev");
        fs::create_dir_all(&dev).expect("mkdir dev");
        fs::write(dev.join("binder"), b"").expect("write binder");

        let mappings = resolve_binder_mappings_with_root(5, temp_dir.path());
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].path_on_host, Some("/dev/binder".to_string()));
        assert_eq!(mappings[0].path_in_container, Some("/dev/binder".to_string()));
    }

    #[test]
    fn test_resolve_binder_mappings_mock_empty_fallback() {
        let temp_dir = TempTestDir::new("test_empty");
        let mappings = resolve_binder_mappings_with_root(5, temp_dir.path());
        assert!(mappings.is_empty());
    }

    #[test]
    fn test_build_container_host_config_structure() {
        let instance = mock_instance(0, 5555);
        let host_config = build_container_host_config(&instance);

        assert_eq!(host_config.privileged, Some(true));
        assert_eq!(host_config.shm_size, Some(2 * 1024 * 1024 * 1024));

        let binds = host_config.binds.expect("binds should be set");
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0], format!("{}:/data", instance.data_path.display()));

        let port_bindings = host_config.port_bindings.expect("port bindings should be set");
        let binding = port_bindings.get("5555/tcp").expect("port 5555/tcp must be bound");
        let list = binding.as_ref().expect("binding list must exist");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].host_ip, Some("127.0.0.1".to_string()));
        assert_eq!(list[0].host_port, Some("5555".to_string()));
    }

    #[test]
    fn test_build_container_host_config_with_gpu_mock() {
        let temp_dir = TempTestDir::new("test_gpu_mock");
        let dri = temp_dir.path().join("dev/dri");
        fs::create_dir_all(&dri).expect("mkdir dri");
        fs::write(dri.join("renderD128"), b"").expect("write renderD128");

        let instance = mock_instance(0, 5555);
        let host_config = build_container_host_config_with_root(&instance, temp_dir.path());
        let devices = host_config.devices.expect("devices must be present");
        assert!(devices
            .iter()
            .any(|d| d.path_on_host == Some("/dev/dri/renderD128".to_string())));
    }

    #[test]
    fn test_build_container_config_structure() {
        let instance = mock_instance(1, 5557);
        let config = build_container_config(&instance);

        assert_eq!(config.image, Some(instance.image_tag.clone()));

        let exposed_ports = config.exposed_ports.expect("exposed ports set");
        assert!(exposed_ports.contains_key("5555/tcp"));

        let cmd = config.cmd.expect("cmd boot args set");
        assert!(cmd.iter().any(|arg| arg == "androidboot.hardware=redroid"));
        assert!(cmd.iter().any(|arg| arg.starts_with("redroid.width=")));
        assert!(cmd.iter().any(|arg| arg.starts_with("redroid.fps=")));

        let labels = config.labels.expect("labels set");
        assert_eq!(labels.get("app"), Some(&"cosmic-redroid".to_string()));
        assert_eq!(labels.get("instance_id"), Some(&instance.id.to_string()));
        assert_eq!(labels.get("instance_name"), Some(&instance.name));
    }

    #[test]
    fn test_telemetry_cpu_ram_calculation_normal() {
        let stats = create_mock_stats(
            1_500_000_000,
            1_000_000_000,
            Some(12_000_000_000),
            Some(10_000_000_000),
            Some(4),
            4,
            Some(2 * 1024 * 1024 * 1024),
            Some(8 * 1024 * 1024 * 1024),
        );

        let metrics = calculate_metrics_from_stats(&stats);

        // delta_cpu = 500M, delta_sys = 2000M, ratio = 0.25 * 4 * 100 = 100%
        assert!((metrics.cpu_percent - 100.0).abs() < 0.01);
        assert_eq!(metrics.memory_used_mb, 2048);
        assert_eq!(metrics.memory_limit_mb, 8192);
        assert_eq!(metrics.memory_percent(), 25.0);
    }

    #[test]
    fn test_telemetry_zero_system_delta_no_division_by_zero() {
        let stats = create_mock_stats(
            1000,
            1000,
            Some(5000),
            Some(5000),
            Some(4),
            4,
            None,
            None,
        );

        let metrics = calculate_metrics_from_stats(&stats);
        assert_eq!(metrics.cpu_percent, 0.0);
        assert_eq!(metrics.memory_used_mb, 0);
        assert_eq!(metrics.memory_limit_mb, 0);
    }

    #[test]
    fn test_telemetry_fallback_to_percpu_usage_when_online_cpus_none() {
        let stats = create_mock_stats(
            2_000_000_000,
            1_000_000_000,
            Some(20_000_000_000),
            Some(10_000_000_000),
            None,
            2,
            Some(512 * 1024 * 1024),
            Some(1024 * 1024 * 1024),
        );

        let metrics = calculate_metrics_from_stats(&stats);
        // delta_cpu = 1000M, delta_sys = 10000M, ratio = 0.1 * 2 cpus * 100 = 20%
        assert!((metrics.cpu_percent - 20.0).abs() < 0.01);
        assert_eq!(metrics.memory_used_mb, 512);
        assert_eq!(metrics.memory_limit_mb, 1024);
    }

    #[test]
    fn test_telemetry_saturating_counter_decrement() {
        // Test counter wrap or reset in cgroups
        let stats = create_mock_stats(
            500,
            1000,
            Some(2000),
            Some(1000),
            Some(1),
            1,
            Some(100),
            Some(200),
        );

        let metrics = calculate_metrics_from_stats(&stats);
        assert_eq!(metrics.cpu_percent, 0.0);
    }
}
