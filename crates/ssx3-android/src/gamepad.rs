//! Android game controllers: key codes and axes to pad 1, and the
//! settings-overlay chord. Pure (Android's codes and values as plain
//! numbers), so the host tests it.
//!
//! Buttons map by position as on the desktop (Android names them after the
//! Xbox layout): the bottom face button is ✕, the right one ○, the left one
//! □, the top one △. The left stick is X/Y, the right stick Z/RZ. Analog
//! triggers press L2/R2 past half travel; a hat (a d-pad reported as axes)
//! presses the d-pad.
//!
//! Select+Start together (within 200 ms), holding Start for 600 ms, or the
//! controller's mode button opens and closes the settings overlay. Start and
//! Select therefore reach the game on release (a tap) or, for Select, after
//! 200 ms held. In the overlay the d-pad or a stick moves, ✕ selects and ○
//! closes. Controls held when the overlay opens or closes, or when the game
//! pauses, stay released for the game until they are let go.
use std::collections::VecDeque;
use winit::keyboard::KeyCode;

// android.view.KeyEvent key codes.
pub const KEYCODE_BACK: u32 = 4;
const KEYCODE_DPAD_UP: u32 = 19;
const KEYCODE_DPAD_DOWN: u32 = 20;
const KEYCODE_DPAD_LEFT: u32 = 21;
const KEYCODE_DPAD_RIGHT: u32 = 22;
const KEYCODE_BUTTON_A: u32 = 96;
const KEYCODE_BUTTON_B: u32 = 97;
const KEYCODE_BUTTON_X: u32 = 99;
const KEYCODE_BUTTON_Y: u32 = 100;
const KEYCODE_BUTTON_L1: u32 = 102;
const KEYCODE_BUTTON_R1: u32 = 103;
const KEYCODE_BUTTON_L2: u32 = 104;
const KEYCODE_BUTTON_R2: u32 = 105;
const KEYCODE_BUTTON_THUMBL: u32 = 106;
const KEYCODE_BUTTON_THUMBR: u32 = 107;
const KEYCODE_BUTTON_START: u32 = 108;
const KEYCODE_BUTTON_SELECT: u32 = 109;
const KEYCODE_BUTTON_MODE: u32 = 110;

// android.view.InputDevice sources: the class bits a controller's events carry.
const SOURCE_GAMEPAD: u32 = 0x0000_0401;
const SOURCE_JOYSTICK: u32 = 0x0100_0010;

pub const SELECT: u16 = 1 << 0;
pub const START: u16 = 1 << 3;
const CHORD: u16 = SELECT | START;
const CIRCLE: u16 = 1 << 13;
const CROSS: u16 = 1 << 14;
const DPAD: u16 = 0xf0;

/// libpad button bit for a controller's button or d-pad key.
pub fn button_bit(keycode: u32) -> Option<u16> {
    Some(match keycode {
        KEYCODE_BUTTON_SELECT => 1 << 0,
        KEYCODE_BUTTON_THUMBL => 1 << 1,
        KEYCODE_BUTTON_THUMBR => 1 << 2,
        KEYCODE_BUTTON_START => 1 << 3,
        KEYCODE_DPAD_UP => 1 << 4,
        KEYCODE_DPAD_RIGHT => 1 << 5,
        KEYCODE_DPAD_DOWN => 1 << 6,
        KEYCODE_DPAD_LEFT => 1 << 7,
        KEYCODE_BUTTON_L2 => 1 << 8,
        KEYCODE_BUTTON_R2 => 1 << 9,
        KEYCODE_BUTTON_L1 => 1 << 10,
        KEYCODE_BUTTON_R1 => 1 << 11,
        KEYCODE_BUTTON_Y => 1 << 12, // Triangle (top)
        KEYCODE_BUTTON_B => 1 << 13, // Circle (right)
        KEYCODE_BUTTON_A => 1 << 14, // Cross (bottom)
        KEYCODE_BUTTON_X => 1 << 15, // Square (left)
        _ => return None,
    })
}

/// Whether a key opens and closes the settings overlay by itself.
pub fn opens_menu(keycode: u32) -> bool {
    keycode == KEYCODE_BUTTON_MODE
}

