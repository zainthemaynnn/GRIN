use bevy::prelude::*;

// useless? perhaps. but I am a numpad enjoyer.
pub trait KeyCodeExt: Sized {
    const ANY_UP: [Self; 2];
    const ANY_LEFT: [Self; 2];
    const ANY_DOWN: [Self; 2];
    const ANY_RIGHT: [Self; 2];
    const ANY_ZERO: [Self; 2];
    const ANY_ONE: [Self; 2];
    const ANY_TWO: [Self; 2];
    const ANY_THREE: [Self; 2];
    const ANY_FOUR: [Self; 2];
    const ANY_FIVE: [Self; 2];
    const ANY_SIX: [Self; 2];
    const ANY_SEVEN: [Self; 2];
    const ANY_EIGHT: [Self; 2];
    const ANY_NINE: [Self; 2];
    fn any_numbered(num: usize) -> Vec<Self>;
}

impl KeyCodeExt for KeyCode {
    const ANY_UP: [Self; 2] = [Self::KeyW, Self::ArrowUp];
    const ANY_LEFT: [Self; 2] = [Self::KeyA, Self::ArrowLeft];
    const ANY_DOWN: [Self; 2] = [Self::KeyS, Self::ArrowDown];
    const ANY_RIGHT: [Self; 2] = [Self::KeyD, Self::ArrowRight];
    const ANY_ZERO: [Self; 2] = [Self::Digit0, Self::Numpad0];
    const ANY_ONE: [Self; 2] = [Self::Digit1, Self::Numpad1];
    const ANY_TWO: [Self; 2] = [Self::Digit2, Self::Numpad2];
    const ANY_THREE: [Self; 2] = [Self::Digit3, Self::Numpad3];
    const ANY_FOUR: [Self; 2] = [Self::Digit4, Self::Numpad4];
    const ANY_FIVE: [Self; 2] = [Self::Digit5, Self::Numpad5];
    const ANY_SIX: [Self; 2] = [Self::Digit6, Self::Numpad6];
    const ANY_SEVEN: [Self; 2] = [Self::Digit7, Self::Numpad7];
    const ANY_EIGHT: [Self; 2] = [Self::Digit8, Self::Numpad8];
    const ANY_NINE: [Self; 2] = [Self::Digit9, Self::Numpad9];

    fn any_numbered(num: usize) -> Vec<Self> {
        match num {
            0 => Self::ANY_ZERO,
            1 => Self::ANY_ONE,
            2 => Self::ANY_TWO,
            3 => Self::ANY_THREE,
            4 => Self::ANY_FOUR,
            5 => Self::ANY_FIVE,
            6 => Self::ANY_SIX,
            7 => Self::ANY_SEVEN,
            8 => Self::ANY_EIGHT,
            9 => Self::ANY_NINE,
            _ => panic!("only 0-9 are supported as numbered keys"),
        }
        .into()
    }
}

// this is a bit less useless
pub trait InputExt {
    fn pressed_number(&self) -> Option<usize>;
    fn just_released_number(&self) -> Option<usize>;
    fn just_pressed_number(&self) -> Option<usize>;
}

impl InputExt for ButtonInput<KeyCode> {
    /// Return a pressed number, if any. Prioritizes decreasingly from 0-9.
    fn pressed_number(&self) -> Option<usize> {
        for n in 0..=9 {
            if self.any_pressed(KeyCode::any_numbered(n)) {
                return Some(n);
            }
        }
        None
    }

    /// Return a just released number, if any. Prioritizes decreasingly from 0-9.
    fn just_released_number(&self) -> Option<usize> {
        for n in 0..=9 {
            if self.any_just_released(KeyCode::any_numbered(n)) {
                return Some(n);
            }
        }
        None
    }

    /// Return a just pressed number, if any. Prioritizes decreasingly from 0-9.
    fn just_pressed_number(&self) -> Option<usize> {
        for n in 0..=9 {
            if self.any_just_pressed(KeyCode::any_numbered(n)) {
                return Some(n);
            }
        }
        None
    }
}
