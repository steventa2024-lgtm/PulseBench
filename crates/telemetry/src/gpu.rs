//! GPU discovery and live sampling. NVIDIA is sampled through `nvidia-smi`; other vendors are
//! named via OS tools but have no live metrics (reported as unavailable, never guessed).

use std::path::Path;
use std::time::Duration;

use pulsebench_sandbox::runtime::capture;
use pulsebench_types::{GpuInfo, GpuVendor};

/// One `nvidia-smi` reading for one GPU.
#[derive(Debug, Clone, PartialEq)]
pub struct NvidiaReading {
    pub name: String,
    pub vram_total_mb: Option<u64>,
    pub vram_used_mb: Option<u64>,
    pub util_percent: Option<f32>,
    pub temp_c: Option<f32>,
    pub driver: Option<String>,
}

pub const NVIDIA_QUERY: &str = "name,memory.total,memory.used,utilization.gpu,temperature.gpu,driver_version";

fn num<T: std::str::FromStr>(s: &str) -> Option<T> {
    let s = s.trim();
    if s.is_empty() || s.starts_with('[') || s.eq_ignore_ascii_case("n/a") {
        return None;
    }
    s.parse().ok()
}

/// Parse `nvidia-smi --query-gpu=... --format=csv,noheader,nounits`.
pub fn parse_nvidia_csv(text: &str) -> Vec<NvidiaReading> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 6 {
                return None;
            }
            Some(NvidiaReading {
                name: f[0].to_string(),
                vram_total_mb: num(f[1]),
                vram_used_mb: num(f[2]),
                util_percent: num(f[3]),
                temp_c: num(f[4]),
                driver: Some(f[5].to_string()).filter(|d| !d.is_empty() && !d.starts_with('[')),
            })
        })
        .collect()
}

fn nvidia_smi_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = which::which("nvidia-smi") {
        return Some(p);
    }
    // Windows installs it outside PATH on some driver versions.
    let candidates = [r"C:\Windows\System32\nvidia-smi.exe", r"C:\Program Files\NVIDIA Corporation\NVSMI\nvidia-smi.exe"];
    candidates.iter().map(Path::new).find(|p| p.exists()).map(|p| p.to_path_buf())
}

pub async fn query_nvidia() -> Option<Vec<NvidiaReading>> {
    let exe = nvidia_smi_path()?;
    let out = capture(&exe, &[format!("--query-gpu={NVIDIA_QUERY}"), "--format=csv,noheader,nounits".to_string()], Duration::from_secs(5))
        .await?;
    let readings = parse_nvidia_csv(&out);
    (!readings.is_empty()).then_some(readings)
}

pub fn vendor_from_name(name: &str) -> GpuVendor {
    let n = name.to_ascii_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("rtx") || n.contains("quadro") || n.contains("tesla") {
        GpuVendor::Nvidia
    } else if n.contains("amd") || n.contains("radeon") || n.contains("advanced micro") {
        GpuVendor::Amd
    } else if n.contains("intel") {
        GpuVendor::Intel
    } else if n.contains("apple") {
        GpuVendor::Apple
    } else {
        GpuVendor::Other
    }
}

/// Parse `Get-CimInstance Win32_VideoController | ConvertTo-Json` output (object or array).
pub fn parse_windows_video_controllers(json: &str) -> Vec<(String, Option<String>)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    let items = match v {
        serde_json::Value::Array(a) => a,
        o @ serde_json::Value::Object(_) => vec![o],
        _ => vec![],
    };
    items
        .iter()
        .filter_map(|i| {
            let name = i.get("Name")?.as_str()?.trim().to_string();
            let driver = i.get("DriverVersion").and_then(|d| d.as_str()).map(str::to_string);
            // Skip remote-display and virtual adapters.
            let lower = name.to_ascii_lowercase();
            (!lower.contains("basic display") && !lower.contains("remote") && !lower.contains("virtual")).then_some((name, driver))
        })
        .collect()
}

/// Parse `lspci` lines for display controllers.
pub fn parse_lspci(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains("VGA compatible controller") || l.contains("3D controller") || l.contains("Display controller"))
        .filter_map(|l| l.split_once(": ").map(|(_, rest)| rest.trim().to_string()))
        .collect()
}

