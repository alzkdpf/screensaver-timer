#![cfg(target_os = "macos")]
mod platform;
use eframe::egui::{self, Color32, FontData, FontFamily, RichText};
use screensaver_timer::{clock_text, whole_seconds, Timer};
use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Snapshot {
    running: bool,
    remaining: Option<u64>,
    message: String,
    error: bool,
    launches: u32,
    busy: bool,
}
extern "C" {
    fn timer_menu_create(
        state: *const std::ffi::c_void,
        read_seconds: extern "C" fn(*const std::ffi::c_void) -> i64,
    ) -> *mut std::ffi::c_void;
    fn timer_menu_shows_countdown(handle: *const std::ffi::c_void) -> i32;
    fn timer_dock_badge_is_clear() -> i32;
    fn timer_menu_destroy(handle: *mut std::ffi::c_void);
}

struct MenuBar(*mut std::ffi::c_void);
impl Drop for MenuBar {
    fn drop(&mut self) {
        // AppKit invalidates the timer before the snapshot is released.
        unsafe { timer_menu_destroy(self.0) };
    }
}

extern "C" fn menu_seconds(state: *const std::ffi::c_void) -> i64 {
    // App owns the Arc for the entire lifetime of the main-thread AppKit timer.
    let snapshot = unsafe { &*state.cast::<Mutex<Snapshot>>() };
    match snapshot.lock() {
        Ok(s) if !s.running => -1,
        Ok(s) => s.remaining.map_or(-2, |seconds| seconds as i64),
        Err(_) => -2,
    }
}

