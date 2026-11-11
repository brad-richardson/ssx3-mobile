#![cfg(target_os = "android")]

use ssx3_engine::{ssx3_ps2, Pace};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

mod audio;
mod bootstrap;
mod card_stage;
mod display;
mod entry;
mod gamepad;
mod input;
mod lifecycle;
mod needs_data;
mod policy;
mod present;

use gamepad::{Action, Router};
use present::Presenter;
use ssx3_mobile_core::MobileCore;

struct App {
    core: MobileCore,
    audio: audio::AudioOwner,
    window: Option<Arc<Window>>,
    presenter: Option<Presenter>,
    router: Router,
    keyboard: u16,
    menu: bool,
    menu_image: Vec<u32>,
    native_focus: bool,
    surface: bool,
    playing: bool,
    audio_active: bool,
    fatal: bool,
    last_log: Instant,
}

impl App {
    fn new(data: PathBuf, saves: PathBuf) -> Self {
        let core = MobileCore::boot(data, saves);
        eprintln!(
            "[mobile] boot game_hz={} audio=pull",
            core.engine().vsync_hz()
        );
        let audio = audio::AudioOwner::new();
        Self {
            core,
            audio,
            window: None,
            presenter: None,
            router: Router::default(),
            keyboard: 0,
            menu: false,
            menu_image: Vec::new(),
            native_focus: false,
            surface: false,
            playing: false,
            audio_active: false,
            fatal: false,
            last_log: Instant::now(),
        }
    }

