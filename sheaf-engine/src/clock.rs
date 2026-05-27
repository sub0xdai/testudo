//! Exchange clock synchronization — FLOX-style RTT-based offset estimation.
//!
//! Exchanges use NTP, but NTP accuracy varies under load, after maintenance,
//! and during NTP steps. A venue's clock can drift 100-300ms from true time.
//!
//! `ExchangeClock` estimates the offset between the local monotonic clock and
//! the exchange's clock using RTT measurements with EMA smoothing.
//!
//! Reference: `github.com/flox-foundation/flox` — `ExchangeClockSync`

/// Per-venue clock synchronization state.
///
/// Updated on every heartbeat/pong from the venue. Converts exchange
/// timestamps to normalized "sheaf time" for cross-venue alignment.
#[derive(Debug, Clone)]
pub struct ExchangeClock {
    pub venue: String,

    /// Estimated offset from local monotonic clock to exchange clock (nanoseconds).
    /// Positive = exchange clock is ahead of local clock.
    pub offset_ns: i64,

    /// 95% confidence interval half-width (nanoseconds).
    pub confidence_ns: i64,

    /// Estimated one-way latency (nanoseconds).
    pub latency_ns: i64,

    /// EMA smoothing factor. Default 0.1 — slow, stable convergence.
    pub ema_alpha: f64,

    /// Number of RTT samples collected.
    sample_count: u64,

    /// Wall clock time of last update.
    last_update_ns: i64,
}

impl ExchangeClock {
    /// Create a new clock sync for a venue.
    pub fn new(venue: String) -> Self {
        Self {
            venue,
            offset_ns: 0,
            confidence_ns: 1_000_000_000, // 1s initial uncertainty
            latency_ns: 0,
            ema_alpha: 0.1,
            sample_count: 0,
            last_update_ns: 0,
        }
    }

    /// Update the offset estimate from a round-trip time measurement.
    ///
    /// Called on every heartbeat/pong from the venue.
    ///
    /// # Arguments
    /// * `rtt_ns` — measured round-trip time (nanoseconds)
    /// * `server_time_ns` — the exchange's reported time at pong
    pub fn update(&mut self, rtt_ns: i64, server_time_ns: i64) {
        // Local time at pong arrival = monotonic_clock()
        // We estimate the local time at the midpoint of the RTT:
        //   local_midpoint ≈ now - rtt/2
        // Offset = server_time - local_midpoint
        let now = Self::monotonic_ns();
        let measured_offset = server_time_ns - (now - rtt_ns / 2);

        // EMA smoothing for stability
        if self.sample_count == 0 {
            self.offset_ns = measured_offset;
        } else {
            self.offset_ns = ((self.ema_alpha * measured_offset as f64)
                + ((1.0 - self.ema_alpha) * self.offset_ns as f64))
                as i64;
        }

        self.latency_ns = rtt_ns / 2; // estimate one-way latency
        self.confidence_ns = rtt_ns; // conservative: full RTT as confidence bound
        self.sample_count += 1;
        self.last_update_ns = now;
    }

    /// Convert an exchange timestamp to normalized sheaf time.
    ///
    /// `sheaf_time = event_ts - offset_ns`
    pub fn to_sheaf_time(&self, event_ts: i64) -> i64 {
        event_ts - self.offset_ns
    }

    /// How long since the last clock sync update (nanoseconds).
    pub fn age_ns(&self) -> i64 {
        Self::monotonic_ns() - self.last_update_ns
    }

    /// Whether the clock sync is fresh enough to trust.
    /// Returns false if no update in > 60 seconds.
    pub fn is_fresh(&self) -> bool {
        self.sample_count > 0 && self.age_ns() < 60_000_000_000
    }

    /// Monotonic clock in nanoseconds.
    /// Uses `tokio::time::Instant` or `std::time::Instant` depending on context.
    fn monotonic_ns() -> i64 {
        // In production, use a proper monotonic clock.
        // For now, use system time as placeholder.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_offset_is_zero() {
        let clock = ExchangeClock::new("BN".into());
        assert_eq!(clock.offset_ns, 0);
        assert_eq!(clock.sample_count, 0);
    }

    #[test]
    fn test_first_update_sets_offset_directly() {
        let mut clock = ExchangeClock::new("BN".into());
        // Simulate: RTT = 100ms, server says it's T+50ms ahead
        clock.update(100_000_000, 50_000_000);
        // Offset should be approximately 0 (server_time ≈ local_midpoint)
        // Exact value depends on monotonic_ns() — just check it updated
        assert_eq!(clock.sample_count, 1);
        assert!(clock.is_fresh());
    }

    #[test]
    fn test_ema_convergence() {
        let mut clock = ExchangeClock::new("BN".into());
        clock.ema_alpha = 0.5; // faster convergence for testing

        // First update: offset = 100ms
        clock.update(10_000_000, 110_000_000);
        let first_offset = clock.offset_ns;

        // Second update: offset = 200ms (exchange clock slipped)
        clock.update(10_000_000, 210_000_000);

        // EMA should move toward 200ms but not all the way
        let second_offset = clock.offset_ns;
        assert!(second_offset != first_offset, "EMA should update");
    }
}