enum Action {
    Start(u64),
    Cancel,
    Now,
}
struct App {
    hours: u64,
    minutes: u64,
    seconds: u64,
    tx: Sender<Action>,
    _menu: MenuBar,
    snapshot: Arc<Mutex<Snapshot>>,
    detection: Arc<Mutex<String>>,
    dry_run: bool,
    smoke: bool,
    began: Instant,
    smoke_cancel_at: Option<(Instant, u32)>,
}
impl App {
    fn new(cc: &eframe::CreationContext<'_>, dry_run: bool, smoke: bool) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("/System/Library/Fonts/AppleSDGothicNeo.ttc") {
            fonts
                .font_data
                .insert("Korean".into(), FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, "Korean".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        cc.egui_ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(10., 12.);
            s.spacing.button_padding = egui::vec2(18., 10.);
            s.visuals.selection.bg_fill = Color32::from_rgb(29, 104, 78);
        });
        let snapshot = Arc::new(Mutex::new(Snapshot {
            running: false,
            remaining: None,
            message: "입력이 없을 때 기다릴 시간을 설정하세요.".into(),
            error: false,
            launches: 0,
            busy: false,
        }));
        let detection = Arc::new(Mutex::new("확인 중…".into()));
        let (tx, rx) = mpsc::channel();
        let state = snapshot.clone();
        thread::spawn(move || {
            let mut timer = Timer::default();
            loop {
                let action = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(action) => Some(action),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let now = Instant::now();
                let idle = if smoke {
                    Some(Duration::MAX)
                } else if timer.is_running() || matches!(action, Some(Action::Start(_))) {
                    platform::idle_time()
                } else {
                    None
                };
                let mut launch = false;
                {
                    let mut s = state.lock().unwrap();
                    match action {
                        Some(Action::Start(seconds)) => match timer.start(now, seconds) {
                            Ok(()) => {
                                s.message =
                                    "감시 중 · 입력이 들어오면 남은 시간이 초기화됩니다.".into();
                                s.error = false;
                            }
                            Err(e) => {
                                s.message = e.into();
                                s.error = true;
                            }
                        },
                        Some(Action::Cancel) => {
                            timer.cancel();
                            s.message = "감시를 중지했습니다.".into();
                            s.error = false;
                        }
                        Some(Action::Now) => {
                            launch = true;
                        }
                        None => {}
                    }
                    launch |= timer.take_due(now, idle);
                    s.running = timer.is_running();
                    s.remaining = timer.remaining(now, idle).map(whole_seconds);
                    if launch {
                        s.busy = true;
                        s.message = "macOS에 실행 요청 중…".into();
                    }
                }
                if launch {
                    let result = platform::launch(dry_run);
                    let now = Instant::now();
                    timer.reset_cycle(now);
                    let mut s = state.lock().unwrap();
                    s.remaining = timer.remaining(now, idle).map(whole_seconds);
                    s.error = result.is_err();
                    s.message = result.unwrap_or_else(|e| e);
                    s.launches += 1;
                    s.busy = false;
                }
            }
        });
        let detected = detection.clone();
        thread::spawn(move || loop {
            *detected.lock().unwrap() = platform::detect();
            thread::sleep(Duration::from_secs(2));
        });
        if smoke {
            tx.send(Action::Start(1)).unwrap();
        }
        let menu =
            MenuBar(unsafe { timer_menu_create(Arc::as_ptr(&snapshot).cast(), menu_seconds) });
        Self {
            _menu: menu,
            hours: 0,
            minutes: if smoke { 0 } else { 5 },
            seconds: if smoke { 1 } else { 0 },
            tx,
            snapshot,
            detection,
            dry_run,
            smoke,
            began: Instant::now(),
            smoke_cancel_at: None,
        }
    }
    fn send(&self, action: Action) {
        let _ = self.tx.send(action);
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.snapshot.lock().unwrap().clone();
        let running = state.running;
        let total = self.hours * 3600 + self.minutes * 60 + self.seconds;
        let left = if running {
            state
                .remaining
                .map(clock_text)
                .unwrap_or_else(|| "—".into())
        } else {
            clock_text(total)
        };
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(247, 247, 243)).inner_margin(28.)).show(ctx, |ui| {
            ui.label(RichText::new("SCREEN SAVER TIMER").size(12.).color(Color32::from_rgb(29, 104, 78)));
            ui.heading(RichText::new("잠깐, 화면도 쉬어가기").size(26.));
            ui.label("마우스나 키보드 입력이 없으면 화면 보호기를 실행합니다.");
            ui.add_space(10.);
            ui.vertical_centered(|ui| {
                ui.label(if running { "다음 실행까지" } else { "대기 시간" });
                ui.label(RichText::new(left).monospace().size(54.));
            });
            ui.label(format!("대기 시간: {}", clock_text(total)));
            ui.add_space(8.);
            ui.add_enabled_ui(!running && !state.busy, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut self.hours).range(0..=24).suffix(" 시간"));
                    ui.add(egui::DragValue::new(&mut self.minutes).range(0..=59).suffix(" 분"));
                    ui.add(egui::DragValue::new(&mut self.seconds).range(0..=59).suffix(" 초"));
                });
                ui.horizontal(|ui| { for minutes in [1, 5, 15, 30] {
                    if ui.button(format!("{minutes}분")).clicked() { self.hours = 0; self.minutes = minutes; self.seconds = 0; }
                }});
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!running && !state.busy && (1..=86400).contains(&total),
                    egui::Button::new(RichText::new("감시 시작").color(Color32::WHITE)).fill(Color32::from_rgb(29, 104, 78))).clicked() { self.send(Action::Start(total)); }
                if ui.add_enabled(running, egui::Button::new("취소")).clicked() { self.send(Action::Cancel); }
                if ui.add_enabled(!state.busy, egui::Button::new("지금 실행")).clicked() { self.send(Action::Now); }
            });
            if !(1..=86400).contains(&total) { ui.colored_label(Color32::DARK_RED, "1초부터 24시간까지 설정하세요."); }
            ui.add_space(4.);
            if running && state.remaining.is_none() { ui.label("입력 상태 확인 불가 · 실행 대기 중"); }
            ui.label(RichText::new(&state.message).color(if state.error { Color32::DARK_RED } else { Color32::from_gray(55) }));
            ui.separator();
            ui.label(RichText::new("화면 보호기 상태").strong());
            ui.label(self.detection.lock().unwrap().clone());
            ui.label(RichText::new("2초마다 프로세스를 확인합니다. 화면 표시·잠금 상태와 다를 수 있습니다.").size(12.).color(Color32::from_gray(95)));
            ui.add_space(4.);
            ui.label(RichText::new("창을 최소화해도 Dock과 메뉴 막대에서 남은 시간을 볼 수 있습니다.\n화면 보호기에서 돌아와도 감시는 계속됩니다.").size(12.).color(Color32::from_gray(95)));
            if self.dry_run { ui.colored_label(Color32::from_rgb(160, 90, 0), "안전 테스트 모드 · 실제 화면 보호기를 실행하지 않습니다."); }
        });
        ctx.request_repaint_after(Duration::from_millis(200));
        if self.smoke {
            if let Some((cancelled, count)) = self.smoke_cancel_at {
                if cancelled.elapsed() > Duration::from_millis(1600) {
                    if state.launches == count
                        && !state.running
                        && !state.busy
                        && unsafe { timer_dock_badge_is_clear() != 0 }
                    {
                        self.smoke = false;
                        println!("SMOKE_OK: GUI, menu and Dock countdown verified; at least 3 idle launches; cancel stopped repeats and cleared Dock badge");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        eprintln!("SMOKE_FAILED: repeats continued after cancel");
                        std::process::exit(1);
                    }
                }
            } else if state.launches >= 3
                && state.running
                && !state.busy
                && !state.error
                && unsafe { timer_menu_shows_countdown(self._menu.0) != 0 }
            {
                self.send(Action::Cancel);
                self.smoke_cancel_at = Some((Instant::now(), state.launches));
            }
            if self.began.elapsed() > Duration::from_secs(15) {
                eprintln!("SMOKE_FAILED: {}", state.message);
                std::process::exit(1);
            }
        }
    }
}
fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--diagnose") {
        println!(
            "Engine installed: {}\n{}",
            std::path::Path::new(platform::ENGINE).exists(),
            platform::detect()
        );
        println!(
            "Idle seconds: {:?}",
            platform::idle_time().map(|d| d.as_secs_f64())
        );
        return Ok(());
    }
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let dry_run = smoke || args.iter().any(|a| a == "--dry-run");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([540., 620.])
            .with_min_inner_size([540., 620.])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Screen Saver Timer",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, dry_run, smoke)))),
    )
}
