use std::time::{Duration, SystemTime};

pub const MAX_SECONDS: u64 = 24 * 60 * 60;

#[derive(Default, Debug)]
pub struct Timer {
    deadline: Option<SystemTime>,
    interval: Duration,
}
impl Timer {
    pub fn start(&mut self, now: SystemTime, seconds: u64) -> Result<(), &'static str> {
        if !(1..=MAX_SECONDS).contains(&seconds) {
            return Err("시간은 1초부터 24시간까지 설정할 수 있습니다.");
        }
        self.interval = Duration::from_secs(seconds);
        self.deadline = now.checked_add(self.interval);
        self.deadline.ok_or("시간을 계산할 수 없습니다.")?;
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.deadline = None;
    }
    pub fn deadline(&self) -> Option<SystemTime> {
        self.deadline
    }
    // Drop intervals missed while an OS request was in progress, without firing again.
    pub fn skip_missed(&mut self, now: SystemTime) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = now.checked_add(self.interval);
        }
    }
    pub fn take_due(&mut self, now: SystemTime) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.deadline = now.checked_add(self.interval);
            true
        } else {
            false
        }
    }
}

pub fn remaining(deadline: SystemTime, now: SystemTime) -> u64 {
    let d = deadline.duration_since(now).unwrap_or_default();
    d.as_secs() + u64::from(d.subsec_nanos() > 0)
}
pub fn clock_text(seconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1000)
    }
    #[test]
    fn fires_once_at_deadline() {
        let mut t = Timer::default();
        t.start(now(), 5).unwrap();
        assert!(!t.take_due(now() + Duration::from_secs(4)));
        assert!(t.take_due(now() + Duration::from_secs(5)));
        assert!(!t.take_due(now() + Duration::from_secs(6)));
    }
    #[test]
    fn cancellation_prevents_launch() {
        let mut t = Timer::default();
        t.start(now(), 1).unwrap();
        t.cancel();
        assert!(!t.take_due(now() + Duration::from_secs(10)));
    }
    #[test]
    fn restart_replaces_deadline() {
        let mut t = Timer::default();
        t.start(now(), 1).unwrap();
        t.start(now(), 10).unwrap();
        assert!(!t.take_due(now() + Duration::from_secs(1)));
        assert!(t.take_due(now() + Duration::from_secs(10)));
    }
    #[test]
    fn validates_limits() {
        let mut t = Timer::default();
        assert!(t.start(now(), 0).is_err());
        assert!(t.start(now(), MAX_SECONDS + 1).is_err());
        assert!(t.start(now(), MAX_SECONDS).is_ok());
    }
    #[test]
    fn overdue_after_sleep_fires_once() {
        let mut t = Timer::default();
        t.start(now(), 60).unwrap();
        assert!(t.take_due(now() + Duration::from_secs(7200)));
        assert!(!t.take_due(now() + Duration::from_secs(7201)));
    }
    #[test]
    fn repeats_across_many_cycles_until_cancelled() {
        let mut t = Timer::default();
        t.start(now(), 5).unwrap();
        for cycle in 1..=4 {
            let due = now() + Duration::from_secs(cycle * 5);
            assert!(!t.take_due(due - Duration::from_millis(1)));
            assert!(t.take_due(due));
            assert!(!t.take_due(due));
            assert_eq!(t.deadline(), Some(due + Duration::from_secs(5)));
        }
        t.cancel();
        for elapsed in [25, 30, 1000] {
            assert!(!t.take_due(now() + Duration::from_secs(elapsed)));
        }
        assert_eq!(t.deadline(), None);
    }
    #[test]
    fn missed_cycles_do_not_burst_and_resume_from_wake_time() {
        let mut t = Timer::default();
        t.start(now(), 5).unwrap();
        let wake = now() + Duration::from_secs(103);
        assert!(t.take_due(wake));
        for _ in 0..20 {
            assert!(!t.take_due(wake));
        }
        assert_eq!(t.deadline(), Some(wake + Duration::from_secs(5)));
        assert!(t.take_due(wake + Duration::from_secs(5)));
    }
    #[test]
    fn slow_os_request_skips_overdue_cycles() {
        let mut t = Timer::default();
        t.start(now(), 1).unwrap();
        assert!(t.take_due(now() + Duration::from_secs(1)));
        t.skip_missed(now() + Duration::from_secs(7));
        assert!(!t.take_due(now() + Duration::from_secs(7)));
        assert!(t.take_due(now() + Duration::from_secs(8)));
        t.cancel();
        t.skip_missed(now() + Duration::from_secs(100));
        assert_eq!(t.deadline(), None);
    }
    #[test]
    fn rounds_up_and_clamps_expired_time() {
        let end = now() + Duration::from_millis(1001);
        assert_eq!(remaining(end, now()), 2);
        assert_eq!(remaining(end, end + Duration::from_secs(1)), 0);
        assert_eq!(clock_text(3661), "01:01:01");
    }
}
