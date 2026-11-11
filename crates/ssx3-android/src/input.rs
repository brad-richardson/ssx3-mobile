use crate::{gamepad, lifecycle, App};
use winit::event_loop::ActiveEventLoop;
use winit::platform::android::activity::input::{Axis, InputEvent, KeyAction, MotionAction};
use winit::platform::android::activity::InputStatus;
use winit::platform::android::ActiveEventLoopExtAndroid;

fn now_ms() -> u64 {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        ns: i64,
    }
    extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut t = Timespec { sec: 0, ns: 0 };
    unsafe { clock_gettime(1, &mut t) };
    t.sec as u64 * 1000 + t.ns as u64 / 1_000_000
}

pub fn poll(app: &mut App, event_loop: &ActiveEventLoop) {
    app.router.menu(app.menu);
    if let Ok(mut iter) = event_loop.android_app().input_events_iter() {
        while iter.next(|event| match event {
            InputEvent::KeyEvent(key) => {
                let code: u32 = key.key_code().into();
                let down = match key.action() {
                    KeyAction::Down => true,
                    KeyAction::Up => false,
                    _ => return InputStatus::Unhandled,
                };
                if code == gamepad::KEYCODE_BACK {
                    if down {
                        lifecycle::java(lifecycle::JavaAction::Background);
                    }
                } else if gamepad::opens_menu(code) {
                    if down && key.repeat_count() == 0 {
                        app.menu_action(gamepad::Action::Toggle);
                    }
                } else if let Some(bit) = gamepad::button_bit(code) {
                    if !down || key.repeat_count() == 0 {
                        for action in
                            app.router
                                .key(key.event_time().max(0) as u64 / 1_000_000, bit, down)
                        {
                            app.menu_action(action);
                        }
                    }
                } else {
                    return InputStatus::Unhandled;
                }
                InputStatus::Handled
            }
            InputEvent::MotionEvent(m) if gamepad::is_controller(m.source().into()) => {
                if m.action() != MotionAction::Move || m.pointer_count() == 0 {
                    return InputStatus::Unhandled;
                }
                let p = m.pointer_at_index(0);
                let a = |axis| p.axis_value(axis);
                for action in app.router.motion(
                    m.event_time().max(0) as u64 / 1_000_000,
                    [a(Axis::Z), a(Axis::Rz), a(Axis::X), a(Axis::Y)],
                    gamepad::axis_buttons(
                        a(Axis::HatX),
                        a(Axis::HatY),
                        a(Axis::Ltrigger).max(a(Axis::Brake)),
                        a(Axis::Rtrigger).max(a(Axis::Gas)),
                    ),
                ) {
                    app.menu_action(action);
                }
                InputStatus::Handled
            }
            _ => InputStatus::Unhandled,
        }) {}
    }
    for action in app.router.advance(now_ms()) {
        app.menu_action(action);
    }
}

/// Controller key winit consumed in the narrow interval after our queue read.
pub fn pad_key(app: &mut App, code: u32, down: bool) {
    if let Some(bit) = gamepad::button_bit(code) {
        for action in app.router.key(now_ms(), bit, down) {
            app.menu_action(action);
        }
    } else if down && gamepad::opens_menu(code) {
        app.menu_action(gamepad::Action::Toggle);
    }
}