/// Parse macOS `system_profiler SPDisplaysDataType -json`.
pub fn parse_macos_displays(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return vec![] };
    v.get("SPDisplaysDataType")
        .and_then(|d| d.as_array())
        .map(|a| a.iter().filter_map(|g| g.get("sppci_model").and_then(|m| m.as_str()).map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Enumerate GPUs. NVIDIA GPUs come with VRAM/driver data; others only a name.
pub async fn detect_gpus() -> Vec<GpuInfo> {
    if let Some(readings) = query_nvidia().await {
        return readings
            .into_iter()
            .map(|r| GpuInfo {
                name: r.name,
                vendor: GpuVendor::Nvidia,
                vram_total_mb: r.vram_total_mb,
                vram_used_mb: r.vram_used_mb,
                driver_version: r.driver,
                telemetry_available: true,
            })
            .collect();
    }
    let names: Vec<(String, Option<String>)> = if cfg!(windows) {
        match which::which("powershell") {
            Ok(ps) => capture(
                &ps,
                &[
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    "Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion | ConvertTo-Json -Compress".into(),
                ],
                Duration::from_secs(10),
            )
            .await
            .map(|o| parse_windows_video_controllers(&o))
            .unwrap_or_default(),
            Err(_) => vec![],
        }
    } else if cfg!(target_os = "macos") {
        match which::which("system_profiler") {
            Ok(sp) => capture(&sp, &["SPDisplaysDataType".into(), "-json".into()], Duration::from_secs(10))
                .await
                .map(|o| parse_macos_displays(&o).into_iter().map(|n| (n, None)).collect())
                .unwrap_or_default(),
            Err(_) => vec![],
        }
    } else {
        match which::which("lspci") {
            Ok(l) => capture(&l, &[], Duration::from_secs(5))
                .await
                .map(|o| parse_lspci(&o).into_iter().map(|n| (n, None)).collect())
                .unwrap_or_default(),
            Err(_) => vec![],
        }
    };
    names
        .into_iter()
        .map(|(name, driver)| GpuInfo {
            vendor: vendor_from_name(&name),
            name,
            vram_total_mb: None,
            vram_used_mb: None,
            driver_version: driver,
            telemetry_available: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_csv() {
        let out = "NVIDIA GeForce RTX 3070 Ti, 8192, 1043, 7, 41, 561.09\n";
        let r = parse_nvidia_csv(out);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "NVIDIA GeForce RTX 3070 Ti");
        assert_eq!(r[0].vram_total_mb, Some(8192));
        assert_eq!(r[0].vram_used_mb, Some(1043));
        assert_eq!(r[0].util_percent, Some(7.0));
        assert_eq!(r[0].temp_c, Some(41.0));
        assert_eq!(r[0].driver.as_deref(), Some("561.09"));
    }

    #[test]
    fn unsupported_fields_become_none() {
        let out = "NVIDIA RTX A2000, 6144, 100, [N/A], [Not Supported], 535.1\nbroken line\n";
        let r = parse_nvidia_csv(out);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].util_percent, None);
        assert_eq!(r[0].temp_c, None);
    }

    #[test]
    fn multi_gpu() {
        let out = "A, 8192, 1, 0, 30, 1\nB, 24576, 2, 3, 31, 1\n";
        assert_eq!(parse_nvidia_csv(out).len(), 2);
    }

    #[test]
    fn windows_controllers_object_and_array() {
        let one = r#"{"Name":"AMD Radeon RX 7800 XT","DriverVersion":"31.0.1"}"#;
        assert_eq!(parse_windows_video_controllers(one)[0].0, "AMD Radeon RX 7800 XT");
        let many = r#"[{"Name":"Microsoft Basic Display Adapter"},{"Name":"Intel(R) Arc(TM) A770","DriverVersion":"32.0"}]"#;
        let r = parse_windows_video_controllers(many);
        assert_eq!(r.len(), 1);
        assert_eq!(vendor_from_name(&r[0].0), GpuVendor::Intel);
    }

    #[test]
    fn lspci_and_macos() {
        let l = "00:02.0 VGA compatible controller: Intel Corporation UHD Graphics 630\n01:00.0 3D controller: NVIDIA Corporation GA104 [GeForce RTX 3070 Ti]\n00:1f.3 Audio device: x";
        let g = parse_lspci(l);
        assert_eq!(g.len(), 2);
        assert!(g[1].contains("RTX 3070 Ti"));
        let m = r#"{"SPDisplaysDataType":[{"sppci_model":"Apple M2 Pro"}]}"#;
        assert_eq!(parse_macos_displays(m), vec!["Apple M2 Pro".to_string()]);
    }
}