/// Whether an input source (a bit set) is a game controller.
pub fn is_controller(source: u32) -> bool {
    source & SOURCE_GAMEPAD == SOURCE_GAMEPAD || source & SOURCE_JOYSTICK == SOURCE_JOYSTICK
}

/// The desktop key a keyboard's Android key code stands for: a keyboard
/// keeps the desktop bindings (and F10).
pub fn desktop_key(keycode: u32) -> Option<KeyCode> {
    Some(match keycode {
        8 => KeyCode::Digit1,
        9 => KeyCode::Digit2,
        19 => KeyCode::ArrowUp,
        20 => KeyCode::ArrowDown,
        21 => KeyCode::ArrowLeft,
        22 => KeyCode::ArrowRight,
        29 => KeyCode::KeyA,
        45 => KeyCode::KeyQ,
        47 => KeyCode::KeyS,
        51 => KeyCode::KeyW,
        52 => KeyCode::KeyX,
        54 => KeyCode::KeyZ,
        60 => KeyCode::ShiftRight,
        62 => KeyCode::Space,
        66 => KeyCode::Enter,
        67 => KeyCode::Backspace,
        111 => KeyCode::Escape,
        140 => KeyCode::F10,
        _ => return None,
    })
}

/// A stick axis (-1..1; Android: right and down positive) as the PS2's byte
/// (0 = left/up, 0xFF = right/down).
pub fn stick_byte(v: f32) -> u8 {
    if v.is_finite() {
        (128.0 + v.clamp(-1.0, 1.0) * 127.0) as u8
    } else {
        128
    }
}

/// The buttons a controller reports as axes: the hat (d-pad: -1, 0 or 1)
/// and the analog triggers (0..1; L2/R2 past half travel).
pub fn axis_buttons(hat_x: f32, hat_y: f32, l2: f32, r2: f32) -> u16 {
    let mut b = 0;
    for (on, bit) in [
        (hat_y < -0.5, 1 << 4),
        (hat_x > 0.5, 1 << 5),
        (hat_y > 0.5, 1 << 6),
        (hat_x < -0.5, 1 << 7),
        (l2 > 0.5, 1 << 8),
        (r2 > 0.5, 1 << 9),
    ] {
        if on {
            b |= bit;
        }
    }
    b
}

/// Pad 1 as the game sees it: libpad buttons and sticks (RX, RY, LX, LY).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub buttons: u16,
    pub sticks: [u8; 4],
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            buttons: 0,
            sticks: [128; 4],
        }
    }
}

/// What the controller asks of the overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Toggle,
    Up,
    Down,
    Left,
    Right,
    Select,
    Close,
}

#[derive(Clone, Copy)]
enum Chord {
    Idle,
    Pending { bit: u16, at: u64 },
    Consumed,
    Ordinary,
}

