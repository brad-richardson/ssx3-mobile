//! iOS window and lifecycle owner. The engine stays on the UIKit event thread.
#![cfg(target_os = "ios")]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use ssx3_engine::{ssx3_ps2, Pace};
use ssx3_mobile_core::MobileCore;
use winit::application::ApplicationHandler;
use winit::event::{Touch, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::ios::WindowAttributesExtIOS;
use winit::window::{Window, WindowId};

mod audio;
mod present;
mod storage;
use present::Presenter;

static AUDIO_INTERRUPTED: AtomicBool = AtomicBool::new(false);
static SCENE_CHECK: std::sync::OnceLock<extern "C" fn(*mut c_void) -> bool> = std::sync::OnceLock::new();

extern "C" {
    fn ssx3_ios_audio_activate(mode: i32) -> bool;
    fn ssx3_ios_audio_deactivate();
    fn ssx3_ios_gc_poll(sticks: *mut f32, has_select: *mut bool) -> u32;
    fn ssx3_ios_app_active() -> bool;
}

#[no_mangle]
pub extern "C" fn ssx3_ios_audio_interrupted(interrupted: bool) {
    AUDIO_INTERRUPTED.store(interrupted, Ordering::Release);
    eprintln!("[ios-audio] interruption={}", if interrupted { "began" } else { "ended" });
}

#[no_mangle]
pub extern "C" fn ssx3_ios_entry(scene_check: extern "C" fn(*mut c_void) -> bool) {
    let _ = SCENE_CHECK.set(scene_check);
    std::panic::set_hook(Box::new(|info| eprintln!("panic: {info}")));
    let event_loop = EventLoop::new().expect("iOS event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("[ios] event loop error: {error}");
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AudioMode { Playback, Ambient, Off }
impl AudioMode {
    fn from_environment() -> Self {
        match std::env::var("SSX3_IOS_AUDIO_SESSION").as_deref() {
            Ok("ambient") => Self::Ambient,
            Ok("off") => Self::Off,
            _ => Self::Playback,
        }
    }
    fn activate(self) -> bool {
        self == Self::Off || unsafe { ssx3_ios_audio_activate((self == Self::Ambient) as i32) }
    }
    fn label(self) -> &'static str {
        match self { Self::Playback => "playback", Self::Ambient => "ambient", Self::Off => "off" }
    }
    fn deactivate(self) {
        if self != Self::Off { unsafe { ssx3_ios_audio_deactivate(); } }
    }
}

struct App {
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    core: Option<MobileCore>,
    audio: audio::AudioOwner,
    audio_mode: AudioMode,
    foreground: bool,
    audible: bool,
    audio_session_ready: bool,
    last_audio_activation: Instant,
    cleared_at: Option<Instant>,
    boot_attempted: bool,
    drawable: Option<(u32, u32)>,
    touches: BTreeMap<u64, u16>,
    controller_previous: u32,
    menu_hold: Option<Instant>,
    menu: bool,
    menu_image: Vec<u32>,
    last_log: Instant,
    touch_events: u64,
    touch_pad_frames: u64,
}

impl App {
    fn new() -> Self {
        let audio_mode = AudioMode::from_environment();
        eprintln!("[ios-audio] requested_mode={}", audio_mode.label());
        Self {
            window: None, presenter: None, core: None,
            audio: audio::AudioOwner::new(audio_mode == AudioMode::Playback), audio_mode,
            foreground: false, audible: false, audio_session_ready: false,
            last_audio_activation: Instant::now() - Duration::from_secs(1),
            cleared_at: None, boot_attempted: false,
            drawable: None, touches: BTreeMap::new(), controller_previous: 0,
            menu_hold: None, menu: false, menu_image: Vec::new(), last_log: Instant::now(),
            touch_events: 0, touch_pad_frames: 0,
        }
    }

    fn start_guest(&mut self) {
        self.boot_attempted = true; // Engine::boot installs process-global state once.
        match storage::initialize() {
            Ok(info) => eprintln!("[ios] data_validated=true ELF={} pack={}", info.elf, info.pack),
            Err(error) => { eprintln!("[ios] data_validated=false {error}"); return; }
        }
        let documents = PathBuf::from(std::env::var("SSX3_IOS_DOCUMENTS").expect("Documents"));
        let mut core = MobileCore::boot(documents.join("data"), documents.join("saves"));
        let window = self.window.as_ref().expect("window").clone();
        let (width, height) = self.drawable.expect("stable drawable");
        self.presenter = None; // Clear presenter may use a different device.
        let gpu = ssx3_ps2::gpu::context();
        let mut presenter = pollster::block_on(Presenter::new(window.clone(), gpu));
        presenter.resize(width, height);
        if !presenter.same_device(gpu) {
            eprintln!("[ios] guest presenter does not share the renderer device");
        }
        let display_hz = window.current_monitor()
            .and_then(|monitor| monitor.refresh_rate_millihertz())
            .map_or(60.0, |mhz| (mhz as f64 / 1000.0).clamp(24.0, 500.0));
        core.set_display_hz(display_hz);
        core.set_surface_size(width, height);
        eprintln!("[ios] guest_started=true game_hz={} display_hz={display_hz} drawable={width}x{height} audio=pull",
            core.engine().vsync_hz());
        self.core = Some(core);
        self.presenter = Some(presenter);
        self.sync_activity();
        window.request_redraw();
    }

    fn sync_activity(&mut self) {
        let interrupted = AUDIO_INTERRUPTED.load(Ordering::Acquire);
        if interrupted { self.audio_session_ready = false; }
        let wants_audio = self.foreground && !self.menu && self.audio_mode != AudioMode::Off && !interrupted;
        if wants_audio && !self.audio_session_ready
            && self.last_audio_activation.elapsed() >= Duration::from_secs(1)
        {
            self.last_audio_activation = Instant::now();
            self.audio_session_ready = self.audio_mode.activate();
            if !self.audio_session_ready {
                eprintln!("[ios-audio] activation_retry_pending mode={}", self.audio_mode.label());
            }
        }
        let audible = wants_audio && self.audio_session_ready;
        if let Some(core) = self.core.as_mut() {
            core.activity(self.foreground && !self.menu, audible);
        }
        if self.audible != audible {
            self.audible = audible;
            self.audio.request(audible);
            eprintln!("[ios] audio_output={audible}");
        }
    }

    fn toggle_menu(&mut self) {
        self.menu = !self.menu;
        if self.menu {
            let rate = self.core.as_ref().and_then(|c| c.settings().get("SSX3_TICK_HZ"));
            self.menu_image = menu_image(rate.as_deref() == Some("120"));
        }
        self.touches.clear();
        self.sync_activity();
        if let Some(window) = &self.window { window.request_redraw(); }
    }

    fn toggle_rate(&mut self) {
        let Some(core) = self.core.as_mut() else { return; };
        let is_120 = core.settings().get("SSX3_TICK_HZ").as_deref() == Some("120");
        let next = if is_120 { "" } else { "120" };
        match core.set_setting("SSX3_TICK_HZ", next) {
            Ok(()) => {
                self.menu_image = menu_image(!is_120);
                eprintln!("[ios] saved_game_hz={} applies_after_process_restart=true", if is_120 {60} else {120});
            }
            Err(error) => eprintln!("[ios] save_game_hz failed: {error}"),
        }
    }

    fn poll_controller(&mut self) -> (u16, [u8; 4]) {
        let mut axes = [0f32; 4];
        let mut has_select = false;
        let raw = unsafe { ssx3_ios_gc_poll(axes.as_mut_ptr(), &mut has_select) };
        const MENU: u32 = 1 << 4;
        const OPTIONS: u32 = 1 << 5;
        let rising = raw & !self.controller_previous;
        if raw & MENU != 0 && raw & OPTIONS != 0 && self.controller_previous & (MENU | OPTIONS) != (MENU | OPTIONS) {
            self.toggle_menu();
            self.menu_hold = None;
        } else if raw & MENU != 0 && !has_select {
            let at = self.menu_hold.get_or_insert_with(Instant::now);
            if at.elapsed() >= Duration::from_millis(600) {
                self.toggle_menu();
                self.menu_hold = None;
            }
        } else {
            self.menu_hold = None;
        }
        if self.menu {
            if rising & (1 << 1) != 0 { self.toggle_menu(); }
            else if rising & ((1 << 0) | (1 << 11) | (1 << 13)) != 0 { self.toggle_rate(); }
            self.controller_previous = raw;
            return (0, [128; 4]);
        }
        self.controller_previous = raw;
        let mut buttons = 0u16;
        for (source, target) in [
            (0,14),(1,13),(2,15),(3,12),(4,3),(6,10),(7,11),
            (8,8),(9,9),(10,4),(11,5),(12,6),(13,7),(14,1),(15,2)
        ] {
            if raw & (1 << source) != 0 { buttons |= 1 << target; }
        }
        if has_select && raw & OPTIONS != 0 { buttons |= 1; }
        let sticks = axes.map(|value| {
            if value.is_finite() { (128.0 + value.clamp(-1.0, 1.0) * 127.0) as u8 } else { 128 }
        });
        (buttons, sticks)
    }

    fn touch(&mut self, touch: Touch) {
        self.touch_events += 1;
        eprintln!("[ios-touch] event={} phase={:?} point={:.0},{:.0}",
            self.touch_events, touch.phase, touch.location.x, touch.location.y);
        match touch.phase {
            TouchPhase::Ended | TouchPhase::Cancelled => { self.touches.remove(&touch.id); }
            TouchPhase::Started | TouchPhase::Moved => {
                let Some((width, height)) = self.drawable else { return; };
                let x = touch.location.x / width as f64;
                let y = touch.location.y / height as f64;
                if self.menu {
                    if touch.phase == TouchPhase::Started {
                        if x < 0.22 && y < 0.25 { self.toggle_menu(); }
                        else { self.toggle_rate(); }
                    }
                    return;
                }
                // Transparent touch zones: upper corners for menu/start,
                // left half for directions, right half for face buttons.
                if touch.phase == TouchPhase::Started && y < 0.18 && x < 0.2 {
                    self.toggle_menu();
                    return;
                }
                let button = if y < 0.18 && x > 0.8 { 1 << 3 } // Start
                    else if x < 0.48 {
                        if y < 0.4 { 1 << 4 } else if y > 0.65 { 1 << 6 }
                        else if x < 0.24 { 1 << 7 } else { 1 << 5 }
                    } else if x > 0.77 {
                        if y < 0.43 { 1 << 12 } else if y > 0.7 { 1 << 14 }
                        else { 1 << 13 }
                    } else { 1 << 15 };
                self.touches.insert(touch.id, button);
            }
        }
    }

    fn present(&mut self) {
        let Some(presenter) = self.presenter.as_mut() else { return; };
        let Some(core) = self.core.as_ref() else {
            if self.drawable.is_some() {
                if presenter.render(1, 1, &[0xff4b263f]).is_ok() && self.cleared_at.is_none() {
                    self.cleared_at = Some(Instant::now());
                    eprintln!("[ios] clear_presented=true");
                }
            }
            return;
        };
        let engine = core.engine();
        presenter.set_aspect(engine.aspect_box().unwrap_or(0.0));
        presenter.set_overlay(self.menu.then_some((800, 448, self.menu_image.as_slice())));
        if engine.native_image() {
            if let Some(renderer) = engine.renderer() {
                if presenter.same_device(ssx3_ps2::gpu::context()) {
                    if let Some((view, _, _)) = renderer.present_view() {
                        let _ = presenter.render_view(&view);
                    }
                } else if let Some((width, height, pixels)) = engine.read_image() {
                    let _ = presenter.render(width, height, &pixels);
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.foreground = true;
        event_loop.set_control_flow(ControlFlow::Poll);
        self.audio_session_ready = false;
        if self.window.is_none() {
            let attrs = Window::default_attributes().with_title("SSX 3 Mobile")
                .with_prefers_status_bar_hidden(true).with_prefers_home_indicator_hidden(true);
            let window = Arc::new(event_loop.create_window(attrs).expect("iOS window"));
            let view = match window.window_handle().expect("iOS window handle").as_raw() {
                RawWindowHandle::UiKit(handle) => handle.ui_view.as_ptr(),
                _ => unreachable!("iOS window handle"),
            };
            let attached = SCENE_CHECK.get().is_some_and(|check| check(view));
            eprintln!("[ios] scene_attach={attached}");
            let size = window.outer_size();
            if size.width > 0 && size.height > 0 { self.drawable = Some((size.width, size.height)); }
            let gpu = ssx3_ps2::gpu::context().expect("shared iOS Metal device");
            self.presenter = Some(pollster::block_on(Presenter::new(window.clone(), Some(gpu))));
            self.window = Some(window);
        }
        self.sync_activity();
        if let Some(window) = &self.window { window.request_redraw(); }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.foreground = false;
        self.audio_session_ready = false;
        self.touches.clear();
        self.controller_previous = 0;
        self.sync_activity();
        self.audio_mode.deactivate();
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100)));
        eprintln!("[ios] suspended=true");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                self.drawable = Some((size.width, size.height));
                if let Some(presenter) = self.presenter.as_mut() { presenter.resize(size.width, size.height); }
                if let Some(core) = self.core.as_mut() { core.set_surface_size(size.width, size.height); }
            }
            WindowEvent::Touch(touch) => self.touch(touch),
            WindowEvent::RedrawRequested if self.foreground => {
                self.sync_activity();
                if self.core.is_some() && !self.menu {
                    match self.core.as_mut().unwrap().pace() {
                        Pace::Run(count) => {
                            for _ in 0..count {
                                let (controller, sticks) = self.poll_controller();
                                let touch = self.touches.values().fold(0, |held, bit| held | bit);
                                if touch != 0 { self.touch_pad_frames += 1; }
                                if !self.core.as_mut().unwrap().step(controller | touch, sticks) {
                                    eprintln!("[ios] guest halted {}", self.core.as_ref().unwrap().engine().status());
                                    event_loop.exit();
                                    return;
                                }
                            }
                        }
                        Pace::Wait(wait) => std::thread::sleep(Duration::from_secs_f64(wait.min(0.002))),
                        Pace::Repeat | Pace::Paused => {}
                    }
                }
                self.present();
                if self.last_log.elapsed() >= Duration::from_secs(1) {
                    if let Some(core) = self.core.as_ref() {
                        let (callbacks, missing) = self.audio.counters();
                        eprintln!("[ios] {} audio_callbacks={callbacks} audio_missing_frames={missing} touch_events={} touch_pad_frames={}",
                            core.engine().status(), self.touch_events, self.touch_pad_frames);
                    }
                    self.last_log = Instant::now();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.foreground {
            if unsafe { ssx3_ios_app_active() } {
                self.foreground = true;
                self.audio_session_ready = false;
                event_loop.set_control_flow(ControlFlow::Poll);
                self.sync_activity();
                eprintln!("[ios] foreground_recovered_from_native_state=true");
                if let Some(window) = &self.window { window.request_redraw(); }
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100)));
            }
            return;
        }
        self.poll_controller();
        if !self.boot_attempted && self.drawable.is_some()
            && self.cleared_at.is_some_and(|at| at.elapsed() >= Duration::from_millis(250)) {
            self.start_guest();
        }
        if let Some(window) = &self.window { window.request_redraw(); }
    }
}

fn menu_image(rate_120: bool) -> Vec<u32> {
    use font8x8::UnicodeFonts;
    let mut image = vec![0xB820_2018; 800 * 448];
    let message = format!("SSX 3 MOBILE\\n\\nGame rate: {} Hz (next app restart)\\n\\nTouch anywhere to change\\nTop left: close\\nController A / arrows: change\\nController B: close", if rate_120 {120} else {60});
    let (mut x, mut y) = (32usize, 40usize);
    for character in message.chars() {
        if character == '\n' { x = 32; y += 18; continue; }
        if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
            for (row, pattern) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if pattern & (1 << col) != 0 { image[(y + row) * 800 + x + col] = 0xFFFA_F0EB; }
                }
            }
        }
        x += 9;
    }
    image
}
