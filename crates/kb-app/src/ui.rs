//! Layout system for every screen the browser draws itself: address bar, bottom
//! toolbar, sheets (menu, tabs), the keyboard, lists, home, errors and notices.
//! One stylesheet, built from the constants in `lib.rs`, so edges line up.
//!
//! Measurements (device pixels on the 1264 x 1680 panel):
//! - Unit: 8 px. Side margin 20 px on bars and sheets, 24 px on pages.
//! - Address bar 96 px tall; its pill is 68 px. Bottom toolbar 104 px, five equal cells.
//! - Sheets 780 px tall: a pinned 72 px title strip, then a separate 708 px scrolling body.
//! - Icons: 40 px in rows, 36 px in bars. Touch targets are at least 68 px.
//! - Type: 32 px rows and bar text, 26 px secondary text, 22 px captions, 44 px page titles.
//!
//! Icons are simple line drawings in an inline SVG (24-unit grid, 2-unit stroke), drawn
//! here for this browser. Colours are black on white plus light grey fills and dividers;
//! e-ink shows mid-greys only roughly, so no meaning rests on a tint alone.

use crate::{esc, width_name, DeviceStatus, Key, Layer, BOT_H, LEVEL_MAX, PAGE_WIDTHS, QUICK_H, SHEET_TITLE_H, TOP_H};

/// Line icon from the set below, at `size` device pixels.
pub fn icon(name: &str, size: u32) -> String {
    let body = match name {
        "back" => r#"<path d="M15 5 L8 12 L15 19"/>"#,
        "forward" => r#"<path d="M9 5 L16 12 L9 19"/>"#,
        "reload" => r#"<path d="M19 12 A7 7 0 1 1 16.9 7"/><path d="M19 4 L19 8.5 L14.5 8.5"/>"#,
        "star" => r#"<path d="M12 4 L14.4 9.2 L20 9.8 L15.8 13.6 L17 19.2 L12 16.3 L7 19.2 L8.2 13.6 L4 9.8 L9.6 9.2 Z"/>"#,
        "star-on" => r#"<path fill="currentColor" d="M12 4 L14.4 9.2 L20 9.8 L15.8 13.6 L17 19.2 L12 16.3 L7 19.2 L8.2 13.6 L4 9.8 L9.6 9.2 Z"/>"#,
        "dots" => r#"<circle cx="5" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="19" cy="12" r="1.6" fill="currentColor"/>"#,
        "lock" => r#"<rect x="6" y="11" width="12" height="9" rx="2"/><path d="M9 11 V8 A3 3 0 0 1 15 8 V11"/>"#,
        "info" => r#"<circle cx="12" cy="12" r="8"/><path d="M12 11 V16"/><circle cx="12" cy="8" r="0.8" fill="currentColor"/>"#,
        "tab" => r#"<rect x="4" y="4" width="16" height="16" rx="3"/>"#,
        "plus" => r#"<path d="M12 5 V19 M5 12 H19"/>"#,
        "minus" => r#"<path d="M5 12 H19"/>"#,
        "close" => r#"<path d="M6 6 L18 18 M18 6 L6 18"/>"#,
        "home" => r#"<path d="M4 11 L12 4 L20 11"/><path d="M6 10 V20 H18 V10"/>"#,
        "menu-page" => r#"<rect x="5" y="4" width="14" height="16" rx="2"/><path d="M8 9 H16 M8 13 H16 M8 17 H13"/>"#,
        "read" => r#"<path d="M4 6 H10 A2 2 0 0 1 12 8 V19 A2 2 0 0 0 10 17 H4 Z"/><path d="M20 6 H14 A2 2 0 0 0 12 8 V19 A2 2 0 0 1 14 17 H20 Z"/>"#,
        "text" => r#"<path d="M4 19 L9 6 L14 19 M6.5 14 H11.5 M16 19 L19 11 L22 19"/>"#,
        "moon" => r#"<path d="M19 14.5 A7.5 7.5 0 1 1 9.5 5 A6 6 0 0 0 19 14.5 Z"/>"#,
        "save" => r#"<path d="M12 4 V15 M7 10.5 L12 15.5 L17 10.5"/><path d="M5 19 H19"/>"#,
        "bookmarks" => r#"<path d="M7 4 H17 V20 L12 16 L7 20 Z"/>"#,
        "saved" => r#"<path d="M4 6 H14 A2 2 0 0 1 16 8 V18 H6 A2 2 0 0 1 4 16 Z"/><path d="M16 10 H20 V20 H8 V18"/>"#,
        "trash" => r#"<path d="M5 7 H19 M9 7 V5 H15 V7 M7 7 L8 20 H16 L17 7"/>"#,
        "exit" => r#"<path d="M14 5 H19 V19 H14"/><path d="M4 12 H15 M11 8 L15 12 L11 16"/>"#,
        "newtab" => r#"<rect x="4" y="5" width="16" height="14" rx="2"/><path d="M12 9 V15 M9 12 H15"/>"#,
        "check" => r#"<path d="M5 12.5 L10 17 L19 7"/>"#,
        "image" => r#"<rect x="4" y="5" width="16" height="14" rx="2"/><circle cx="9" cy="10" r="1.6"/><path d="M5 17 L10 13 L13 16 L15 14 L19 18"/>"#,
        "sun" => r#"<circle cx="12" cy="12" r="4"/><path d="M12 2 V5 M12 19 V22 M2 12 H5 M19 12 H22 M4.9 4.9 L7 7 M17 17 L19.1 19.1 M4.9 19.1 L7 17 M17 7 L19.1 4.9"/>"#,
        "warm" => r#"<circle cx="12" cy="12" r="8"/><path d="M12 4 A8 8 0 0 1 12 20 Z" fill="currentColor"/>"#,
        "wifi" => r#"<path d="M2.5 9 A14 14 0 0 1 21.5 9"/><path d="M5.5 12.5 A9.5 9.5 0 0 1 18.5 12.5"/><path d="M8.5 16 A5 5 0 0 1 15.5 16"/><circle cx="12" cy="19.5" r="1" fill="currentColor"/>"#,
        "wifi-off" => r#"<path d="M2.5 9 A14 14 0 0 1 21.5 9"/><path d="M5.5 12.5 A9.5 9.5 0 0 1 18.5 12.5"/><path d="M8.5 16 A5 5 0 0 1 15.5 16"/><circle cx="12" cy="19.5" r="1" fill="currentColor"/><path d="M3 3 L21 21" stroke-width="2.4"/>"#,
        "battery" => r#"<rect x="3" y="7" width="16" height="10" rx="2"/><path d="M21 10.5 V13.5"/><rect x="5.5" y="9.5" width="9" height="5" fill="currentColor" stroke="none"/>"#,
        "chev-up" =>r#"<path d="M6 15 L12 9 L18 15"/>"#,
        "chev-down" => r#"<path d="M6 9 L12 15 L18 9"/>"#,
        "up" => r#"<path d="M12 19 V6 M6 11 L12 5 L18 11"/>"#,
        "down" => r#"<path d="M12 5 V18 M6 13 L12 19 L18 13"/>"#,
        "pulse" => r#"<path d="M2 12 H7 L9.5 5 L14 19 L16.5 12 H22"/>"#,
        _ => "",
    };
    format!(
        r#"<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="flex:0 0 auto">{body}</svg>"#
    )
}

