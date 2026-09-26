//! Key-code and modifier helpers shared by the input widgets.
//!
//! Key events arrive either as Win32 virtual-key codes (`WM_KEYDOWN`) or as
//! `Qt::Key` values (X11/Wayland backends), so every predicate accepts both
//! encodings, matching the convention used by `LineEdit` and `Button`.

pub(crate) fn is_up(key: u32) -> bool {
    matches!(key, 0x26 | 0x0100_0013)
}

pub(crate) fn is_down(key: u32) -> bool {
    matches!(key, 0x28 | 0x0100_0015)
}

pub(crate) fn is_left(key: u32) -> bool {
    matches!(key, 0x25 | 0x0100_0012)
}

pub(crate) fn is_right(key: u32) -> bool {
    matches!(key, 0x27 | 0x0100_0014)
}

pub(crate) fn is_page_up(key: u32) -> bool {
    matches!(key, 0x21 | 0x0100_0016)
}

pub(crate) fn is_page_down(key: u32) -> bool {
    matches!(key, 0x22 | 0x0100_0017)
}

pub(crate) fn is_home(key: u32) -> bool {
    matches!(key, 0x24 | 0x0100_0010)
}

pub(crate) fn is_end(key: u32) -> bool {
    matches!(key, 0x23 | 0x0100_0011)
}

pub(crate) fn is_enter(key: u32) -> bool {
    matches!(key, 0x0D | 0x0100_0004 | 0x0100_0005)
}

pub(crate) fn is_escape(key: u32) -> bool {
    matches!(key, 0x1B | 0x0100_0000)
}

pub(crate) fn is_backspace(key: u32) -> bool {
    matches!(key, 0x08 | 0x0100_0003)
}

pub(crate) fn is_delete(key: u32) -> bool {
    matches!(key, 0x2E | 0x0100_0007)
}

pub(crate) fn is_f4(key: u32) -> bool {
    matches!(key, 0x73 | 0x0100_0033)
}

pub(crate) const KEY_SPACE: u32 = 0x20;

pub(crate) fn has_shift(modifiers: u32) -> bool {
    modifiers & 0x0200_0000 != 0 || modifiers & 1 != 0
}

pub(crate) fn has_ctrl(modifiers: u32) -> bool {
    modifiers & 0x0400_0000 != 0 || modifiers & 2 != 0
}

pub(crate) fn has_alt(modifiers: u32) -> bool {
    modifiers & 0x0800_0000 != 0 || modifiers & 4 != 0
}

/// Maps a key code to the character it types for numeric/text entry.
///
/// Navigation codes (`0x21..=0x28`, `0x2E`) are never treated as text, since
/// they collide with Win32 virtual keys; use `InputMethod` commits for those
/// characters instead.
pub(crate) fn typed_char(key: u32, modifiers: u32) -> Option<char> {
    if has_ctrl(modifiers) || has_alt(modifiers) {
        return None;
    }
    match key {
        0x30..=0x39 => char::from_u32(key),
        0x41..=0x5A => {
            let c = char::from_u32(key)?;
            Some(if has_shift(modifiers) { c } else { c.to_ascii_lowercase() })
        }
        // Numeric keypad digits (VK_NUMPAD0..VK_NUMPAD9).
        0x60..=0x69 => char::from_u32(key - 0x60 + u32::from(b'0')),
        0x20 | 0x2B | 0x2C | 0x2D | 0x2F => char::from_u32(key),
        // VK_OEM_PLUS, VK_OEM_COMMA, VK_OEM_MINUS, VK_OEM_PERIOD
        0xBB => Some('+'),
        0xBC => Some(','),
        0xBD => Some('-'),
        0xBE => Some('.'),
        // VK_ADD, VK_SUBTRACT, VK_DECIMAL
        0x6B => Some('+'),
        0x6D => Some('-'),
        0x6E => Some('.'),
        _ => None,
    }
}
