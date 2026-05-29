// @anchor exchange:router:reconnect
// @tags api

use std::time::Duration;
use tokio::sync::watch;

/// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s (capped).
pub fn reconnect_delay(attempt: u32) -> Duration {
    let capped = attempt.min(5);
    Duration::from_secs(1u64 << capped)
}

/// Sleep for `delay`, returning `true` if stop signal received.
pub async fn wait_or_cancel(delay: Duration, stop_rx: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = stop_rx.changed() => changed.is_ok() && *stop_rx.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_exponential_backoff() {
        assert_eq!(reconnect_delay(0), Duration::from_secs(1));
        assert_eq!(reconnect_delay(1), Duration::from_secs(2));
        assert_eq!(reconnect_delay(2), Duration::from_secs(4));
        assert_eq!(reconnect_delay(3), Duration::from_secs(8));
        assert_eq!(reconnect_delay(4), Duration::from_secs(16));
        assert_eq!(reconnect_delay(5), Duration::from_secs(32));
        // Capped at 32s
        assert_eq!(reconnect_delay(6), Duration::from_secs(32));
        assert_eq!(reconnect_delay(100), Duration::from_secs(32));
    }

    #[tokio::test]
    async fn wait_or_cancel_respects_stop_signal() {
        let (stop_tx, mut stop_rx) = watch::channel(false);

        // Send stop signal immediately
        stop_tx.send(true).unwrap();

        let cancelled = wait_or_cancel(Duration::from_secs(60), &mut stop_rx).await;
        assert!(cancelled);
    }

    #[tokio::test]
    async fn wait_or_cancel_completes_on_timeout() {
        let (_stop_tx, mut stop_rx) = watch::channel(false);

        let cancelled = wait_or_cancel(Duration::from_millis(10), &mut stop_rx).await;
        assert!(!cancelled);
    }
}
