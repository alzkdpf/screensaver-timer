#![cfg(target_os = "macos")]
mod platform;
use eframe::egui::{self, Color32, FontData, FontFamily, RichText};
use screensaver_timer::{clock_text, remaining, Timer};
use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone)]
struct Snapshot {
    deadline: Option<SystemTime>,
    message: String,
    error: bool,
    launches: u32,
    busy: bool,
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
            deadline: None,
            message: "반복 간격을 설정한 뒤 시작하세요.".into(),
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
                let mut launch = false;
                {
                    let mut s = state.lock().unwrap();
                    match action {
                        Some(Action::Start(seconds)) => {
                            match timer.start(SystemTime::now(), seconds) {
                                Ok(()) => {
                                    s.message =
                                        "반복 실행 중 · 취소하거나 앱을 종료할 때까지 계속됩니다."
                                            .into();
                                    s.error = false;
                                }
                                Err(e) => {
                                    s.message = e.into();
                                    s.error = true;
                                }
                            }
                        }
                        Some(Action::Cancel) => {
                            timer.cancel();
                            s.message = "반복 실행을 취소했습니다.".into();
                            s.error = false;
                        }
                        Some(Action::Now) => {
                            launch = true;
                        }
                        None => {}
                    }
                    launch |= timer.take_due(SystemTime::now());
                    s.deadline = timer.deadline();
                    if launch {
                        s.busy = true;
                        s.message = "macOS에 실행 요청 중…".into();
                    }
                }
                if launch {
                    let result = platform::launch(dry_run);
                    timer.skip_missed(SystemTime::now());
                    let mut s = state.lock().unwrap();
                    s.deadline = timer.deadline();
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
        Self {
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
        let running = state.deadline.is_some();
        let total = self.hours * 3600 + self.minutes * 60 + self.seconds;
        let left = state
            .deadline
            .map(|d| remaining(d, SystemTime::now()))
            .unwrap_or(total);
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::from_rgb(247, 247, 243)).inner_margin(28.)).show(ctx, |ui| {
            ui.label(RichText::new("SCREEN SAVER / INTERVAL").size(12.).color(Color32::from_rgb(29, 104, 78)));
            ui.heading(RichText::new("잠깐, 화면도 쉬어가기").size(26.));
            ui.label("설정한 간격마다 화면 보호기를 반복 실행합니다.");
            ui.add_space(10.);
            ui.vertical_centered(|ui| {
                ui.label(if running { "다음 실행까지" } else { "반복 간격" });
                ui.label(RichText::new(clock_text(left)).monospace().size(54.));
            });
            ui.label(format!("반복 간격: {}", clock_text(total)));
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
                    egui::Button::new(RichText::new("반복 시작").color(Color32::WHITE)).fill(Color32::from_rgb(29, 104, 78))).clicked() { self.send(Action::Start(total)); }
                if ui.add_enabled(running, egui::Button::new("취소")).clicked() { self.send(Action::Cancel); }
                if ui.add_enabled(!state.busy, egui::Button::new("지금 실행")).clicked() { self.send(Action::Now); }
            });
            if !(1..=86400).contains(&total) { ui.colored_label(Color32::DARK_RED, "1초부터 24시간까지 설정하세요."); }
            ui.add_space(4.);
            ui.label(RichText::new(&state.message).color(if state.error { Color32::DARK_RED } else { Color32::from_gray(55) }));
            ui.separator();
            ui.label(RichText::new("화면 보호기 상태").strong());
            ui.label(self.detection.lock().unwrap().clone());
            ui.label(RichText::new("2초마다 프로세스를 확인합니다. 화면 표시·잠금 상태와 다를 수 있습니다.").size(12.).color(Color32::from_gray(95)));
            ui.add_space(4.);
            ui.label(RichText::new("취소·앱 종료 시 반복 중단 · 복귀 후에도 예약 유지\n잠자기 중 놓친 주기는 한 번만 실행 · 지금 실행도 반복 유지").size(12.).color(Color32::from_gray(95)));
            if self.dry_run { ui.colored_label(Color32::from_rgb(160, 90, 0), "안전 테스트 모드 · 실제 화면 보호기를 실행하지 않습니다."); }
        });
        ctx.request_repaint_after(Duration::from_millis(200));
        if self.smoke {
            if let Some((cancelled, count)) = self.smoke_cancel_at {
                if cancelled.elapsed() > Duration::from_millis(1600) {
                    if state.launches == count && state.deadline.is_none() && !state.busy {
                        self.smoke = false;
                        println!("SMOKE_OK: GUI rendered; at least 3 interval launches; cancellation stopped repeats");
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        eprintln!("SMOKE_FAILED: repeats continued after cancel");
                        std::process::exit(1);
                    }
                }
            } else if state.launches >= 3 && state.deadline.is_some() && !state.busy && !state.error
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
