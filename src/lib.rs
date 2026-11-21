use std::time::{Duration, Instant};

pub const MAX_SECONDS: u64 = 24 * 60 * 60;

#[derive(Default)]
pub struct Timer {
    cycle: Option<(Instant, Duration)>,
}

impl Timer {
    pub fn start(&mut self, now: Instant, seconds: u64) -> Result<(), &'static str> {
        if !(1..=MAX_SECONDS).contains(&seconds) {
            return Err("시간은 1초부터 24시간까지 설정할 수 있습니다.");
        }
        self.cycle = Some((now, Duration::from_secs(seconds)));
        Ok(())
    }

    pub fn cancel(&mut self) {
        self.cycle = None;
    }

    pub fn is_running(&self) -> bool {
        self.cycle.is_some()
    }

    pub fn reset_cycle(&mut self, now: Instant) {
        if let Some((start, _)) = &mut self.cycle {
            *start = now;
        }
    }

    pub fn remaining(&self, now: Instant, idle: Option<Duration>) -> Option<Duration> {
        let (start, interval) = self.cycle?;
        // Ignore any idle time accrued before arming or before the previous request completed.
        let elapsed = now.saturating_duration_since(start).min(idle?);
        Some(interval.saturating_sub(elapsed))
    }

    pub fn take_due(&mut self, now: Instant, idle: Option<Duration>) -> bool {
        if self.remaining(now, idle) == Some(Duration::ZERO) {
            self.reset_cycle(now);
            true
        } else {
            false
        }
    }
}

pub fn whole_seconds(duration: Duration) -> u64 {
    duration.as_secs() + u64::from(duration.subsec_nanos() > 0)
}

pub fn clock_text(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use Duration as D;

    #[test]
    fn input_restarts_countdown_and_prevents_launch_at_old_deadline() {
        let now = Instant::now();
        let mut t = Timer::default();
        t.start(now, 5).unwrap();
        assert_eq!(
            t.remaining(now + D::from_secs(3), Some(D::from_secs(3))),
            Some(D::from_secs(2))
        );
        let input = now + D::from_secs(5);
        assert!(!t.take_due(input, Some(D::ZERO)));
        assert_eq!(t.remaining(input, Some(D::ZERO)), Some(D::from_secs(5)));
        assert!(t.take_due(input + D::from_secs(5), Some(D::from_secs(5))));
    }

    #[test]
    fn arming_ignores_prior_idle_and_repeats_until_cancelled() {
        let now = Instant::now();
        let mut t = Timer::default();
        t.start(now, 5).unwrap();
        assert!(!t.take_due(now, Some(D::MAX)));
        for cycle in 1..=4 {
            let due = now + D::from_secs(cycle * 5);
            assert!(t.take_due(due, Some(D::MAX)));
            assert!(!t.take_due(due, Some(D::MAX)));
        }
        t.cancel();
        assert!(!t.take_due(now + D::from_secs(30), Some(D::MAX)));
        assert!(!t.is_running());
    }

    #[test]
    fn delayed_cycle_fires_once_and_waits_after_request() {
        let now = Instant::now();
        let mut t = Timer::default();
        t.start(now, 2).unwrap();
        let wake = now + D::from_secs(60);
        assert!(t.take_due(wake, Some(D::MAX)));
        let finished = wake + D::from_secs(4);
        t.reset_cycle(finished);
        assert!(!t.take_due(finished, Some(D::MAX)));
        assert!(t.take_due(finished + D::from_secs(2), Some(D::MAX)));
    }

    #[test]
    fn unavailable_idle_data_does_not_fire() {
        let now = Instant::now();
        let mut t = Timer::default();
        t.start(now, 1).unwrap();
        assert!(!t.take_due(now + D::from_secs(60), None));
        assert!(t.is_running());
    }

    #[test]
    fn validates_limits_and_rounds_up() {
        let now = Instant::now();
        let mut t = Timer::default();
        assert!(t.start(now, 0).is_err());
        assert!(t.start(now, MAX_SECONDS + 1).is_err());
        assert!(t.start(now, MAX_SECONDS).is_ok());
        assert_eq!(whole_seconds(D::from_millis(1001)), 2);
        assert_eq!(clock_text(3661), "01:01:01");
    }
}
