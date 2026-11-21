use std::process::{Command, Stdio};

#[derive(Default)]
pub struct SystemTimings {
    pub screen_saver_seconds: Option<u64>,
    pub display_sleep: Vec<DisplaySleep>,
}

pub struct DisplaySleep {
    pub power_source: &'static str,
    pub minutes: Option<u64>,
}

fn read_command(path: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(path)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn parse_power_profiles(output: &str) -> Vec<DisplaySleep> {
    let mut profiles: Vec<DisplaySleep> = Vec::new();
    let mut profile = None;
    for line in output.lines() {
        let line = line.trim();
        if line.ends_with(':') {
            profile = match line {
                "Battery Power:" => Some("배터리"),
                "AC Power:" => Some("전원 어댑터"),
                "UPS Power:" => Some("UPS"),
                _ => None,
            }
            .map(|power_source| {
                profiles.push(DisplaySleep {
                    power_source,
                    minutes: None,
                });
                profiles.len() - 1
            });
        } else if let Some(index) = profile {
            let mut columns = line.split_whitespace();
            if columns.next() == Some("displaysleep") {
                profiles[index].minutes = columns.next().and_then(|value| value.parse().ok());
            }
        }
    }
    profiles
}

impl SystemTimings {
    pub fn read() -> Self {
        let screen_saver_seconds = read_command(
            "/usr/bin/defaults",
            &["-currentHost", "read", "com.apple.screensaver", "idleTime"],
        )
        .and_then(|value| value.trim().parse().ok());
        let display_sleep = read_command("/usr/bin/pmset", &["-g", "custom"])
            .map_or_else(Vec::new, |value| parse_power_profiles(&value));
        Self {
            screen_saver_seconds,
            display_sleep,
        }
    }
}

pub fn idle_duration(seconds: Option<u64>) -> String {
    match seconds {
        None => "확인할 수 없음".into(),
        Some(0) => "안 함".into(),
        Some(s) if s % 60 == 0 => format!("{}분 후", s / 60),
        Some(s) if s < 60 => format!("{s}초 후"),
        Some(s) => format!("{}분 {}초 후", s / 60, s % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_power_profiles_and_disabled_timer() {
        let profiles = parse_power_profiles(
            "Battery Power:\n sleep 1\n displaysleep 5\nAC Power:\n displaysleep 0\nUPS Power:\n disksleep 15\n",
        );
        let timings: Vec<_> = profiles
            .iter()
            .map(|profile| (profile.power_source, profile.minutes))
            .collect();
        assert_eq!(
            timings,
            [("배터리", Some(5)), ("전원 어댑터", Some(0)), ("UPS", None)]
        );
        assert_eq!(idle_duration(Some(0)), "안 함");
        assert_eq!(idle_duration(None), "확인할 수 없음");
    }

    #[test]
    fn does_not_assign_unknown_profiles_to_the_previous_power_source() {
        let profiles =
            parse_power_profiles("AC Power:\n displaysleep 10\nUnknown Power:\n displaysleep 99\n");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].minutes, Some(10));
    }
}
