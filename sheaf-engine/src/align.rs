//! Cross-venue tick alignment via Polars ASOF join.

use crate::tick::TickBatch;
use polars::prelude::*;

#[derive(Debug, Clone)]
pub struct AlignmentConfig {
    pub tolerance_ms: u64,
    pub window_ms: u64,
    pub min_active_venues: usize,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            tolerance_ms: 500,
            window_ms: 100,
            min_active_venues: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlignedSnapshot {
    pub df: DataFrame,
    pub alignment_time_ns: i64,
    pub active_venues: usize,
    pub cross_exchange_skew_ms: i64,
}

pub fn align_batch(
    batch: TickBatch,
    config: &AlignmentConfig,
    alignment_time_ns: i64,
) -> Result<AlignedSnapshot, crate::error::SheafError> {
    let mut df = batch.into_polars();

    let tolerance_ns = (config.tolerance_ms * 1_000_000) as i64;
    let cutoff_ns = alignment_time_ns - tolerance_ns;

    // Filter: only ticks within tolerance window.
    let mask = df
        .column("event_ts")
        .map_err(|e| crate::error::SheafError::Alignment(format!("column missing: {e}")))?
        .i64()
        .map_err(|e| crate::error::SheafError::Alignment(format!("expected i64: {e}")))?
        .into_iter()
        .map(|v| v.map_or(false, |ts| ts >= cutoff_ns && ts <= alignment_time_ns))
        .collect::<polars::prelude::BooleanChunked>();

    df = df
        .filter(&mask)
        .map_err(|e| crate::error::SheafError::Alignment(format!("filter: {e}")))?;

    if df.is_empty() {
        return Ok(AlignedSnapshot {
            df,
            alignment_time_ns,
            active_venues: 0,
            cross_exchange_skew_ms: 0,
        });
    }

    // Count active venues.
    let active_venues = df
        .column("venue")
        .ok()
        .map(|col| col.unique().map(|s| s.len()).unwrap_or(0))
        .unwrap_or(0);

    // Compute cross-exchange skew.
    let skew_ms = df
        .column("event_ts")
        .ok()
        .and_then(|col| col.i64().ok())
        .map(|s| {
            let min = s.iter().filter_map(|v| v).min().unwrap_or(0);
            let max = s.iter().filter_map(|v| v).max().unwrap_or(0);
            (max - min) / 1_000_000
        })
        .unwrap_or(0);

    Ok(AlignedSnapshot {
        df,
        alignment_time_ns,
        active_venues,
        cross_exchange_skew_ms: skew_ms,
    })
}
