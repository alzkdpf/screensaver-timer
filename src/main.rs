#![cfg(target_os = "macos")]
mod platform;
use eframe::egui::{self, Color32, FontData, FontFamily, RichText};
use screensaver_timer::{
    after_saver, clock_text, confirms_saver_start, whole_seconds, AfterSaver, Timer, DISPLAY_DELAY,
    SAVER_START_TIMEOUT,
};
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
    sleeps: u32,
    display_pending: bool,
    awaiting_saver: bool,
    busy: bool,
}
extern "C" {
    fn timer_menu_create(
        state: *const std::ffi::c_void,
        read_seconds: extern "C" fn(*const std::ffi::c_void) -> i64,
    ) -> *mut std::ffi::c_void;
    fn timer_menu_shows_countdown(handle: *const std::ffi::c_void) -> i32;
    fn timer_dock_badge_is_clear() -> i32;
    fn saver_observe(
        state: *const std::ffi::c_void,
        event: extern "C" fn(*const std::ffi::c_void, i32),
    ) -> *mut std::ffi::c_void;
    fn saver_unobserve(handle: *mut std::ffi::c_void);
    fn read_display_delay() -> i64;
    fn save_display_delay(seconds: i64);
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
    Start(u64, u64),
    Cancel,
    Now(u64),
    SaverStarted(Instant),
    SaverStopped,
}

struct SaverObserver {
    handle: *mut std::ffi::c_void,
    // The native observer uses this allocation until it is removed in Drop.
    _sender: Box<Sender<Action>>,
}
impl SaverObserver {
    fn new(tx: Sender<Action>) -> Self {
        let sender = Box::new(tx);
        let handle =
            unsafe { saver_observe((&*sender as *const Sender<Action>).cast(), saver_event) };
        Self {
            handle,
            _sender: sender,
        }
    }
}
impl Drop for SaverObserver {
    fn drop(&mut self) {
        unsafe { saver_unobserve(self.handle) };
    }
}
extern "C" fn saver_event(state: *const std::ffi::c_void, started: i32) {
    let tx = unsafe { &*state.cast::<Sender<Action>>() };
    let _ = tx.send(if started != 0 {
        Action::SaverStarted(Instant::now())
    } else {
        Action::SaverStopped
    });
}

