use serde::{Deserialize, Serialize};

use crate::RuntimeFingerprint;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkMode {
    Headless,
    Rendered,
    Presented,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkRunner {
    LoomPlan,
    DirectMetalEncoding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub mode: BenchmarkMode,
    pub runner: BenchmarkRunner,
    pub warmup_ticks: u32,
    pub sample_ticks: u32,
    pub warmup_seconds: Option<u32>,
    pub sample_seconds: Option<u32>,
    pub pacing_hz: Option<u32>,
    pub pacing_lead_microseconds: u32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            mode: BenchmarkMode::Headless,
            runner: BenchmarkRunner::LoomPlan,
            warmup_ticks: 60,
            sample_ticks: 240,
            warmup_seconds: None,
            sample_seconds: None,
            pacing_hz: None,
            pacing_lead_microseconds: 0,
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
    pub runner: BenchmarkRunner,
    pub viewport: Option<ViewportSize>,
    pub warmup_ticks: u32,
    pub sample_ticks: u32,
    pub requested_warmup_seconds: Option<u32>,
    pub requested_sample_seconds: Option<u32>,
    pub gpu_ms: TimingSummary,
    pub cpu_orchestration_ms: TimingSummary,
    pub end_to_end_tick_ms: TimingSummary,
    pub sample_wall_time_seconds: f64,
    pub submitted_ticks_per_second: f64,
    pub gpu_p95_below_8_33_ms: bool,
    pub synchronized_each_tick: bool,
    pub max_in_flight_command_buffers: u32,
    pub pacing: Option<PacingResult>,
    pub presentation: Option<PresentationResult>,
    pub resources: ResourceMetrics,
    pub runtime: RuntimeFingerprint,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresentationResult {
    pub display_link_driven: bool,
    pub presented_frames: u32,
    pub drawable_starvation_events: u32,
    pub gpu_deadline_misses: u32,
    pub presentation_deadline_misses: u32,
    pub skipped_presentations: u32,
    pub lateness_tolerance_ms: f64,
    pub render_gpu_ms: TimingSummary,
    pub render_cpu_orchestration_ms: TimingSummary,
    pub render_end_to_end_ms: TimingSummary,
    pub display_target_lead_ms: TimingSummary,
    pub presentation_lateness_ms: TimingSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PacingResult {
    pub target_hz: u32,
    pub tick_budget_ms: f64,
    pub submission_lead_ms: f64,
    pub deadline_misses: u32,
    pub deadline_miss_rate: f64,
    pub maximum_lateness_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub gpu_stream_buffer_bytes: u64,
    pub gpu_value_buffer_bytes: u64,
    pub gpu_indirect_buffer_bytes: u64,
    pub initialization_application_blits: u64,
    pub steady_state_application_copies_per_tick: u64,
    pub steady_state_application_blits_per_tick: u64,
    pub steady_state_heap_allocations_per_tick: Option<u64>,
    pub peak_resident_set_bytes: Option<u64>,
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
    pub p99: f64,
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
        p99: percentile(&ordered, 0.99),
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
        assert_eq!(result.p99, 5.0);
        assert_eq!(result.maximum, 5.0);
        assert_eq!(result.mean, 3.0);
    }
}
