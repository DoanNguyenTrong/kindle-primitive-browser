//! Layout constants in device pixels, shared by the browser and the drivers. The Kindle driver
//! uses a 1264 x 1680 portrait screen. Bars keep their height while the keyboard area scales
//! with the height supplied by the driver.

pub const TOP_H: u32 = 96;

pub const BOT_H: u32 = 104;

/// Height of the keyboard's keys on the 1680 px portrait screen (the keys start at y 950 there).
const KEYS_H_PORTRAIT: u32 = 730;

/// Top of the keyboard's keys on a screen of `height` device pixels: the address bar and the
/// suggestions take the rest. 950 in portrait.
pub fn kb_top(height: u32) -> u32 {
    height - height * KEYS_H_PORTRAIT / 1680
}

pub const PROGRESS_H: u32 = 8;

pub const MENU_H: u32 = 780;

pub const SHEET_TITLE_H: u32 = 72;

/// Height of the quick settings panel that drops from the top edge.
pub const QUICK_H: u32 = 940;

pub(crate) const MAX_DOC_BYTES: u64 = 6 * 1024 * 1024;

pub(crate) const SUBRESOURCE_MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn toast_rows(height: u32) -> (u32, u32) {
    let y1 = height - BOT_H - 16;
    (y1 - 88, y1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_keys_start_at_950_on_the_portrait_screen() {
        assert_eq!(kb_top(1680), 950);
    }
}