    fn menu_action(&mut self, action: Action) {
        match action {
            Action::Toggle => self.toggle_menu(),
            Action::Close if self.menu => self.toggle_menu(),
            Action::Select | Action::Left | Action::Right if self.menu => {
                let current = self.core.settings().get("SSX3_TICK_HZ");
                let next = if current.as_deref() == Some("120") {
                    ""
                } else {
                    "120"
                };
                if let Err(e) = self.core.set_setting("SSX3_TICK_HZ", next) {
                    eprintln!("[mobile] save game rate: {e}");
                }
                self.menu_image = menu_image(next);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn toggle_menu(&mut self) {
        self.menu = !self.menu;
        self.router.menu(self.menu);
        self.keyboard = 0;
        if self.menu {
            self.menu_image = menu_image(
                self.core
                    .settings()
                    .get("SSX3_TICK_HZ")
                    .as_deref()
                    .unwrap_or(""),
            );
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn sync_lifecycle(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if lifecycle::destroyed() {
            self.core.activity(false, false);
            self.audio.request(false);
            self.core.shutdown();
            self.presenter = None;
            self.window = None;
            event_loop.exit();
            return false;
        }
        if self.fatal {
            event_loop.set_control_flow(ControlFlow::Wait);
            return false;
        }
        let bits = lifecycle::bits();
        let active = self.surface && self.native_focus && bits & 11 == 3;
        let audible = active && bits & 4 != 0;
        if self.playing != active {
            self.playing = active;
            self.router.focus(active);
            self.keyboard = 0;
            lifecycle::java(if active {
                lifecycle::JavaAction::Playing
            } else {
                lifecycle::JavaAction::Paused
            });
            eprintln!(
                "[mobile] playback={active} frame={}",
                self.core.engine().frames()
            );
        }
        self.core
            .activity(active && !self.menu, audible && !self.menu);
        let audio_active = audible && !self.menu;
        if self.audio_active != audio_active {
            self.audio_active = audio_active;
            self.audio.request(audio_active);
        }
        event_loop.set_control_flow(if active {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });
        active
    }

    fn present(&mut self) {
        let Some(p) = self.presenter.as_mut() else {
            return;
        };
        let engine = self.core.engine();
        p.set_aspect(engine.aspect_box().unwrap_or(0.0));
        p.set_overlay(self.menu.then_some((800, 448, self.menu_image.as_slice())));
        if engine.native_image() {
            if let Some(g) = engine.renderer() {
                let gpu = ssx3_ps2::gpu::context();
                if p.same_device(gpu) {
                    if let Some((view, _, _)) = g.present_view() {
                        let _ = p.render_view(&view);
                    }
                } else if let Some((w, h, px)) = engine.read_image() {
                    let _ = p.render(w, h, &px);
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, _: winit::event::StartCause) {
        input::poll(self, event_loop);
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let w = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("SSX 3"))
                .expect("window"),
        );
        let size = w.inner_size();
        self.surface = size.width > 0 && size.height > 0;
        if self.surface {
            self.presenter = Some(pollster::block_on(Presenter::new(
                w.clone(),
                ssx3_ps2::gpu::context(),
            )));
            self.core.set_surface_size(size.width, size.height);
        }
        self.core
            .set_display_hz(lifecycle::refresh_rate().unwrap_or(60.0));
        lifecycle::request_panel_rate(self.core.engine().vsync_hz());
        w.request_redraw();
        self.window = Some(w);
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.presenter = None;
        self.window = None;
        self.surface = false;
        self.native_focus = false;
        self.core.activity(false, false);
        self.audio_active = false;
        self.audio.request(false);
        self.router.focus(false);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Focused(v) => {
                self.native_focus = v;
                if v {
                    lifecycle::request_panel_rate(self.core.engine().vsync_hz());
                }
            }
            WindowEvent::CloseRequested => lifecycle::java(lifecycle::JavaAction::Finish),
            WindowEvent::Resized(size) => {
                self.surface = size.width > 0 && size.height > 0;
                if self.surface {
                    if self.presenter.is_none() {
                        if let Some(w) = &self.window {
                            self.presenter = Some(pollster::block_on(Presenter::new(
                                w.clone(),
                                ssx3_ps2::gpu::context(),
                            )));
                        }
                    } else if let Some(p) = self.presenter.as_mut() {
                        p.resize(size.width, size.height);
                    }
                    self.core.set_surface_size(size.width, size.height);
                } else {
                    self.presenter = None;
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(KeyCode::F10) = event.physical_key {
                    if down && !event.repeat {
                        self.toggle_menu();
                    }
                } else if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(bit) = keyboard_bit(code) {
                        if down {
                            self.keyboard |= bit;
                        } else {
                            self.keyboard &= !bit;
                        }
                    }
                } else if let PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Android(
                    code,
                )) = event.physical_key
                {
                    if !(down && event.repeat) {
                        input::pad_key(self, code, down);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if !self.sync_lifecycle(event_loop) {
                    return;
                }
                if self.menu {
                    self.present();
                    return;
                }
                let present = match self.core.pace() {
                    Pace::Run(n) => {
                        for _ in 0..n {
                            let snapshot = self.router.consume();
                            if !self
                                .core
                                .step(snapshot.buttons | self.keyboard, snapshot.sticks)
                            {
                                eprintln!("[mobile] halted {}", self.core.engine().status());
                                self.fatal = true;
                                self.core.activity(false, false);
                                self.audio.request(false);
                                lifecycle::java(lifecycle::JavaAction::Fatal);
                                event_loop.set_control_flow(ControlFlow::Wait);
                                return;
                            }
                        }
                        true
                    }
                    Pace::Repeat => true,
                    Pace::Wait(wait) => {
                        std::thread::sleep(Duration::from_secs_f64(wait.min(0.002)));
                        false
                    }
                    Pace::Paused => false,
                };
                if self.last_log.elapsed().as_secs() >= 1 {
                    if let Some(hz) = lifecycle::refresh_rate() {
                        self.core.set_display_hz(hz);
                    }
                    let (callbacks, underrun) = self.audio.counters();
                    eprintln!(
                        "[mobile] {} audio_callbacks={} audio_missing_frames={}",
                        self.core.engine().status(),
                        callbacks,
                        underrun
                    );
                    self.last_log = Instant::now();
                }
                if present {
                    self.present();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.sync_lifecycle(event_loop) {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
}

fn keyboard_bit(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::ArrowUp => 1 << 4,
        KeyCode::ArrowRight => 1 << 5,
        KeyCode::ArrowDown => 1 << 6,
        KeyCode::ArrowLeft => 1 << 7,
        KeyCode::Enter => 1 << 3,
        KeyCode::KeyZ | KeyCode::Space => 1 << 14,
        KeyCode::KeyX => 1 << 13,
        KeyCode::KeyA => 1 << 15,
        _ => return None,
    })
}

fn menu_image(rate: &str) -> Vec<u32> {
    use font8x8::UnicodeFonts;
    let mut image = vec![0xB820_2018; 800 * 448];
    let text = format!("SSX 3 MOBILE\n\nGame rate: {} Hz (next restart)\n\nA / left / right: change\nB / MODE: close", if rate == "120" { 120 } else { 60 });
    let (mut x, mut y) = (32, 40);
    for c in text.chars() {
        if c == '\n' {
            x = 32;
            y += 18;
            continue;
        }
        if let Some(glyph) = font8x8::BASIC_FONTS.get(c) {
            for (gy, row) in glyph.iter().enumerate() {
                for gx in 0..8 {
                    if row & (1 << gx) != 0 {
                        image[(y + gy) * 800 + x + gx] = 0xFFFA_F0EB;
                    }
                }
            }
        }
        x += 9;
    }
    image
}
