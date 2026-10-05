//! Background telemetry sampler.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use pulsebench_types::{ResourceSummary, TelemetrySample};
use sysinfo::System;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::gpu::query_nvidia;

struct Inner {
    sys: Mutex<System>,
    samples: Mutex<Vec<TelemetrySample>>,
    tx: broadcast::Sender<TelemetrySample>,
}

/// Samples CPU, RAM and (when `nvidia-smi` exists) GPU metrics at a fixed interval.
#[derive(Clone)]
pub struct Sampler {
    inner: Arc<Inner>,
    cancel: CancellationToken,
}

impl Sampler {
    pub fn start(interval: Duration) -> Sampler {
        let (tx, _) = broadcast::channel(256);
        let mut sys = System::new();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let inner = Arc::new(Inner { sys: Mutex::new(sys), samples: Mutex::new(Vec::new()), tx });
        let cancel = CancellationToken::new();
        let sampler = Sampler { inner, cancel: cancel.clone() };
        let me = sampler.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tick.tick() => { me.sample_now().await; }
                }
            }
        });
        sampler
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TelemetrySample> {
        self.inner.tx.subscribe()
    }

    pub fn latest(&self) -> Option<TelemetrySample> {
        self.inner.samples.lock().unwrap().last().cloned()
    }

    /// Take a sample immediately (used at task boundaries so short tasks always have data).
    pub async fn sample_now(&self) -> TelemetrySample {
        let (cpu, used, total) = {
            let mut sys = self.inner.sys.lock().unwrap();
            sys.refresh_cpu_usage();
            sys.refresh_memory();
            (sys.global_cpu_usage(), sys.used_memory(), sys.total_memory())
        };
        let gpu = query_nvidia().await;
        let (gpu_percent, vram_used, vram_total, temp) = match gpu {
            Some(readings) => {
                // Multiple GPUs: sum memory, report the busiest/hottest.
                let sum = |f: fn(&crate::gpu::NvidiaReading) -> Option<u64>| {
                    let vals: Vec<u64> = readings.iter().filter_map(f).collect();
                    (!vals.is_empty()).then(|| vals.iter().sum::<u64>())
                };
                let maxf = |f: fn(&crate::gpu::NvidiaReading) -> Option<f32>| {
                    readings.iter().filter_map(f).fold(None, |a: Option<f32>, v| Some(a.map_or(v, |x| x.max(v))))
                };
                (maxf(|r| r.util_percent), sum(|r| r.vram_used_mb), sum(|r| r.vram_total_mb), maxf(|r| r.temp_c))
            }
            None => (None, None, None, None),
        };
        let sample = TelemetrySample {
            ts: Utc::now(),
            cpu_percent: Some(cpu),
            ram_used_mb: used / (1024 * 1024),
            ram_total_mb: total / (1024 * 1024),
            gpu_percent,
            vram_used_mb: vram_used,
            vram_total_mb: vram_total,
            gpu_temp_c: temp,
        };
        self.inner.samples.lock().unwrap().push(sample.clone());
        let _ = self.inner.tx.send(sample.clone());
        sample
    }

    pub fn samples_between(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<TelemetrySample> {
        self.inner.samples.lock().unwrap().iter().filter(|s| s.ts >= from && s.ts <= to).cloned().collect()
    }

    pub fn summarize(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> ResourceSummary {
        summarize(&self.samples_between(from, to))
    }
}

pub fn summarize(samples: &[TelemetrySample]) -> ResourceSummary {
    let max_f = |f: &dyn Fn(&TelemetrySample) -> Option<f32>| {
        samples.iter().filter_map(f).fold(None, |a: Option<f32>, v| Some(a.map_or(v, |x| x.max(v))))
    };
    let cpu: Vec<f32> = samples.iter().filter_map(|s| s.cpu_percent).collect();
    ResourceSummary {
        samples: samples.len() as u32,
        peak_cpu_percent: max_f(&|s| s.cpu_percent),
        avg_cpu_percent: (!cpu.is_empty()).then(|| cpu.iter().sum::<f32>() / cpu.len() as f32),
        peak_ram_mb: samples.iter().map(|s| s.ram_used_mb).max(),
        peak_gpu_percent: max_f(&|s| s.gpu_percent),
        peak_vram_mb: samples.iter().filter_map(|s| s.vram_used_mb).max(),
        peak_gpu_temp_c: max_f(&|s| s.gpu_temp_c),
    }
}

/// Merge several summaries (e.g. per-task into per-model), taking peaks.
pub fn merge_summaries(parts: &[ResourceSummary]) -> ResourceSummary {
    let max_f = |f: &dyn Fn(&ResourceSummary) -> Option<f32>| {
        parts.iter().filter_map(f).fold(None, |a: Option<f32>, v| Some(a.map_or(v, |x| x.max(v))))
    };
    let total: u32 = parts.iter().map(|p| p.samples).sum();
    let weighted: f32 = parts.iter().filter_map(|p| p.avg_cpu_percent.map(|a| a * p.samples as f32)).sum();
    let weight: u32 = parts.iter().filter(|p| p.avg_cpu_percent.is_some()).map(|p| p.samples).sum();
    ResourceSummary {
        samples: total,
        peak_cpu_percent: max_f(&|p| p.peak_cpu_percent),
        avg_cpu_percent: (weight > 0).then(|| weighted / weight as f32),
        peak_ram_mb: parts.iter().filter_map(|p| p.peak_ram_mb).max(),
        peak_gpu_percent: max_f(&|p| p.peak_gpu_percent),
        peak_vram_mb: parts.iter().filter_map(|p| p.peak_vram_mb).max(),
        peak_gpu_temp_c: max_f(&|p| p.peak_gpu_temp_c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cpu: f32, ram: u64, vram: Option<u64>) -> TelemetrySample {
        TelemetrySample {
            ts: Utc::now(),
            cpu_percent: Some(cpu),
            ram_used_mb: ram,
            ram_total_mb: 32000,
            gpu_percent: vram.map(|_| 50.0),
            vram_used_mb: vram,
            vram_total_mb: vram.map(|_| 8192),
            gpu_temp_c: None,
        }
    }

    #[test]
    fn summary_takes_peaks_and_leaves_missing_gpu_as_none() {
        let s = summarize(&[sample(10.0, 1000, None), sample(30.0, 1500, None)]);
        assert_eq!(s.peak_cpu_percent, Some(30.0));
        assert_eq!(s.avg_cpu_percent, Some(20.0));
        assert_eq!(s.peak_ram_mb, Some(1500));
        assert_eq!(s.peak_vram_mb, None);
        assert_eq!(s.peak_gpu_percent, None);
    }

    #[test]
    fn summary_of_nothing_is_empty_not_zero() {
        let s = summarize(&[]);
        assert_eq!(s.samples, 0);
        assert_eq!(s.peak_cpu_percent, None);
        assert_eq!(s.peak_ram_mb, None);
    }

    #[test]
    fn merge_weights_average_by_samples() {
        let a = summarize(&[sample(10.0, 1, Some(100))]);
        let b = summarize(&[sample(20.0, 2, Some(300)), sample(40.0, 2, Some(200))]);
        let m = merge_summaries(&[a, b]);
        assert_eq!(m.samples, 3);
        assert_eq!(m.peak_vram_mb, Some(300));
        assert!((m.avg_cpu_percent.unwrap() - 70.0 / 3.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn sampler_collects_real_samples() {
        let s = Sampler::start(Duration::from_millis(100));
        tokio::time::sleep(Duration::from_millis(450)).await;
        let snap = s.sample_now().await;
        s.stop();
        assert!(snap.ram_total_mb > 0);
        assert!(s.samples_between(Utc::now() - chrono::Duration::seconds(5), Utc::now()).len() >= 3);
    }
}
