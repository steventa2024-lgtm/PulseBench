//! Static hardware/OS description.

use pulsebench_sandbox::runtime::{detect_docker, detect_runtimes};
use pulsebench_types::SystemInfo;
use sha2::{Digest, Sha256};
use sysinfo::System;

use crate::gpu::detect_gpus;

pub async fn detect_system() -> SystemInfo {
    // sysinfo does blocking work; keep it off the async threads.
    let base = tokio::task::spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let cpu_model =
            sys.cpus().first().map(|c| c.brand().trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Unknown CPU".into());
        let host = System::host_name().unwrap_or_default();
        let hash: String = Sha256::digest(host.as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect();
        (
            System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
            System::os_version().or_else(System::long_os_version).unwrap_or_default(),
            cpu_model,
            System::physical_core_count().map(|c| c as u32),
            sys.cpus().len() as u32,
            sys.total_memory() / (1024 * 1024),
            sys.available_memory() / (1024 * 1024),
            hash,
        )
    })
    .await
    .expect("system detection task");
    let (os_name, os_version, cpu_model, physical, logical, ram_total_mb, ram_available_mb, hostname_hash) = base;

    let (gpus, docker, runtimes) = tokio::join!(detect_gpus(), detect_docker(), detect_runtimes());
    SystemInfo {
        os_name,
        os_version,
        arch: std::env::consts::ARCH.to_string(),
        hostname_hash,
        cpu_model,
        cpu_physical_cores: physical,
        cpu_logical_cores: logical,
        ram_total_mb,
        ram_available_mb,
        gpus,
        docker,
        runtimes,
    }
}

/// Short label for the title bar, such as `NVIDIA GeForce RTX 3070 Ti`.
pub fn primary_gpu_label(info: &SystemInfo) -> Option<String> {
    info.gpus.first().map(|g| g.name.clone())
}
