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

use crate::{esc, width_name, Key, Layer, BOT_H, KB_Y, PAGE_WIDTHS, TOP_H, SHEET_TITLE_H};

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
        "chev-up" =>r#"<path d="M6 15 L12 9 L18 15"/>"#,
        "chev-down" => r#"<path d="M6 9 L12 15 L18 9"/>"#,
        "up" => r#"<path d="M12 19 V6 M6 11 L12 5 L18 11"/>"#,
        "down" => r#"<path d="M12 5 V18 M6 13 L12 19 L18 13"/>"#,
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
.sheet-title{{display:flex;align-items:center;justify-content:space-between;height:{SHEET_TITLE_H}px;padding:0 24px;font-size:26px;letter-spacing:1px;color:#666;border-bottom:2px solid #111}}
.sheet-controls{{display:flex;gap:16px}}
.sheet-title .btn{{height:68px;font-size:24px}}
.section{{padding:22px 24px 6px;font-size:22px;color:#777;letter-spacing:1px}}
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

/// One row of the menu: icon, label, and an optional value on the right.
fn menu_row(href: &str, icon_name: &str, label: &str, end: &str) -> String {
    format!(
        r#"<a class="row" href="{h}"><span class="lead">{i}</span><span class="main">{l}</span><span class="end">{e}</span></a>"#,
        h = esc(href),
        i = icon(icon_name, 36),
        l = esc(label),
        e = esc(end),
    )
}

/// Menu sheet: grouped rows with icons. The list scrolls when it is taller than the sheet.
pub fn menu(reader: bool, cache_mb: u64, marked: bool, night: bool, display: &str, images: bool) -> String {
    let groups: Vec<(&str, Vec<String>)> = vec![
        ("PAGE", vec![
            menu_row("kb:reload", "reload", "Reload", ""),
            menu_row("kb:reader", "read", if reader { "Normal view" } else { "Reading view" }, ""),
            menu_row("kb:display", "text", "Display", display),
            menu_row("kb:images", "image", "Images", if images { "On" } else { "Off" }),
            menu_row("kb:night", "moon", "Night mode", if night { "On" } else { "Off" }),
        ]),
        ("SAVE", vec![
            menu_row("kb:mark", if marked { "star-on" } else { "star" }, if marked { "Remove bookmark" } else { "Add bookmark" }, ""),
            menu_row("kb:save", "save", "Save for offline", ""),
            menu_row("kbp://bookmarks", "bookmarks", "Bookmarks", ""),
            menu_row("kbp://saved", "saved", "Saved pages", ""),
        ]),
        ("BROWSER", vec![
            menu_row("kb:home", "home", "Home", ""),
            menu_row("kb:clearcache", "trash", "Clear cache", &format!("{cache_mb} MB")),
            menu_row("kb:clearhistory", "trash", "Clear history", ""),
        ]),
    ];
    let mut out = String::new();
    for (title, rows) in groups {
        out.push_str(&format!(r#"<div class="section">{title}</div>"#));
        out.push_str(&rows.concat());
    }
    out.push_str(&format!(r#"<div style="height:24px"></div>"#));
    document("Menu", &format!(r#"<div class="sheet">{out}</div>"#), "")
}

/// A separate layer keeps dismissal (and Exit in the menu) visible as the body scrolls.
pub fn sheet_header(title: &str, exit: bool) -> String {
    let exit = if exit { r#"<a class="btn on" href="kb:exit">Exit browser</a>"# } else { "" };
    document(title, &format!(
        r#"<div class="sheet-title"><span>{}</span><div class="sheet-controls">{exit}<a class="btn" href="kb:closemenu">Done</a></div></div>"#,
        esc(title)
    ), "")
}

/// Display sheet: text size steps, page width, and day or night. Changes apply at once and the
/// sheet stays open, so the reader can compare settings on the page.
pub fn display(text_pct: u32, width: u32, night: bool) -> String {
    let seg = |href: &str, label: &str, on: bool| {
        format!(r#"<a class="segbtn{}" href="{href}">{label}</a>"#, if on { " on" } else { "" })
    };
    let widths: String = PAGE_WIDTHS
        .iter()
        .map(|&w| seg(&format!("kb:width:{w}"), width_name(w), w == width))
        .collect();
    let body = format!(
        r#"<div class="sheet">
<div class="section">TEXT SIZE</div>
<div class="stepper"><a class="step" href="kb:font:-" aria-label="Smaller text">A-</a><span class="stepval">{text_pct}%</span><a class="step" href="kb:font:+" aria-label="Larger text">A+</a></div>
<div class="section">PAGE WIDTH</div>
<div class="seg">{widths}</div>
<div class="note">Narrow suits phone layouts. Medium fits this screen. Wide shows more text per line.</div>
<div class="section">THEME</div>
<div class="seg">{day}{dark}</div></div>"#,
        day = seg("kb:nightoff", "Day", !night),
        dark = seg("kb:nighton", "Night", night),
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
pub fn field(typed: &[char], caret: usize, suggestions: &[(String, String)], input: bool) -> String {
    let caret = caret.min(typed.len());
    let mut text = String::new();
    for (i, c) in typed.iter().enumerate() {
        if i == caret {
            text.push_str(r#"<span class="caret"></span>"#);
        }
        text.push_str(&format!(r#"<a class="ch" href="kb:pos:{i}">{}</a>"#, esc(&c.to_string())));
    }
    if caret == typed.len() {
        text.push_str(r#"<span class="caret"></span>"#);
    }
    if typed.is_empty() {
        text.push_str(r#"<span class="hint">Type an address or search</span>"#);
    }
    let title = if input { "Text field" } else { "Address" };
    let go = if input { "Done" } else { "Go" };
    let mut rows = String::new();
    for (i, (u, t)) in suggestions.iter().enumerate() {
        let label = if t.is_empty() { u.as_str() } else { t.as_str() };
        rows.push_str(&format!(
            r#"<a class="row" href="kb:sugg:{i}"><span class="lead">{ic}</span><span class="main">{l}<span class="sub">{u}</span></span></a>"#,
            ic = icon("bookmarks", 32),
            l = esc(label),
            u = esc(u),
        ));
    }
    let body = format!(
        r#"<div class="bar"><span class="title">{title}</span><a class="btn" href="kb:cancel">Cancel</a><a class="btn on" href="kb:go">{go}</a></div><div style="padding:20px"><div class="omni" style="display:block;height:auto;min-height:88px;padding:18px 22px;border-radius:18px;font-size:36px;word-break:break-all">{text}</div></div>{rows}"#
    );
    document("Address field", &body, &format!("body{{height:{KB_Y}px}}"))
}

/// Keyboard keys: rows from `keyboard_rows`, in a 16 px frame at the bottom.
pub fn keyboard(rows: &[Vec<Key>]) -> String {
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
    let extra = "body{padding:14px 12px 18px;border-top:2px solid #e2e2e2;background:#fff}\
.krow{display:flex;gap:8px;margin-bottom:8px}\
.k{display:block;height:124px;line-height:124px;text-align:center;border:1px solid #bdbdbd;border-radius:12px;background:#f7f7f7;color:#111;font-size:48px;text-decoration:none}\
.k.sm{font-size:30px}.k.on,.k.go{background:#111;color:#fff;border-color:#111}";
    document("Keyboard", &out, extra)
}

/// Keyboard layers from the layout rows (`Layer` decides which set).
pub fn keyboard_for(layer: Layer) -> String {
    keyboard(&crate::keyboard_rows(layer))
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

/// Collapsible section heading on the home screen: title, count, and a chevron. Tapping toggles it.
fn section_head(title: &str, count: usize, href: &str, open: bool) -> String {
    format!(
        r#"<a class="sechead" href="{h}"><span class="sechead-title">{t}</span><span class="sechead-count">{count}</span><span class="grow"></span>{c}</a>"#,
        h = esc(href),
        t = esc(title),
        c = icon(if open { "chev-up" } else { "chev-down" }, 32),
    )
}

/// Home screen: the address bar is the only search box. Bookmark tiles in a grid and recent pages,
/// each section with a heading that shows or hides it.
pub fn home(bookmarks: &[(String, String)], recent: &[(String, String)], bm_open: bool, recent_open: bool) -> String {
    let bm_body = if !bm_open {
        String::new()
    } else if bookmarks.is_empty() {
        r#"<div class="empty">Bookmarks you add appear here.</div>"#.to_string()
    } else {
        let tiles: String = bookmarks
            .iter()
            .map(|(u, t)| {
                let title = if t.is_empty() { u.as_str() } else { t.as_str() };
                let letter = title.chars().next().unwrap_or('•').to_uppercase().to_string();
                format!(
                    r#"<a class="tile" href="{u}"><span class="ico">{l}</span><span class="t">{t}</span></a>"#,
                    u = esc(u),
                    l = esc(&letter),
                    t = esc(title),
                )
            })
            .collect();
        format!(r#"<div class="grid">{tiles}</div>"#)
    };
    let recent_body = if !recent_open {
        String::new()
    } else if recent.is_empty() {
        r#"<div class="empty">Pages you visit appear here.</div>"#.to_string()
    } else {
        recent
            .iter()
            .take(6)
            .map(|(u, t)| {
                let label = if t.is_empty() { u.as_str() } else { t.as_str() };
                format!(
                    r#"<a class="row" href="{u}"><span class="lead">{i}</span><span class="main">{l}<span class="sub">{u}</span></span></a>"#,
                    i = icon("menu-page", 30),
                    u = esc(u),
                    l = esc(label),
                )
            })
            .collect()
    };
    let body = format!(
        r#"<div class="page">{bm_head}{bm_body}{rec_head}</div>{recent_body}
<div class="page"><div class="actions"><a class="btn" href="kbp://bookmarks">All bookmarks</a><a class="btn" href="kbp://saved">Saved pages</a></div></div>"#,
        bm_head = section_head("BOOKMARKS", bookmarks.len(), "kb:homeshow:bm", bm_open),
        rec_head = section_head("RECENT", recent.len(), "kb:homeshow:recent", recent_open),
    );
    let extra = ".sechead{display:flex;align-items:center;gap:16px;height:72px;margin-top:20px;font-size:24px;letter-spacing:1px;color:#555;text-decoration:none}\
.sechead-count{font-size:22px;color:#888}\
.grow{flex:1}";
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

/// Short notice shown above the bottom toolbar. Placed by the caller.
pub fn toast(message: &str) -> String {
    let body = format!(r#"<div class="toast">{}</div>"#, esc(message));
    document("Notice", &body, "body{height:72px}")
}
