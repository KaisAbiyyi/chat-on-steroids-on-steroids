//! PERF-04: bounded idle child-exit observation for owned processes and PTYs.
//! Never detach exit custody, ACK handling or output delivery.
use std::time::Duration;

pub(crate) struct IdlePoll {
    next_ms: u64,
}

impl IdlePoll {
    pub(crate) fn new() -> Self {
        Self { next_ms: 10 }
    }

    fn next_delay(&mut self) -> Duration {
        let wait = Duration::from_millis(self.next_ms);
        self.next_ms = self.next_ms.saturating_mul(2).min(80);
        wait
    }

    pub(crate) fn wait(&mut self) {
        std::thread::sleep(self.next_delay());
    }
}

#[cfg(test)]
mod tests {
    use super::IdlePoll;
    use std::time::Duration;

    #[test]
    fn idle_checks_back_off_and_never_exceed_an_eighty_ms_receipt_delay() {
        let mut poll = IdlePoll::new();
        let observed: Vec<_> = (0..10).map(|_| poll.next_delay()).collect();
        assert_eq!(observed[..4], [10, 20, 40, 80].map(Duration::from_millis));
        assert!(
            observed[4..]
                .iter()
                .all(|delay| *delay == Duration::from_millis(80))
        );
    }

    #[test]
    fn a_new_process_starts_with_low_latency() {
        let mut previous = IdlePoll::new();
        for _ in 0..8 {
            previous.next_delay();
        }
        let mut fresh = IdlePoll::new();
        assert_eq!(fresh.next_delay(), Duration::from_millis(10));
        assert_eq!(previous.next_delay(), Duration::from_millis(80));
    }
}