/// Controller state in event time (milliseconds), published to the game
/// once per pad update. Taps that begin and end between two updates still
/// reach the game, one update each.
pub struct Router {
    keys: u16,
    axes: u16,
    sticks: [f32; 4],
    blocked: u16,
    blocked_stick: [bool; 2],
    chord: Chord,
    pulses: VecDeque<u16>,
    last_guest: u16,
    menu: bool,
    focused: bool,
    nav: u16,
    clock: u64,
}
impl Default for Router {
    fn default() -> Self {
        Self {
            keys: 0,
            axes: 0,
            sticks: [0.; 4],
            blocked: 0,
            blocked_stick: [false; 2],
            chord: Chord::Idle,
            pulses: VecDeque::new(),
            last_guest: 0,
            menu: false,
            focused: true,
            nav: 0,
            clock: 0,
        }
    }
}
impl Router {
    fn raw(&self) -> u16 {
        self.keys | self.axes
    }
    fn pulse(&mut self, bits: u16) {
        if bits == 0 || self.menu || !self.focused {
            return;
        }
        if let Some(last) = self.pulses.back_mut().filter(|p| **p & bits == 0) {
            *last |= bits;
        } else if self.pulses.len() < 256 {
            self.pulses.push_back(bits);
        } else {
            // A bounded queue fails closed rather than dropping a release.
            self.focus(false);
        }
    }
    fn quarantine(&mut self) {
        self.blocked |= self.raw();
        for s in 0..2 {
            self.blocked_stick[s] |= !self.centered(s);
        }
        self.pulses.clear();
        self.last_guest = 0;
    }
    /// The overlay opened or closed: held controls act on neither side.
    pub fn menu(&mut self, open: bool) {
        if self.menu == open {
            return;
        }
        self.menu = open;
        self.chord = if self.raw() & CHORD != 0 {
            Chord::Consumed
        } else {
            Chord::Idle
        };
        self.quarantine();
        self.nav = self.navigation();
    }
    /// The game paused or resumed (focus, surface, lifecycle).
    pub fn focus(&mut self, focused: bool) {
        self.focused = focused;
        self.quarantine();
        self.nav = if self.menu { self.navigation() } else { 0 };
        self.chord = if self.blocked & CHORD != 0 {
            Chord::Consumed
        } else {
            Chord::Idle
        };
    }
    fn centered(&self, s: usize) -> bool {
        self.sticks[s * 2..s * 2 + 2].iter().all(|x| x.abs() < 0.5)
    }
    fn navigation(&self) -> u16 {
        let mut n = self.raw() & (DPAD | CROSS | CIRCLE);
        for s in 0..2 {
            let (x, y) = (self.sticks[s * 2], self.sticks[s * 2 + 1]);
            if x <= -0.5 {
                n |= 1 << 7;
            }
            if x >= 0.5 {
                n |= 1 << 5;
            }
            if y <= -0.5 {
                n |= 1 << 4;
            }
            if y >= 0.5 {
                n |= 1 << 6;
            }
        }
        // Opposite directions cancel.
        if n & 0x50 == 0x50 {
            n &= !0x50;
        }
        if n & 0xa0 == 0xa0 {
            n &= !0xa0;
        }
        n
    }
    fn toggle(&mut self) -> Vec<Action> {
        self.menu(!self.menu);
        self.chord = Chord::Consumed;
        vec![Action::Toggle]
    }
    /// Time passes: a held Start opens the overlay, a held Select reaches
    /// the game.
    pub fn advance(&mut self, ms: u64) -> Vec<Action> {
        self.clock = self.clock.max(ms);
        if !self.focused {
            return vec![];
        }
        if let Chord::Pending { bit, at } = self.chord {
            let elapsed = self.clock.saturating_sub(at);
            if bit == SELECT && elapsed >= 200 {
                self.chord = Chord::Ordinary;
                self.pulse(self.raw() & CHORD);
            } else if bit == START && elapsed >= 600 {
                return self.toggle();
            }
        }
        vec![]
    }
    pub fn key(&mut self, ms: u64, bit: u16, down: bool) -> Vec<Action> {
        let mut out = self.advance(ms);
        let before = self.raw();
        if down {
            self.keys |= bit;
        } else {
            self.keys &= !bit;
        }
        self.changed(before, &mut out);
        out
    }
    /// A controller's axes: sticks (RX, RY, LX, LY) and the buttons it
    /// reports as axes.
    pub fn motion(&mut self, ms: u64, sticks: [f32; 4], axes: u16) -> Vec<Action> {
        let mut out = self.advance(ms);
        let before = self.raw();
        self.sticks = sticks.map(|x| if x.is_finite() { x.clamp(-1., 1.) } else { 0. });
        self.axes = axes;
        for s in 0..2 {
            if self.centered(s) {
                self.blocked_stick[s] = false;
            }
        }
        self.changed(before, &mut out);
        out
    }
    fn changed(&mut self, before: u16, out: &mut Vec<Action>) {
        let raw = self.raw();
        // Only a full release of a control ends its quarantine.
        self.blocked &= raw;
        if !self.focused {
            return;
        }
        let rising = raw & !before & !self.blocked;
        let chord_raw = raw & CHORD;
        match self.chord {
            Chord::Consumed => {
                if chord_raw == 0 && self.blocked & CHORD == 0 {
                    self.chord = Chord::Idle;
                }
            }
            Chord::Ordinary => {
                self.pulse(rising & CHORD);
                if chord_raw == 0 {
                    self.chord = Chord::Idle;
                }
            }
            Chord::Idle if rising & CHORD != 0 => {
                if rising & CHORD == CHORD {
                    out.extend(self.toggle());
                } else {
                    self.chord = Chord::Pending {
                        bit: rising & CHORD,
                        at: self.clock,
                    };
                }
            }
            Chord::Pending { bit, at } => {
                let other = CHORD ^ bit;
                if rising & other != 0 {
                    if chord_raw & bit != 0 && self.clock.saturating_sub(at) < 200 {
                        out.extend(self.toggle());
                    } else {
                        self.chord = Chord::Ordinary;
                        self.pulse(chord_raw);
                    }
                } else if chord_raw & bit == 0 {
                    self.pulse(bit);
                    self.chord = Chord::Idle;
                }
            }
            _ => {}
        }
        if self.menu {
            let nav = self.navigation();
            let edge = nav & !self.nav;
            self.nav = nav;
            // Circle wins; vertical wins diagonals; one action per edge.
            let action = if edge & CIRCLE != 0 {
                Some(Action::Close)
            } else if edge & 0x10 != 0 {
                Some(Action::Up)
            } else if edge & 0x40 != 0 {
                Some(Action::Down)
            } else if edge & 0x80 != 0 {
                Some(Action::Left)
            } else if edge & 0x20 != 0 {
                Some(Action::Right)
            } else if edge & CROSS != 0 {
                Some(Action::Select)
            } else {
                None
            };
            out.extend(action);
        } else {
            self.pulse(rising & !CHORD);
        }
    }
    /// Pad 1 for one pad update. Repeated quick taps are separated by a
    /// released update.
    pub fn consume(&mut self) -> Snapshot {
        if self.menu || !self.focused {
            return Snapshot::default();
        }
        let mut buttons = self.raw() & !self.blocked & !CHORD;
        if matches!(self.chord, Chord::Ordinary) {
            buttons |= self.raw() & CHORD & !self.blocked;
        }
        // A queued later edge must not appear early merely because it is held.
        buttons &= !self.pulses.iter().fold(0, |bits, p| bits | p);
        if let Some(&pulse) = self.pulses.front() {
            if pulse & self.last_guest != 0 {
                buttons &= !pulse;
            } else {
                buttons |= pulse;
                self.pulses.pop_front();
            }
        }
        self.last_guest = buttons;
        let mut sticks = self.sticks.map(stick_byte);
        for s in 0..2 {
            if self.blocked_stick[s] {
                sticks[s * 2..s * 2 + 2].fill(128);
            }
        }
        Snapshot { buttons, sticks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_table() {
        let codes = [
            109, 106, 107, 108, 19, 22, 20, 21, 104, 105, 102, 103, 100, 97, 96, 99,
        ];
        for (i, k) in codes.into_iter().enumerate() {
            assert_eq!(button_bit(k), Some(1 << i));
        }
        assert_eq!(button_bit(23), None);
        assert_eq!(button_bit(66), None);
        assert!(opens_menu(110) && !opens_menu(108) && !opens_menu(KEYCODE_BACK));
        assert!(is_controller(0x501) && is_controller(0x100_0010) && is_controller(0x100_0511));
        assert!(!is_controller(0x101) && !is_controller(0x201));
        assert_eq!(desktop_key(66), Some(KeyCode::Enter));
        assert_eq!(desktop_key(52), Some(KeyCode::KeyX));
        assert_eq!(desktop_key(140), Some(KeyCode::F10));
        assert_eq!(desktop_key(96), None);
        assert_eq!(desktop_key(24), None); // volume stays with the system
    }

    #[test]
    fn axis_conversion() {
        for (v, b) in [
            (-2., 1),
            (-1., 1),
            (-0.5, 64),
            (0., 128),
            (0.5, 191),
            (1., 255),
            (2., 255),
            (f32::NAN, 128),
            (f32::INFINITY, 128),
        ] {
            assert_eq!(stick_byte(v), b, "{v}");
        }
        for v in [0., 0.49, 0.5] {
            assert_eq!(axis_buttons(0., 0., v, v), 0);
        }
        for v in [0.51, 1.] {
            assert_eq!(axis_buttons(0., 0., v, v), 0x300);
        }
        assert_eq!(axis_buttons(1., -1., 0., 0.), 0x30);
        assert_eq!(axis_buttons(-1., 1., 0., 0.), 0xc0);
        let mut r = Router::default();
        r.motion(0, [0., 1., -1., 0.25], 0);
        assert_eq!(r.consume().sticks, [128, 255, 1, 159]);
    }

    #[test]
    fn held_buttons_and_taps() {
        let mut r = Router::default();
        r.key(0, CROSS, true);
        assert_eq!(r.consume().buttons, CROSS);
        assert_eq!(r.consume().buttons, CROSS);
        r.key(1, CROSS, false);
        assert_eq!(r.consume().buttons, 0);
        // Three taps between updates: three pressed updates, each separated.
        for t in [10, 20, 30] {
            r.key(t, CROSS, true);
            r.key(t + 1, CROSS, false);
        }
        let mut edges = 0;
        let mut last = 0;
        for _ in 0..8 {
            let b = r.consume().buttons;
            edges += (b & !last & CROSS != 0) as u32;
            last = b;
        }
        assert_eq!(edges, 3);
    }

    #[test]
    fn chord_and_start_hold() {
        for first in [START, SELECT] {
            for skew in [0, 150, 199, 200, 250] {
                let mut r = Router::default();
                r.key(0, first, true);
                let a = r.key(skew, CHORD ^ first, true);
                assert_eq!(a.contains(&Action::Toggle), skew < 200, "{first} {skew}");
                if skew < 200 {
                    assert_eq!(r.consume(), Snapshot::default());
                } else {
                    assert!(r.advance(600).is_empty());
                    assert_eq!(r.consume().buttons & CHORD, CHORD);
                }
            }
        }
        for held in [599, 600] {
            let mut r = Router::default();
            r.key(0, START, true);
            assert_eq!(r.consume().buttons, 0);
            let a = r.key(held, START, false);
            if held < 600 {
                assert!(a.is_empty());
                assert_eq!(r.consume().buttons, START);
                assert_eq!(r.consume().buttons, 0);
            } else {
                assert_eq!(a, vec![Action::Toggle]);
            }
        }
        let mut r = Router::default();
        r.key(0, SELECT, true);
        r.advance(199);
        assert_eq!(r.consume().buttons, 0);
        r.advance(200);
        assert_eq!(r.consume().buttons, SELECT);
    }

    #[test]
    fn overlay_navigation_and_quarantine() {
        let mut r = Router::default();
        r.key(0, CROSS, true);
        r.motion(0, [0., 0., 1., 0.], 0);
        r.menu(true);
        assert_eq!(r.consume(), Snapshot::default());
        assert!(r.key(1, CROSS, true).is_empty()); // held from the game
        assert_eq!(r.key(2, CIRCLE, true), vec![Action::Close]);
        r.menu(false);
        // Circle, Cross and the stick stay released until let go.
        assert_eq!(r.consume(), Snapshot::default());
        r.key(3, CROSS, false);
        r.key(4, CIRCLE, false);
        r.motion(5, [0.; 4], 0);
        r.key(6, CROSS, true);
        assert_eq!(r.consume().buttons, CROSS);
        let mut r = Router::default();
        r.menu(true);
        assert_eq!(r.motion(0, [0.; 4], 0x10), vec![Action::Up]); // hat
        assert!(r.motion(1, [0.; 4], 0).is_empty());
        assert_eq!(r.motion(2, [0., 0., 0.5, 0.5], 0), vec![Action::Down]);
        assert_eq!(r.key(3, CROSS, true), vec![Action::Select]);
    }

    #[test]
    fn focus_loss_releases_everything() {
        let mut r = Router::default();
        r.key(0, START, true);
        r.key(1, CROSS, true);
        r.focus(false);
        assert_eq!(r.consume(), Snapshot::default());
        r.focus(true);
        assert!(r.advance(6000).is_empty());
        assert_eq!(r.consume(), Snapshot::default());
        r.key(6001, START, false);
        r.key(6002, CROSS, false);
        r.key(6010, START, true);
        assert_eq!(r.advance(6610), vec![Action::Toggle]);
    }
}
