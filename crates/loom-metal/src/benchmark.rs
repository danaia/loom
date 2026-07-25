use serde::{Deserialize, Serialize};

use crate::RuntimeFingerprint;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkMode {
    Headless,
    Rendered,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub mode: BenchmarkMode,
    pub warmup_ticks: u32,
    pub sample_ticks: u32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            mode: BenchmarkMode::Headless,
            warmup_ticks: 60,
            sample_ticks: 240,
            viewport_width: 960,
            viewport_height: 720,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub experiment: String,
    pub particle_count: u32,
    pub mode: BenchmarkMode,
    pub viewport: Option<ViewportSize>,
    pub warmup_ticks: u32,
    pub sample_ticks: u32,
    pub gpu_ms: TimingSummary,
    pub cpu_orchestration_ms: TimingSummary,
    pub gpu_p95_below_8_33_ms: bool,
    pub synchronized_each_tick: bool,
    pub runtime: RuntimeFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimingSummary {
    pub minimum: f64,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub maximum: f64,
}

pub(crate) fn summarize(samples: &[f64]) -> TimingSummary {
    let mut ordered = samples.to_vec();
    ordered.sort_by(f64::total_cmp);
    let mean = ordered.iter().sum::<f64>() / ordered.len() as f64;
    TimingSummary {
        minimum: ordered[0],
        mean,
        p50: percentile(&ordered, 0.50),
        p95: percentile(&ordered, 0.95),
        maximum: ordered[ordered.len() - 1],
    }
}

fn percentile(ordered: &[f64], quantile: f64) -> f64 {
    let index = ((ordered.len() as f64 * quantile).ceil() as usize)
        .saturating_sub(1)
        .min(ordered.len() - 1);
    ordered[index]
}

#[cfg(test)]
mod tests {
    use super::summarize;

    #[test]
    fn timing_summary_is_deterministic() {
        let result = summarize(&[4.0, 1.0, 3.0, 2.0, 5.0]);
        assert_eq!(result.minimum, 1.0);
        assert_eq!(result.p50, 3.0);
        assert_eq!(result.p95, 5.0);
        assert_eq!(result.maximum, 5.0);
        assert_eq!(result.mean, 3.0);
    }
}