/// Shared stylesheet, from the layout constants.
pub fn css() -> String {
    let field_h = TOP_H - 28; // address pill
    let cell_h = BOT_H - 16; // toolbar cell
    format!(
        r#"*{{box-sizing:border-box}}
html,body{{margin:0;background:#fff;color:#111;font-family:sans-serif;overflow:hidden}}
.bar{{display:flex;align-items:center;gap:12px;height:{TOP_H}px;padding:0 20px;border-bottom:2px solid #e2e2e2}}
.bar .title{{flex:1;font-size:32px;color:#444}}
.omni{{display:flex;align-items:center;gap:12px;flex:1;height:{field_h}px;padding:0 24px 0 12px;background:#f1f1f1;border-radius:{rad}px;font-size:30px;color:#111;text-decoration:none;overflow:hidden;white-space:nowrap}}
.omni-text{{overflow:hidden;text-overflow:ellipsis;min-width:0}}
.omni .dim{{color:#777}}
.omni .hint{{color:#777;font-size:30px}}
.iconbtn{{display:flex;align-items:center;justify-content:center;width:{field_h}px;height:{field_h}px;border-radius:{rad}px;color:#111;text-decoration:none}}
.iconbtn.on{{background:#111;color:#fff}}
.iconbtn.off{{color:#bbb}}
.btn{{display:flex;align-items:center;justify-content:center;gap:10px;height:68px;padding:0 24px;border:2px solid #111;border-radius:14px;font-size:30px;color:#111;text-decoration:none;white-space:nowrap}}
.btn.on{{background:#111;color:#fff}}
.btn.off{{color:#bbb;border-color:#ccc}}
.toolbar{{display:flex;align-items:center;justify-content:space-around;height:{BOT_H}px;padding:0 8px;border-top:2px solid #e2e2e2}}
.cell{{display:flex;align-items:center;justify-content:center;flex:1;height:{cell_h}px;color:#111;text-decoration:none}}
.cell.off{{color:#c4c4c4}}
.cell.on{{color:#111}}
.count{{display:flex;align-items:center;justify-content:center;width:44px;height:44px;border:2px solid #111;border-radius:10px;font-size:24px;font-weight:bold;color:#111}}
.sheet{{background:#fff}}
.sheet-title{{display:flex;align-items:center;justify-content:space-between;height:{SHEET_TITLE_H}px;padding:0 24px;font-size:26px;letter-spacing:1px;color:#666;border-top:3px solid #111}}
.sheet-controls{{display:flex;gap:16px}}
.sheet-title .btn{{height:68px;font-size:24px}}
.section{{padding:22px 24px 6px;font-size:22px;color:#777;letter-spacing:1px}}
.menu-label{{padding:18px 24px 10px;font-size:22px;color:#777;letter-spacing:1px}}
.tiles{{display:grid;grid-template-columns:repeat(4,1fr);gap:16px;padding:0 24px}}
.tile{{display:flex;flex-direction:column;justify-content:space-between;height:148px;padding:16px 18px;border:2px solid #d4d4d4;border-radius:18px;color:#111;text-decoration:none;overflow:hidden}}
.tile.on{{background:#111;color:#fff;border-color:#111}}
.tile-ico{{display:flex}}
.tile-label{{font-size:28px;line-height:1.1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.tile-cap{{font-size:22px;color:#777;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.tile.on .tile-cap{{color:#ddd}}
.row{{display:flex;align-items:center;gap:20px;min-height:96px;padding:12px 24px;border-bottom:1px solid #e6e6e6;font-size:32px;color:#111;text-decoration:none}}
.row .main{{flex:1;min-width:0}}
.row .sub{{display:block;font-size:22px;color:#777;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}}
.row .end{{flex:0 0 auto;font-size:28px;color:#666}}
.row .lead{{flex:0 0 auto;color:#333;width:40px;display:flex;justify-content:center}}
.row.current{{font-weight:bold}}
.row.danger .main{{color:#111}}
.page{{padding:{PAGE_TOP}px 24px {PAGE_BOTTOM}px}}
.page h1{{font-size:44px;margin:12px 0 28px;font-weight:bold}}
.section-title{{font-size:24px;color:#777;letter-spacing:1px;margin:36px 0 14px}}
.grid{{display:grid;grid-template-columns:1fr 1fr 1fr;gap:20px}}
.tile{{display:flex;flex-direction:column;justify-content:space-between;height:180px;padding:20px;border:1px solid #dcdcdc;border-radius:16px;color:#111;text-decoration:none}}
.tile .ico{{display:flex;align-items:center;justify-content:center;width:64px;height:64px;border-radius:32px;background:#111;color:#fff;font-size:30px}}
.tile .t{{font-size:26px;line-height:1.25;overflow:hidden;max-height:66px}}
.search{{display:flex;align-items:center;gap:14px;height:72px;padding:0 24px;border-radius:36px;background:#f1f1f1;font-size:30px;color:#666;text-decoration:none}}
.empty{{padding:40px 0;font-size:30px;color:#555}}
.actions{{display:flex;gap:12px;margin-top:28px}}
.actions .btn{{flex:1}}
.toast{{position:absolute;left:20px;right:20px;height:72px;display:flex;align-items:center;justify-content:center;background:#111;color:#fff;border-radius:14px;font-size:28px}}
.caret{{display:inline-block;width:3px;height:44px;background:#111;vertical-align:middle;margin:0 2px}}
.ch{{color:#111}}
.hint{{color:#888;font-size:28px;vertical-align:middle}}
"#,
        rad = field_h / 2,
        PAGE_TOP = TOP_H + 20,
        PAGE_BOTTOM = BOT_H + 20,
    )
}

/// Wrap a body in the shared document shell.
pub fn document(title: &str, body: &str, extra_css: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{t}</title><style>{css}{extra}</style></head><body>{body}</body></html>"#,
        t = esc(title),
        css = css(),
        extra = extra_css,
    )
}

/// Address bar: a grey pill (site badge, address with host in bold, path dimmed) and Reload.
pub fn top(url: &str) -> String {
    let (host, path) = crate::split_url(url);
    let internal = url.is_empty() || url.starts_with("kbp://") || url.starts_with("kb://");
    let (badge, shown) = if internal {
        ("".to_string(), r#"<span class="hint">Search or type an address</span>"#.to_string())
    } else {
        let secure = url.starts_with("https://");
        let name = if secure { "lock" } else { "info" };
        (icon(name, 22), format!(r#"<span class="omni-text"><b>{}</b><span class="dim">{}</span></span>"#, esc(&host), esc(&path)))
    };
    let body = format!(
        r#"<div class="bar"><a class="omni" href="kb:addr">{badge}{shown}</a><a class="iconbtn" href="kb:reload" aria-label="Reload">{r}</a></div>"#,
        r = icon("reload", 34),
    );
    document("Address bar", &body, "")
}

/// Bottom toolbar: Back, Forward, tab count, Save (bookmark), and the menu. Five equal cells.
pub fn bottom(back: bool, fwd: bool, marked: bool, tabs: usize) -> String {
    let cell = |on: bool, name: &str, href: &str| {
        let (class, target) = if on { ("cell on", href) } else { ("cell off", "#") };
        format!(r#"<a class="{class}" href="{target}" aria-label="{label}" aria-disabled="{disabled}">{i}</a>"#,
            label = if name == "back" { "Back" } else { "Forward" }, disabled = !on, i = icon(name, 40))
    };
    let mark = if marked { icon("star-on", 40) } else { icon("star", 40) };
    let body = format!(
        r#"<div class="toolbar">{}{}<a class="cell" href="kb:tabs" aria-label="Tabs ({tabs})"><span class="count">{tabs}</span></a><a class="cell{mk}" href="kb:mark" aria-label="{mark_label}">{mark}</a><a class="cell" href="kb:menu" aria-label="Menu">{dots}</a></div>"#,
        cell(back, "back", "kb:back"),
        cell(fwd, "forward", "kb:fwd"),
        mk = if marked { " on" } else { "" },
        mark_label = if marked { "Remove bookmark" } else { "Add bookmark" },
        dots = icon("dots", 40),
    );
    document("Toolbar", &body, "")
}

/// One menu tile: icon, label and a caption. `on` marks a state that is currently active.
fn menu_tile(href: &str, icon_name: &str, label: &str, caption: &str, on: bool) -> String {
    format!(
        r#"<a class="tile{c}" href="{h}"><span class="tile-ico">{i}</span><span class="tile-label">{l}</span><span class="tile-cap">{p}</span></a>"#,
        c = if on { " on" } else { "" },
        h = esc(href),
        i = icon(icon_name, 44),
        l = esc(label),
        p = esc(caption),
    )
}

/// A labelled group of menu tiles on one row of four.
fn menu_group(title: &str, tiles: &[String]) -> String {
    format!(r#"<div class="menu-label">{title}</div><div class="tiles">{}</div>"#, tiles.concat())
}

/// Menu sheet: three groups of tiles in a four-column grid. Every tile is a full touch target
/// and the grid fills the width, so no space is left empty. Night mode lives in Display.
pub fn menu(reader: bool, cache_mb: u64, marked: bool, display: &str, images: bool) -> String {
    let page = menu_group("PAGE", &[
        menu_tile("kb:reload", "reload", "Reload", "", false),
        menu_tile("kb:reader", "read", if reader { "Normal view" } else { "Reading view" }, if reader { "On" } else { "Off" }, reader),
        menu_tile("kb:display", "text", "Display", display, false),
        menu_tile("kb:images", "image", "Images", if images { "On" } else { "Off" }, images),
    ]);
    let save = menu_group("SAVE", &[
        menu_tile("kb:mark", if marked { "star-on" } else { "star" }, if marked { "Bookmarked" } else { "Bookmark" }, if marked { "Tap to remove" } else { "This page" }, marked),
        menu_tile("kb:save", "save", "Save offline", "Keep a copy", false),
        menu_tile("kbp://bookmarks", "bookmarks", "Bookmarks", "Manage list", false),
        menu_tile("kbp://saved", "saved", "Saved pages", "Offline copies", false),
    ]);
    let browser = menu_group("BROWSER", &[
        menu_tile("kb:home", "home", "Home", "Start page", false),
        menu_tile("kb:newtab", "newtab", "New tab", "", false),
        menu_tile("kb:clearcache", "trash", "Clear cache", &format!("{cache_mb} MB"), false),
        menu_tile("kb:clearhistory", "trash", "Clear history", "", false),
    ]);
    document("Menu", &format!(r#"<div class="sheet menu">{page}{save}{browser}</div>"#), ".menu{padding-bottom:32px}")
}

/// Title strip for a sheet: its name on the left, its controls on the right.
pub fn sheet_header(title: &str, controls: &str) -> String {
    document(title, &format!(
        r#"<div class="sheet-title"><span>{}</span><div class="sheet-controls">{controls}</div></div>"#,
        esc(title)
    ), "")
}

/// The Find control in the menu's title strip: opens the find keyboard.
pub fn find_button() -> String {
    r#"<a class="btn" href="kb:find">Find</a>"#.to_string()
}

/// The Diagnostics control in the menu's title strip, beside Exit.
pub fn diagnostics_button() -> String {
    r#"<a class="btn" href="kbp://diagnostics">Diagnostics</a>"#.to_string()
}

/// The Exit control shown in the menu's title strip.
pub fn exit_button() -> String {
    // Exit closes the browser and returns to the Kindle library. The open tabs are kept, so the
    // browser resumes them when it is opened again.
    r#"<a class="btn on" href="kb:exit">Exit</a>"#.to_string()
}

/// Display sheet: text size steps, page width, and day or night. Changes apply at once and the
/// sheet stays open, so the reader can compare settings on the page.
/// The Display sheet. `scripts` is the script setting of the current site (`None` on browser pages).
pub fn display(
    text_pct: u32,
    width: u32,
    night: bool,
    scripts: Option<bool>,
    page_wave: &str,
    ui_wave: &str,
    flash_on_load: bool,
    page_dither: bool,
    solid_text: bool,
    book_lines: bool,
) -> String {
    let seg = |href: &str, label: &str, on: bool| {
        format!(r#"<a class="segbtn{}" href="{href}">{label}</a>"#, if on { " on" } else { "" })
    };
    let widths: String = PAGE_WIDTHS
        .iter()
        .map(|&w| seg(&format!("kb:width:{w}"), width_name(w), w == width))
        .collect();
    let site = match scripts {
        Some(on) => format!(
            r#"<div class="section">SCRIPTS ON THIS SITE</div><div class="seg">{on_btn}{off_btn}</div><div class="note">Off shows the page without its scripts; text in &lt;noscript&gt; is shown.</div>"#,
            on_btn = seg("kb:scripts:on", "On", on),
            off_btn = seg("kb:scripts:off", "Off", !on),
        ),
        None => String::new(),
    };
    let page_waves: String = crate::settings::PAGE_WAVES
        .iter()
        .map(|w| seg(&format!("kb:wave:page:{w}"), w, *w == page_wave))
        .collect();
    let ui_waves: String = crate::settings::UI_WAVES
        .iter()
        .map(|w| seg(&format!("kb:wave:ui:{w}"), w, *w == ui_wave))
        .collect();
    let refresh = format!(
        r#"<div class="section">SCREEN REFRESH</div><div class="seg"><a class="segbtn" href="kb:refresh">Refresh screen now</a>{on}{off}</div><div class="note">Page turns use the page waveform. AUTO is what the Kindle's reader sends: partial updates, with the driver choosing the waveform. GC16 is partial with full-quality phases; REAGL is for text; DU is fastest. Changes apply after a page turn or a restart.</div>"#,
        on = seg("kb:wave:flash:on", "Flash on new page", flash_on_load),
        off = seg("kb:wave:flash:off", "No flash on new page", !flash_on_load),
    );
    let dither = format!(
        r#"<div class="section">PAGE TURN DITHERING</div><div class="seg">{on}{off}</div><div class="note">Dithering spreads each gray over neighbouring pixels in an ordered 8x8 pattern, so pages are drawn in the panel's own gray steps. Compare On and Off on the same page.</div>"#,
        on = seg("kb:wave:dither:on", "On", page_dither),
        off = seg("kb:wave:dither:off", "Off", !page_dither),
    );
    let solid = format!(
        r#"<div class="section">SOLID TEXT</div><div class="seg">{on}{off}</div><div class="note">On draws near-black and near-white as pure black and white. Sites often set text in a dark gray, and gray letters flash on every page turn; pure letters change without the flash.</div>"#,
        on = seg("kb:wave:solid:on", "On", solid_text),
        off = seg("kb:wave:solid:off", "Off", !solid_text),
    );
    let lines = format!(
        r#"<div class="section">BOOK LINES</div><div class="seg">{on}{off}</div><div class="note">On spaces paragraphs by one blank line, so the lines of the next page land on the rows of this one and a page turn changes as little of the screen as a book reader's.</div>"#,
        on = seg("kb:wave:lines:on", "On", book_lines),
        off = seg("kb:wave:lines:off", "Off", !book_lines),
    );
    let body = format!(
        r#"<div class="sheet">
<div class="section">TEXT SIZE</div>
<div class="stepper"><a class="step" href="kb:font:-" aria-label="Smaller text">A-</a><span class="stepval">{text_pct}%</span><a class="step" href="kb:font:+" aria-label="Larger text">A+</a></div>
<div class="section">PAGE WIDTH</div>
<div class="seg">{widths}</div>
<div class="note">Narrow suits phone layouts. Medium fits this screen. Wide shows more text per line.</div>
<div class="section">THEME</div>
<div class="seg">{day}{dark}</div>{site}<div class="section">PAGE REFRESH</div><div class="seg">{page_waves}</div><div class="section">BARS AND TAPS REFRESH</div><div class="seg">{ui_waves}</div>{solid}{lines}{refresh}{dither}</div>"#,
        day = seg("kb:nightoff", "Day", !night),
        dark = seg("kb:nighton", "Night", night),
        site = site,
        page_waves = page_waves,
        ui_waves = ui_waves,
        refresh = refresh,
        dither = dither,
        solid = solid,
        lines = lines,
    );
    let extra = ".stepper{display:flex;align-items:center;gap:20px;margin:0 24px;height:104px}\
.step{display:flex;align-items:center;justify-content:center;width:160px;height:96px;border:2px solid #111;border-radius:16px;font-size:36px;font-weight:bold;color:#111;text-decoration:none}\
.stepval{flex:1;text-align:center;font-size:40px;color:#111}\
.seg{display:flex;gap:12px;margin:0 24px;height:88px}\
.segbtn{flex:1;display:flex;align-items:center;justify-content:center;border:2px solid #111;border-radius:16px;font-size:30px;color:#111;text-decoration:none}\
.segbtn.on{background:#111;color:#fff}\
.note{margin:14px 24px 0;font-size:22px;color:#777;line-height:1.35}";
    document("Display", &body, extra)
}

/// One row of the quick settings panel: a level bar of 25 steps with − and + at its ends.
/// Each step is a link (`kb:<action>:<level>`), so a tap sets that level.
fn level_row(icon_name: &str, label: &str, action: &str, level: u32) -> String {
    let steps: String = (0..=LEVEL_MAX)
        .map(|n| {
            let on = if n <= level { " on" } else { "" };
            format!(r#"<a class="qp-step{on}" href="kb:{action}:{n}" aria-label="{label} {n}"></a>"#)
        })
        .collect();
    let down = level.saturating_sub(1);
    let up = (level + 1).min(LEVEL_MAX);
    format!(
        r#"<div class="qp-row"><div class="qp-head"><span class="qp-ico">{i}</span><span class="qp-label">{label}</span><span class="qp-value">{level}</span></div><div class="qp-levels"><a class="qp-nudge" href="kb:{action}:{down}" aria-label="{label} less">{minus}</a><div class="qp-bar">{steps}</div><a class="qp-nudge" href="kb:{action}:{up}" aria-label="{label} more">{plus}</a></div></div>"#,
        i = icon(icon_name, 40),
        minus = icon("minus", 36),
        plus = icon("plus", 36),
    )
}

/// The battery icon's styles. The quick settings panel and the battery gallery both use them.
const BATTERY_CSS: &str = ".batt{position:relative;width:128px;height:56px}\
.batt svg{position:absolute;left:0;top:0}\
.batt-num{position:absolute;left:0;top:0;width:128px;height:56px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:bold;color:#111}\
.batt-num.inv{color:#fff}\
.batt-num .bolt{position:absolute;left:24px;top:17px}\
.batt-clip{position:absolute;left:0;top:0;height:56px;overflow:hidden}";

/// A battery drawn to the charge level: the level is solid black and the rest is white. The number
/// (and the bolt while charging) is black where it sits on the white part and white where it sits
/// on the black part. The split is a clip at the level's edge, so the number changes colour there.
pub fn battery_icon(level: u32, charging: bool) -> String {
    let pct = level.min(100);
    let fill = (104.0 * pct as f32 / 100.0).round() as u32;
    // The level's right edge in battery pixels: the body starts at x 8.
    let edge = 8 + fill;
    let bolt = if charging {
        r##"<svg class="bolt" width="14" height="22" viewBox="0 0 14 22" aria-hidden="true"><path d="M8 0 L1 12 H6 L4 22 L13 9 H8 Z" fill="currentColor"/></svg>"##
    } else {
        ""
    };
    format!(
        r##"<div class="batt"><svg width="128" height="56" viewBox="0 0 128 56" aria-hidden="true"><rect x="2" y="4" width="116" height="48" rx="12" fill="#fff" stroke="#111" stroke-width="3"/><rect x="122" y="18" width="5" height="20" rx="2" fill="#111"/><rect x="8" y="10" width="{fill}" height="36" rx="6" fill="#111"/></svg><div class="batt-num">{bolt}<span>{pct}</span></div><div class="batt-clip" style="width:{edge}px"><div class="batt-num inv">{bolt}<span>{pct}</span></div></div></div>"##
    )
}

/// The quick settings panel that the Kindle draws from the top edge: the clock and battery, Wi-Fi,
/// the brightness and warmth bars, and a handle to close it.
pub fn quick_panel(s: &DeviceStatus) -> String {
    let status = match s.battery {
        Some(p) => battery_icon(p, s.charging),
        None => String::new(),
    };
    let body = format!(
        r#"<div class="quick"><div class="qp-top"><div class="qp-time"><span class="qp-clock">{clock}</span><span class="qp-date">{date}</span></div><div class="qp-status"><a class="qp-wifi{off}" href="kb:wifi" aria-label="Wi-Fi {wifi_word}">{wi}</a>{status}</div></div>
{brightness}
{warmth}
<a class="qp-handle" href="kb:quickclose">{up}<span>Close</span></a></div>"#,
        clock = esc(&s.clock),
        date = esc(&s.date),
        off = if s.wifi_on { "" } else { " off" },
        wifi_word = if s.wifi_on { "on, tap to turn off" } else { "off, tap to turn on" },
        wi = icon(if s.wifi_on { "wifi" } else { "wifi-off" }, 44),
        status = status,
        brightness = level_row("sun", "Brightness", "bright", s.brightness),
        warmth = level_row("warm", "Warmth", "warm", s.warmth),
        up = icon("chev-up", 32),
    );
    let extra = ".quick{height:{QUICK_H}px;background:#fff;border-bottom:3px solid #111;padding:0;display:flex;flex-direction:column}\
.qp-top{display:flex;align-items:center;justify-content:space-between;height:120px;padding:0 28px}\
.qp-time{display:flex;flex-direction:column}\
.qp-clock{font-size:52px;font-weight:bold;color:#111}\
.qp-date{font-size:24px;color:#555}\
.qp-status{display:flex;align-items:center;gap:26px}\
.qp-wifi{display:flex;align-items:center;justify-content:center;width:64px;height:64px;color:#111;text-decoration:none;border-radius:32px}\
.qp-wifi.off{color:#8a8a8a}\
.qp-row{padding:12px 28px 4px}\
.qp-head{display:flex;align-items:center;gap:14px;height:56px;color:#111;font-size:28px}\
.qp-label{flex:1}\
.qp-value{font-size:24px;color:#666;min-width:60px;text-align:right}\
.qp-levels{display:flex;align-items:center;gap:10px;height:84px}\
.qp-nudge{display:flex;align-items:center;justify-content:center;width:72px;height:72px;border:2px solid #111;border-radius:36px;font-size:40px;color:#111;text-decoration:none}\
.qp-bar{display:flex;align-items:stretch;gap:3px;flex:1;height:72px}\
.qp-step{flex:1;height:72px;border:2px solid #bdbdbd;border-radius:6px;background:#fff;text-decoration:none}\
.qp-step.on{background:#111;border-color:#111}\
.qp-handle{display:flex;flex-direction:column;align-items:center;justify-content:center;height:72px;margin-top:auto;border-top:1px solid #d8d8d8;color:#111;text-decoration:none;font-size:22px;letter-spacing:1px}".replace("{QUICK_H}", &QUICK_H.to_string());
    document("Quick settings", &body, &format!("{extra}{BATTERY_CSS}"))
}

/// Tab switcher: a grid of cards (two per row), the open tab marked, and a New tab card.
pub fn tabs(tabs: &[(String, String)], active: usize) -> String {
    let mut cards = String::new();
    for (i, (title, url)) in tabs.iter().enumerate() {
        let label = if title.is_empty() { url.as_str() } else { title.as_str() };
        let class = if i == active { "card current" } else { "card" };
        cards.push_str(&format!(
            r#"<div class="{class}"><a class="card-body" href="kb:tab:{i}"><span class="card-title">{l}</span><span class="sub">{u}</span></a><a class="card-close" href="kb:closetab:{i}" aria-label="Close tab">{x}</a></div>"#,
            l = esc(label),
            u = esc(url),
            x = icon("close", 24),
        ));
    }
    cards.push_str(&format!(
        r#"<a class="card new" href="kb:newtab"><span class="plus">{p}</span><span class="card-title">New tab</span></a>"#,
        p = icon("plus", 48)
    ));
    let body = format!(
        r#"<div class="sheet"><div class="cards">{cards}</div></div>"#
    );
    let extra = ".cards{display:grid;grid-template-columns:1fr 1fr;gap:16px;padding:16px 20px}\
.card{position:relative;height:220px;border:1px solid #cfcfcf;border-radius:14px;overflow:hidden}\
.card.current{border:2px solid #111}\
.card-body{display:flex;flex-direction:column;justify-content:space-between;height:100%;padding:18px 90px 14px 18px;color:#111;text-decoration:none}\
.card-title{font-size:26px;line-height:1.2;overflow:hidden;max-height:68px}\
.card .sub{font-size:20px;color:#777;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}\
.card-close{position:absolute;top:8px;right:8px;width:68px;height:68px;display:flex;align-items:center;justify-content:center;border-radius:34px;background:#f1f1f1;color:#111;text-decoration:none}\
.card.new{display:flex;flex-direction:column;align-items:center;justify-content:center;border-style:dashed;text-decoration:none;color:#111}\
.plus{display:flex;margin-bottom:6px}";
    document("Tabs", &body, extra)
}

/// Keyboard header and address field, above the keys. Input mode edits a page field.
/// Most characters of the address shown at once in the bar; the window follows the caret.
const FIELD_VISIBLE_CHARS: usize = 48;

/// The address bar while the keyboard is open. It is the same bar as the page's address bar: the
/// URL field fills the width, and Cancel sits where the reload button sits. A selected address
/// is highlighted and has no caret.
pub fn field_bar(typed: &[char], caret: usize, selected: bool) -> String {
    let caret = caret.min(typed.len());
    // A selected address shows its beginning (the part the next key replaces); otherwise the
    // window follows the caret. An ellipsis marks each side that is cut.
    let start = if selected || caret < FIELD_VISIBLE_CHARS { 0 } else { caret + 1 - FIELD_VISIBLE_CHARS };
    let end = (start + FIELD_VISIBLE_CHARS).min(typed.len());
    let mut text = String::new();
    if selected {
        text.push_str(r#"<span class="sel">"#);
    }
    if start > 0 {
        text.push_str(r#"<span class="dim">…</span>"#);
    }
    for i in start..end {
        if !selected && i == caret {
            text.push_str(r#"<span class="caret"></span>"#);
        }
        text.push_str(&format!(r#"<a class="ch" href="kb:pos:{i}">{}</a>"#, esc(&typed[i].to_string())));
    }
    if end < typed.len() {
        text.push_str(r#"<span class="dim">…</span>"#);
    }
    if selected {
        text.push_str("</span>");
    } else if caret == typed.len() {
        text.push_str(r#"<span class="caret"></span>"#);
    }
    if typed.is_empty() {
        text.push_str(r#"<span class="hint">Type an address or search</span>"#);
    }
    let body = format!(
        r#"<div class="bar"><div class="omni field-omni"><span class="field-text">{text}</span></div><a class="btn" href="kb:cancel">Cancel</a></div>"#
    );
    // A selected address is shown inverted (dark with white text), as browsers do, so the reader
    // can see that the next key replaces it.
    let extra = ".ch{text-decoration:none}.sel{display:inline-block;background:#111;border-radius:6px;padding:0 6px}.sel .ch{color:#fff}\
.field-omni{min-width:0}.field-text{overflow:hidden;white-space:nowrap}.dim{color:#777}";
    document("Address field", &body, extra)
}

/// The suggestions under the address bar: bookmarks and recent pages that match the typed text
/// (or all of them while the address is selected). The list scrolls when it is longer than the
/// space above the keys.
pub fn field_list(suggestions: &[(String, String, bool)]) -> String {
    let mut rows = String::new();
    for (i, (u, t, bookmarked)) in suggestions.iter().enumerate() {
        let label = if t.is_empty() { u.as_str() } else { t.as_str() };
        // Bookmarks and recent pages have different icons, so the list shows where a row comes from.
        let ic = icon(if *bookmarked { "bookmarks" } else { "menu-page" }, 32);
        rows.push_str(&format!(
            r#"<a class="row" href="kb:sugg:{i}"><span class="lead">{ic}</span><span class="main">{l}<span class="sub">{u}</span></span></a>"#,
            l = esc(label),
            u = esc(u),
        ));
    }
    document("Suggestions", &rows, "")
}

/// Keyboard keys: rows from `keyboard_rows`, in a 16 px frame at the bottom.
pub fn keyboard(rows: &[Vec<Key>], area_h: u32) -> String {
    let mut out = String::new();
    for row in rows {
        let keys: String = row
            .iter()
            .map(|k| {
                let size = if k.label.chars().count() > 1 { " sm" } else { "" };
                format!(
                    r#"<a class="k{c}{size}" style="flex:{w}" href="{h}">{l}</a>"#,
                    c = if k.class.is_empty() { String::new() } else { format!(" {}", k.class) },
                    w = k.width,
                    h = esc(&k.href),
                    l = esc(&k.label),
                )
            })
            .collect();
        out.push_str(&format!(r#"<div class="krow">{keys}</div>"#));
    }
    // Keys are 124 px tall on the portrait keyboard; a shorter area (landscape) shrinks them to fit.
    let rows_n = rows.len().max(1) as u32;
    let key_h = (area_h.saturating_sub(32 + 8 * rows_n) / rows_n).min(124);
    let extra = format!(
        "body{{padding:14px 12px 18px;border-top:2px solid #e2e2e2;background:#fff}}\
.krow{{display:flex;gap:8px;margin-bottom:8px}}\
.k{{display:block;height:{key_h}px;line-height:{key_h}px;text-align:center;border:1px solid #bdbdbd;border-radius:12px;background:#f7f7f7;color:#111;font-size:48px;text-decoration:none}}\
.k.sm{{font-size:30px}}.k.on,.k.go{{background:#111;color:#fff;border-color:#111}}"
    );
    document("Keyboard", &out, &extra)
}

/// Keyboard layers from the layout rows (`Layer` decides which set).
pub fn keyboard_for(layer: Layer, go: &str, area_h: u32) -> String {
    keyboard(&crate::keyboard_rows(layer, go), area_h)
}

/// One row of a list page: link, title and caption, and an optional trailing action.
pub struct ListItem {
    pub href: String,
    pub label: String,
    pub sub: String,
    pub action: Option<(String, String)>,
}

/// A titled list page (bookmarks, saved pages), with the same row style as the sheets.
pub fn list(title: &str, items: &[ListItem], empty: &str) -> String {
    let body = if items.is_empty() {
        format!(r#"<div class="page"><h1>{t}</h1><div class="empty">{e}</div></div>"#, t = esc(title), e = esc(empty))
    } else {
        let rows: String = items
            .iter()
            .map(|item| {
                let end = match &item.action {
                    Some((href, label)) => format!(
                        r#"<a class="btn" style="height:56px;padding:0 16px;font-size:24px" href="{h}">{l}</a>"#,
                        h = esc(href),
                        l = esc(label)
                    ),
                    None => String::new(),
                };
                format!(
                    r#"<div class="row"><a class="main" href="{h}" style="color:#111;text-decoration:none"><span class="main" style="display:block">{l}</span><span class="sub">{s}</span></a>{end}</div>"#,
                    h = esc(&item.href),
                    l = esc(&item.label),
                    s = esc(&item.sub),
                )
            })
            .collect();
        format!(r#"<div class="page"><h1>{t}</h1></div>{rows}"#, t = esc(title))
    };
    document(title, &body, "")
}

/// Heading of a home section: its name, a count, optional links, and a text control that shows or
/// hides the section. Text, not an icon, so the control's meaning is clear on e-ink.
fn section_head(title: &str, count: usize, toggle: &str, open: bool, extra: &str) -> String {
    format!(
        r#"<div class="sechead"><span class="sechead-title">{t}</span><span class="sechead-count">{count}</span><span class="grow"></span>{extra}<a class="sechead-toggle" href="{h}">{label}</a></div>"#,
        t = esc(title),
        h = esc(toggle),
        label = if open { "Hide" } else { "Show" },
    )
}

/// Home screen: the address bar is the only search box. Two sections, each with a heading that
/// shows or hides its content: bookmarks as a grid of tiles, and recent pages as rows.
pub fn home(bookmarks: &[(String, String)], recent: &[(String, String)], bm_open: bool, recent_open: bool) -> String {
    let manage = if bookmarks.is_empty() { String::new() } else { r#"<a class="sechead-link" href="kbp://bookmarks">Manage</a>"#.to_string() };
    let bm_body = if !bm_open {
        String::new()
    } else if bookmarks.is_empty() {
        r#"<div class="home-empty">No bookmarks yet. Tap the star on a page to add it here.</div>"#.to_string()
    } else {
        let tiles: String = bookmarks
            .iter()
            .map(|(u, t)| {
                let title = if t.is_empty() { u.as_str() } else { t.as_str() };
                let letter = title.chars().next().unwrap_or('•').to_uppercase().to_string();
                format!(
                    r#"<a class="mark-tile" href="{u}"><span class="mark-badge">{l}</span><span class="mark-title">{t}</span></a>"#,
                    u = esc(u),
                    l = esc(&letter),
                    t = esc(title),
                )
            })
            .collect();
        format!(r#"<div class="mark-grid">{tiles}</div>"#)
    };
    let recent_body = if !recent_open {
        String::new()
    } else if recent.is_empty() {
        r#"<div class="home-empty">Pages you visit appear here.</div>"#.to_string()
    } else {
        let rows: String = recent
            .iter()
            .take(8)
            .map(|(u, t)| {
                let label = if t.is_empty() { u.as_str() } else { t.as_str() };
                format!(
                    r#"<a class="recent-row" href="{u}"><span class="recent-ico">{i}</span><span class="recent-text"><span class="recent-title">{l}</span><span class="recent-url">{u}</span></span></a>"#,
                    i = icon("menu-page", 36),
                    u = esc(u),
                    l = esc(label),
                )
            })
            .collect();
        format!(r#"<div class="recent-list">{rows}</div>"#)
    };
    let body = format!(
        r#"<div class="home"><div class="brand"><img class="brand-logo" src="{logo}" alt=""><span class="brand-name">{name}</span></div><section>{bm_head}{bm_body}</section><section>{rec_head}{recent_body}</section></div>"#,
        logo = crate::brand::logo_data_url(),
        name = esc(crate::brand::NAME),
        bm_head = section_head("BOOKMARKS", bookmarks.len(), "kb:homeshow:bm", bm_open, &manage),
        rec_head = section_head("RECENT", recent.len(), "kb:homeshow:recent", recent_open, ""),
    );
    let extra = ".home{padding:24px 24px 0}\
.brand{display:flex;align-items:center;gap:18px;padding:4px 0 8px}\
.brand-logo{width:72px;height:69px;background:none}\
.brand-name{font-size:36px;font-weight:bold;color:#111}\
.sechead{display:flex;align-items:center;gap:16px;height:64px;margin-top:20px;font-size:22px;letter-spacing:1px;color:#555}\
.sechead-count{font-size:22px;color:#888}\
.sechead-link,.sechead-toggle{display:flex;align-items:center;height:64px;padding:0 4px;font-size:26px;letter-spacing:0;color:#111;text-decoration:none}\
.sechead-toggle{text-decoration:underline}\
.grow{flex:1}\
.mark-grid{display:grid;grid-template-columns:repeat(4,1fr);gap:16px;padding:12px 0 0}\
.mark-tile{display:flex;flex-direction:column;justify-content:space-between;height:148px;padding:16px 18px;border:2px solid #d4d4d4;border-radius:18px;color:#111;text-decoration:none;overflow:hidden}\
.mark-badge{display:flex;align-items:center;justify-content:center;width:52px;height:52px;border-radius:26px;background:#111;color:#fff;font-size:26px}\
.mark-title{font-size:24px;line-height:1.2;max-height:58px;overflow:hidden}\
.recent-list{padding-top:4px}\
.recent-row{display:flex;align-items:center;gap:20px;min-height:96px;padding:12px 4px;border-bottom:1px solid #e6e6e6;color:#111;text-decoration:none}\
.recent-ico{display:flex;color:#333}\
.recent-text{flex:1;min-width:0;display:flex;flex-direction:column;gap:4px}\
.recent-title{font-size:28px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
.recent-url{font-size:22px;color:#777;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
.home-empty{padding:28px 4px;font-size:26px;color:#666}";
    document("Home", &body, extra)
}

/// Bookmark manager: every bookmark with open, move up, move down and remove.
pub fn bookmarks(items: &[(String, String)]) -> String {
    let head = format!(
        r#"<div class="page"><h1>Bookmarks</h1><div class="sub-head">{n} saved</div></div>"#,
        n = items.len()
    );
    let body = if items.is_empty() {
        format!(
            r#"{head}<div class="page"><div class="empty">No bookmarks yet. Open Menu on a page and tap Add bookmark.</div></div>"#
        )
    } else {
        let rows: String = items
            .iter()
            .enumerate()
            .map(|(i, (url, title))| {
                let label = if title.is_empty() { url.as_str() } else { title.as_str() };
                let letter = label.chars().next().unwrap_or('•').to_uppercase().to_string();
                let up = if i > 0 {
                    format!(r#"<a class="bm-btn" href="kb:bmup:{i}" aria-label="Move up">{}</a>"#, icon("up", 30))
                } else {
                    format!(r#"<span class="bm-btn off">{}</span>"#, icon("up", 30))
                };
                let down = if i + 1 < items.len() {
                    format!(r#"<a class="bm-btn" href="kb:bmdown:{i}" aria-label="Move down">{}</a>"#, icon("down", 30))
                } else {
                    format!(r#"<span class="bm-btn off">{}</span>"#, icon("down", 30))
                };
                format!(
                    r#"<div class="bm"><a class="bm-main" href="{u}"><span class="bm-badge">{l}</span><span class="bm-text"><span class="bm-title">{t}</span><span class="sub">{u}</span></span></a><div class="bm-actions">{up}{down}<a class="bm-btn" href="kb:rm:{i}" aria-label="Remove">{trash}</a></div></div>"#,
                    u = esc(url),
                    l = esc(&letter),
                    t = esc(label),
                    trash = icon("trash", 30),
                )
            })
            .collect();
        format!("{head}{rows}")
    };
    let extra = ".sub-head{font-size:22px;color:#777;margin-top:-12px;padding-bottom:12px}\
.bm{display:flex;align-items:center;gap:16px;min-height:112px;padding:14px 20px 14px 24px;border-bottom:1px solid #e6e6e6}\
.bm-main{flex:1;min-width:0;display:flex;align-items:center;gap:20px;color:#111;text-decoration:none}\
.bm-badge{flex:0 0 auto;width:64px;height:64px;border-radius:32px;background:#111;color:#fff;display:flex;align-items:center;justify-content:center;font-size:30px}\
.bm-text{min-width:0;display:flex;flex-direction:column;gap:6px}\
.bm-title{font-size:30px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
.bm-text .sub{font-size:22px;color:#777;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}\
.bm-actions{display:flex;gap:8px;flex:0 0 auto}\
.bm-btn{display:flex;align-items:center;justify-content:center;width:72px;height:72px;border:2px solid #111;border-radius:14px;color:#111;text-decoration:none}\
.bm-btn.off{border-color:#e0e0e0;color:#c4c4c4}";
    document("Bookmarks", &body, extra)
}

/// Full-page error with the reason and the actions that can help.
pub fn error(title: &str, reason: &str, detail: &str) -> String {
    let body = format!(
        r#"<div class="page"><h1>{t}</h1><div class="empty">{r}</div><div class="caption" style="font-size:22px;color:#777;padding:0 0 20px">{d}</div>
<div class="actions"><a class="btn on" href="kb:reload">Try again</a><a class="btn" href="kb:back">Back</a><a class="btn" href="kb:home">Home</a></div></div>"#,
        t = esc(title),
        r = esc(reason),
        d = esc(detail),
    );
    document(title, &body, "")
}

/// One check on the diagnostics page: what it is, the command run, and what it printed.
pub struct Report {
    pub title: String,
    pub command: String,
    pub output: String,
}

const DIAG_CSS: &str = ".actions{margin:0 0 30px}.diag{margin:0 0 26px}.diag-title{font-size:26px;margin:0}\
.diag-cmd{font-size:20px;color:#666;margin:2px 0 6px}\
.diag-out{font-size:20px;margin:0;padding:6px 0 6px 14px;border-left:3px solid #000;white-space:pre-wrap}.ssh-connect{font-size:38px;font-weight:bold;margin:12px 0 10px;padding:16px;border:3px solid #000;border-radius:14px}";

/// The SSH result, shown as one card: the connect command in large type, then the server's status.
fn ssh_card(r: &Report) -> String {
    let connect = r.output.lines().find(|l| l.starts_with("ssh -p ")).unwrap_or("");
    let status: Vec<&str> = r.output.lines().filter(|l| !l.starts_with("ssh -p ") && !l.trim().is_empty()).collect();
    format!(
        r#"<div class="diag"><h2 class="diag-title">SSH server</h2><div class="ssh-connect">{c}</div><div class="diag-cmd">Login: root, with the SSH password.</div><pre class="diag-out">{s}</pre></div>"#,
        c = esc(connect),
        s = esc(&status.join("\n")),
    )
}

/// A select's options as a sheet: one row per option, the chosen one in bold. A tap picks the row.
pub fn choice(title: &str, options: &[(String, bool)]) -> String {
    let rows: String = options
        .iter()
        .enumerate()
        .map(|(i, (label, picked))| {
            format!(
                r#"<a class="pick{on}" href="kb:pick:{i}">{l}</a>"#,
                on = if *picked { " on" } else { "" },
                l = esc(if label.is_empty() { "(empty)" } else { label }),
            )
        })
        .collect();
    let body = format!(r#"<div class="sheet"><div class="section">{}</div>{rows}</div>"#, esc(title));
    document(
        title,
        &body,
        ".pick{display:block;padding:22px 28px;font-size:32px;color:#111;text-decoration:none;border-bottom:1px solid #ddd}.pick.on{font-weight:bold;background:#eee}",
    )
}

/// The diagnostics page: the actions, the last DNS change (if any), then one section per check. Before the
/// first run there are no checks yet, and the page says what the page does.
pub fn diagnostics(report: Option<&[Report]>, fix: Option<&Report>, ssh_on: bool) -> String {
    let run_label = if report.is_some() { "Run again" } else { "Run checks" };
    let intro = if report.is_some() {
        String::new()
    } else {
        r#"<div class="empty">Runs read-only checks on this device: the shell, system, storage, Wi-Fi, DNS, the hosts file, browser files and processes. Set public DNS points this device at 1.1.1.1 and 8.8.8.8, also after Wi-Fi reconnects and restarts (the originals are saved first); Restore DNS puts them back. Start SSH runs an SSH server on port 2222. Set SSH password changes the root password; type it on the keyboard, 6 to 64 letters, digits, or - _ . @ # % + = !.</div>"#.to_string()
    };
    let ssh_label = if ssh_on { "Stop SSH" } else { "Start SSH" };
    let actions = format!(
        r#"<div class="actions"><a class="btn on" href="kb:diag">{run_label}</a><a class="btn" href="kb:back">Back</a></div><div class="diag-cmd">DNS</div><div class="actions"><a class="btn" href="kb:dns:public">Public DNS</a><a class="btn" href="kb:dns:restore">Restore DNS</a></div><div class="diag-cmd">SSH</div><div class="actions"><a class="btn" href="kb:ssh:toggle">{ssh_label}</a><a class="btn" href="kb:ssh:check">Check SSH</a><a class="btn" href="kb:ssh:password">Set SSH password</a></div>"#
    );
    let section = |r: &Report| {
        format!(
            r#"<div class="diag"><h2 class="diag-title">{t}</h2><div class="diag-cmd">{c}</div><pre class="diag-out">{o}</pre></div>"#,
            t = esc(&r.title),
            c = esc(&r.command),
            o = esc(&r.output),
        )
    };
    let fix_section = fix.map(|r| if r.title == "Start SSH" { ssh_card(r) } else { section(r) }).unwrap_or_default();
    let checks: String = report.unwrap_or(&[]).iter().map(section).collect();
    let body = format!(r#"<div class="page"><h1>Diagnostics</h1>{intro}{actions}{fix_section}{checks}</div>"#);
    document("Diagnostics", &body, DIAG_CSS)
}

/// Short notice shown above the bottom toolbar. Placed by the caller.
pub fn toast(message: &str) -> String {
    let body = format!(r#"<div class="toast">{}</div>"#, esc(message));
    document("Notice", &body, "body{height:72px}")
}

#[cfg(test)]
mod battery_tests {
    use super::*;

    #[test]
    fn battery_shows_the_bolt_only_while_charging_and_the_level_inside() {
        let on = battery_icon(64, true);
        let off = battery_icon(64, false);
        assert!(on.contains("M8 0 L1 12"), "bolt missing while charging");
        assert!(!off.contains("M8 0 L1 12"), "bolt shown while not charging");
        assert!(on.contains(">64<") && off.contains(">64<") && !on.contains("64%"));
    }
}

#[cfg(test)]
mod battery_gallery {
    use super::*;

    /// Writes `artifacts/battery/levels.png`: the battery icon at each level, with and without
    /// charging. Runs only when `KB_BATTERY_GALLERY` is set, e.g.
    /// `KB_BATTERY_GALLERY=1 cargo test -p kb-app battery_gallery`.
    #[test]
    fn write_battery_gallery() {
        if std::env::var_os("KB_BATTERY_GALLERY").is_none() {
            return;
        }
        let levels = [0u32, 5, 10, 15, 25, 35, 50, 64, 75, 90, 100];
        let mut cells = String::new();
        for &level in &levels {
            for charging in [false, true] {
                let caption = format!(
                    "level {level} \u{b7} {}",
                    if charging { "charging" } else { "not charging" }
                );
                cells.push_str(&format!(
                    r#"<div class="cell"><div class="caption">{caption}</div>{}</div>"#,
                    battery_icon(level, charging)
                ));
            }
        }
        let body = format!(r#"<div class="grid">{cells}</div>"#);
        let extra = format!(
            "{BATTERY_CSS}.grid{{display:grid;grid-template-columns:1fr 1fr;gap:36px 48px;padding:40px 40px}}\
.cell{{display:flex;flex-direction:column;gap:14px}}.caption{{font-size:24px;color:#333}}"
        );
        let html = document("Battery levels", &body, &extra);
        let (w, h) = (1264u32, 1560u32);
        let net = std::sync::Arc::new(kb_net::SyncNet::new(4 * 1024 * 1024));
        let mut page = kb_engine::Page::new(&html, "kb://chrome/", w, h, net, &mut || true);
        let gray = page.render_gray();
        let png = crate::encode_png(w, h, &gray).expect("encode");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/battery/levels.png");
        std::fs::create_dir_all(out.parent().expect("dir")).expect("create dir");
        std::fs::write(&out, png).expect("write png");
        eprintln!("wrote {}", out.display());
    }
}