struct App {
    hours: u64,
    display_delay: String,
    _saver_observer: SaverObserver,
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
    smoke_saw_countdown: bool,
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
            // egui keeps the text foreground color when drawing a selection.
            s.visuals.selection.bg_fill = Color32::from_rgb(175, 226, 207);
            s.visuals.selection.stroke.color = Color32::from_rgb(20, 87, 66);
        });
        let snapshot = Arc::new(Mutex::new(Snapshot {
            running: false,
            remaining: None,
            message: "입력이 없을 때 기다릴 시간을 설정하세요.".into(),
            error: false,
            launches: 0,
            sleeps: 0,
            display_pending: false,
            awaiting_saver: false,
            busy: false,
        }));
        let detection = Arc::new(Mutex::new("확인 중…".into()));
        let (tx, rx) = mpsc::channel();
        let state = snapshot.clone();
        thread::spawn(move || {
            let mut timer = Timer::default();
            let mut saver_started: Option<Instant> = None;
            let mut request_at: Option<Instant> = None;
            let mut sleep_requested = false;
            let mut display_delay = DISPLAY_DELAY;
            loop {
                let mut action = match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(action) => Some(action),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let now = Instant::now();
                // Safe tests exercise the same transition as the native start event.
                if dry_run
                    && action.is_none()
                    && request_at.is_some_and(|request| {
                        now.saturating_duration_since(request) >= Duration::from_millis(250)
                    })
                {
                    action = Some(Action::SaverStarted(now));
                }
                let idle = if smoke {
                    Some(Duration::MAX)
                } else if timer.is_running()
                    || saver_started.is_some()
                    || matches!(action, Some(Action::Start(..)))
                {
                    platform::idle_time()
                } else {
                    None
                };
                let mut launch = false;
                let mut sleep = false;
                {
                    let mut s = state.lock().unwrap();
                    match action {
                        Some(Action::Start(seconds, delay)) => match timer.start(now, seconds) {
                            Ok(()) => {
                                display_delay = Duration::from_secs(delay);
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
                            request_at = None;
                            saver_started = None;
                            s.message = "감시를 중지했습니다.".into();
                            s.error = false;
                        }
                        Some(Action::Now(delay)) => {
                            display_delay = Duration::from_secs(delay);
                            request_at = None;
                            saver_started = None;
                            launch = true;
                        }
                        Some(Action::SaverStarted(started))
                            if confirms_saver_start(request_at, started) =>
                        {
                            request_at = None;
                            saver_started = Some(started);
                            sleep_requested = false;
                            s.message =
                                "화면 보호기 시작 확인 · 화면 끄기까지 시간을 셉니다.".into();
                            s.error = false;
                        }
                        Some(Action::SaverStopped)
                            if request_at.is_some()
                                || (saver_started.is_some() && !sleep_requested) =>
                        {
                            request_at = None;
                            saver_started = None;
                            timer.reset_cycle(now);
                            s.message = "화면 보호기가 종료되었습니다.".into();
                            s.error = false;
                        }
                        _ => {}
                    }
                    if request_at.is_some_and(|requested| {
                        now.saturating_duration_since(requested) >= SAVER_START_TIMEOUT
                    }) {
                        request_at = None;
                        timer.reset_cycle(now);
                        s.message =
                            "화면 보호기 시작을 확인하지 못했습니다. 화면 끄기를 건너뜁니다."
                                .into();
                        s.error = true;
                    }
                    let mut display_left = None;
                    if let Some(started) = saver_started {
                        match after_saver(
                            now.saturating_duration_since(started),
                            idle,
                            sleep_requested,
                            display_delay,
                        ) {
                            AfterSaver::Cancelled => {
                                saver_started = None;
                                s.message = "입력이 감지되어 화면 끄기 예약을 종료했습니다.".into();
                                s.error = false;
                            }
                            AfterSaver::Waiting(duration) => display_left = duration,
                            AfterSaver::SleepDisplay => {
                                sleep_requested = true;
                                sleep = true;
                            }
                            AfterSaver::WaitForInput => {}
                        }
                    }
                    if request_at.is_none() && saver_started.is_none() && !launch {
                        launch = timer.take_due(now, idle);
                    }
                    s.awaiting_saver = request_at.is_some();
                    s.running = timer.is_running() || saver_started.is_some() || s.awaiting_saver;
                    s.display_pending = saver_started.is_some();
                    s.remaining = if s.awaiting_saver {
                        None
                    } else if s.display_pending {
                        display_left
                    } else {
                        timer.remaining(now, idle)
                    }
                    .map(whole_seconds);
                    if launch {
                        s.busy = true;
                        s.message = "macOS에 실행 요청 중…".into();
                    }
                }
                if launch {
                    let requested = Instant::now();
                    let result = platform::launch(dry_run);
                    let now = Instant::now();
                    timer.reset_cycle(now);
                    let mut s = state.lock().unwrap();
                    s.error = result.is_err();
                    s.message = match result {
                        Ok(_) => {
                            request_at = Some(requested);
                            s.awaiting_saver = true;
                            s.running = true;
                            s.remaining = None;
                            "화면 보호기 시작을 기다리는 중…".into()
                        }
                        Err(e) => e,
                    };
                    s.launches += 1;
                    s.busy = false;
                } else if sleep {
                    let result = platform::sleep_display(dry_run);
                    let mut s = state.lock().unwrap();
                    s.error = result.is_err();
                    s.message = match result {
                        Ok(()) if dry_run => "안전 모드 · 화면 끄기 요청을 기록했습니다.".into(),
                        Ok(()) => "디스플레이 잠자기를 요청했습니다.".into(),
                        Err(e) => e,
                    };
                    s.sleeps += 1;
                }
            }
        });
        let detected = detection.clone();
        thread::spawn(move || loop {
            *detected.lock().unwrap() = platform::detect();
            thread::sleep(Duration::from_secs(2));
        });
        if smoke {
            tx.send(Action::Start(1, 1)).unwrap();
        }
        let menu =
            MenuBar(unsafe { timer_menu_create(Arc::as_ptr(&snapshot).cast(), menu_seconds) });
        Self {
            _menu: menu,
            _saver_observer: SaverObserver::new(tx.clone()),
            display_delay: match unsafe { read_display_delay() } {
                seconds @ 1..=3600 => seconds as u64,
                _ => DISPLAY_DELAY.as_secs(),
            }
            .to_string(),
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
            smoke_saw_countdown: false,
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
        let display_delay = self
            .display_delay
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|seconds| (1..=3600).contains(seconds));
        let left = if running {
            state
                .remaining
                .map(clock_text)
                .unwrap_or_else(|| "—".into())
        } else {
            clock_text(total)
        };
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(247, 247, 243)).inner_margin(28.)).show(ctx, |ui| {
            ui.heading(RichText::new("화면 보호기 타이머").size(26.));
            ui.label("화면 보호기가 시작된 뒤 디스플레이를 끕니다.");
            ui.add_space(10.);
            ui.vertical_centered(|ui| {
                ui.label(if state.awaiting_saver { "화면 보호기 시작 확인 중" } else if state.display_pending { "화면 끄기까지" } else if running { "다음 실행까지" } else { "대기 시간" });
                ui.label(RichText::new(left).monospace().size(54.));
            });
            ui.separator();
            ui.label(RichText::new("1. 화면 보호기 시작").size(17.).strong());
            ui.label("입력이 없을 때 기다릴 시간");
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
            ui.separator();
            ui.label(RichText::new("2. 디스플레이 끄기").size(17.).strong());
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 36.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label("화면 보호기 시작 후");
                    ui.add_enabled_ui(!running && !state.busy, |ui| {
                        let field = ui.add_sized(
                            [74., 36.],
                            egui::TextEdit::singleline(&mut self.display_delay)
                                .font(egui::TextStyle::Monospace)
                                .horizontal_align(egui::Align::Center).vertical_align(egui::Align::Center),
                        );
                        field.widget_info(|| egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit, true, "화면 보호기 시작 후 대기 시간(초)"));
                        if field.changed() && !self.dry_run {
                            if let Some(seconds) = self.display_delay.trim().parse::<u64>()
                                .ok().filter(|seconds| (1..=3600).contains(seconds)) {
                                unsafe { save_display_delay(seconds as i64) };
                            }
                        }
                    });
                    ui.label("초");
                },
            );
            if display_delay.is_none() {
                ui.colored_label(Color32::DARK_RED, "1~3600 사이의 초를 입력하세요.");
            }
            ui.add_space(8.);
            ui.horizontal(|ui| {
                if ui.add_enabled(!running && !state.busy && display_delay.is_some() && (1..=86400).contains(&total),
                    egui::Button::new(RichText::new("감시 시작").color(Color32::WHITE)).fill(Color32::from_rgb(29, 104, 78))).clicked() { if let Some(seconds) = display_delay { self.send(Action::Start(total, seconds)); } }
                if ui.add_enabled(running, egui::Button::new("취소")).clicked() { self.send(Action::Cancel); }
                if ui.add_enabled(!state.busy && display_delay.is_some(), egui::Button::new("지금 실행")).clicked() { if let Some(seconds) = display_delay { self.send(Action::Now(seconds)); } }
            });
            if !(1..=86400).contains(&total) { ui.colored_label(Color32::DARK_RED, "1초부터 24시간까지 설정하세요."); }
            ui.add_space(4.);
            if running && !state.awaiting_saver && !state.display_pending && state.remaining.is_none() { ui.label("입력 상태 확인 불가 · 실행 대기 중"); }
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
            self.smoke_saw_countdown |= unsafe { timer_menu_shows_countdown(self._menu.0) != 0 };
            if let Some((cancelled, count)) = self.smoke_cancel_at {
                if cancelled.elapsed() > Duration::from_millis(1600) {
                    if state.launches == count
                        && !state.running
                        && !state.busy
                        && unsafe { timer_dock_badge_is_clear() != 0 }
                    {
                        self.smoke = false;
                        println!("SMOKE_OK: GUI and Dock verified; one simulated saver and display sleep; cancel cleared Dock badge");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        eprintln!("SMOKE_FAILED: repeats continued after cancel");
                        std::process::exit(1);
                    }
                }
            } else if state.launches == 1
                && state.sleeps == 1
                && state.running
                && !state.busy
                && !state.error
                && self.smoke_saw_countdown
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
            .with_inner_size([540., 730.])
            .with_min_inner_size([540., 730.])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "Screen Saver Timer",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, dry_run, smoke)))),
    )
}
