use std::{
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub const ENGINE: &str = "/System/Library/CoreServices/ScreenSaverEngine.app";

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

pub fn idle_time() -> Option<Duration> {
    // Combined login session, any input event. Only elapsed time is read.
    let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
    Duration::try_from_secs_f64(seconds).ok()
}

// Bound OS command execution so a stuck system service cannot freeze the UI.
fn status(program: &str, args: &[&str]) -> Result<i32, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let began = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(exit)) => return Ok(exit.code().unwrap_or(-1)),
            Ok(None) if began.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(25))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(e) => e.to_string(),
                    _ => "macOS 응답 시간 초과 (5초)".into(),
                });
            }
        }
    }
}

pub fn launch(dry_run: bool) -> Result<String, String> {
    if dry_run {
        return Ok("테스트 모드: 실행 요청을 기록했습니다. 화면 보호기를 켜지 않았습니다.".into());
    }
    if !Path::new(ENGINE).is_dir() {
        return Err("이 macOS에서 ScreenSaverEngine을 찾지 못했습니다.".into());
    }
    match status("/usr/bin/open", &["-a", ENGINE])? {
        0 => Ok("macOS가 실행 요청을 수락했습니다. 실제 화면 표시는 직접 확인해 주세요.".into()),
        code => Err(format!("화면 보호기 실행 요청 실패 (종료 코드 {code}). 시스템 설정에서 화면 보호기를 확인해 주세요.")),
    }
}

pub fn detect() -> String {
    // getuid has no preconditions. Limit observation to the current login user.
    let uid = unsafe { libc::getuid() }.to_string();
    match status("/usr/bin/pgrep", &["-u", &uid, "-x", "ScreenSaverEngine"]) {
        Ok(0) => "엔진 프로세스 감지됨 · 화면 표시 여부는 확인 불가".into(),
        Ok(1) => "엔진 미감지 · 화면 보호기 꺼짐을 보장하지 않음".into(),
        Ok(code) => format!("상태 확인 불가 · 조회 종료 코드 {code}"),
        Err(e) => format!("상태 확인 불가 · {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn safe_launch_never_invokes_engine() {
        assert!(launch(true).unwrap().contains("테스트 모드"));
    }
    #[test]
    fn command_failure_is_reported() {
        assert!(status("/nonexistent/screensaver-command", &[]).is_err());
    }
    #[test]
    fn exit_code_is_preserved() {
        assert_eq!(status("/usr/bin/false", &[]).unwrap(), 1);
    }
}
