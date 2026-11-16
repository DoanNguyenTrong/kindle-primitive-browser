//! Layout constants in device pixels on the 1264 x 1680 panel, shared by the browser and the drivers.

pub const TOP_H: u32 = 96;

pub const BOT_H: u32 = 104;

pub const KB_Y: u32 = 950;

pub const PROGRESS_H: u32 = 8;

pub const MENU_H: u32 = 780;

pub const SHEET_TITLE_H: u32 = 72;

pub(crate) const MAX_DOC_BYTES: u64 = 6 * 1024 * 1024;

pub(crate) const SUBRESOURCE_MAX_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn toast_rows(height: u32) -> (u32, u32) {
    let y1 = height - BOT_H - 16;
    (y1 - 88, y1)
}
