//! Desktop driver for testing on the Mac. Reads commands from stdin and writes
//! the screen after each one to `$KB_OUT` (default `out.png`). Times every action.
//!
//!   open <url|text> | tap <x> <y> | scroll <dy> | page <+1|-1> | back | fwd | reload
//!   action <name> | quit

use std::{
    io::{self, BufRead},
    thread,
    time::{Duration, Instant},
};

use kb_app::{encode_png, Browser, Update};

const W: u32 = 1264;
const H: u32 = 1680;

fn main() -> io::Result<()> {
    let out = std::env::var("KB_OUT").unwrap_or_else(|_| "out.png".into());
    let mut b = Browser::new(W, H);
    if std::env::var("KB_PROGRESS").is_ok() {
        // Print the progress the device draws under the address bar (throttled).
        let mut last = Instant::now() - Duration::from_secs(1);
        let mut last_frac = -1.0f32;
        b.set_pump(Box::new(move |frac, label| {
            if last.elapsed() >= Duration::from_millis(100) || (frac - last_frac).abs() > 0.2 {
                eprintln!("[progress] {label} {:>3.0}%", frac * 100.0);
                last = Instant::now();
                last_frac = frac;
            }
            true
        }));
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let start = if args.is_empty() { "kbp://home".to_string() } else { args.join(" ") };
    timed("open", &mut b, &out, |b| b.open(&start));

    for line in io::stdin().lock().lines() {
        let line = line?;
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("").to_string();
        let t0 = Instant::now();
        match cmd.as_str() {
            "open" => {
                let rest: Vec<&str> = parts.collect();
                let target = rest.join(" ");
                timed("open", &mut b, &out, |b| b.open(&target));
            }
            "tap" => {
                let x: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                timed("tap", &mut b, &out, |b| b.tap(x, y));
            }
            "scroll" => {
                let dy: f64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                timed("scroll", &mut b, &out, |b| b.scroll(dy));
            }
            "page" => {
                let d: f64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                timed("page", &mut b, &out, |b| b.page_step(d));
            }
            "back" => timed("back", &mut b, &out, |b| b.back()),
            "fwd" => timed("forward", &mut b, &out, |b| b.forward()),
            "reload" => timed("reload", &mut b, &out, |b| b.reload()),
            "action" => {
                let name = parts.next().unwrap_or("").to_string();
                timed("action", &mut b, &out, |b| b.action(&name));
            }
            "press" => {
                // Show only the tap highlight, as the device does before the action runs.
                let x: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                if let Some(u) = b.press(x, y) {
                    write(&u, &out);
                    eprintln!("[press] highlight rows {:?}", u.rows);
                } else {
                    eprintln!("[press] nothing tappable there");
                }
            }
            "longpress" => {
                let x: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                timed("longpress", &mut b, &out, |b| b.long_press(x, y));
            }
            "tabstep" => {
                let dir: i32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(1);
                timed("tabstep", &mut b, &out, |b| b.tab_step(dir));
            }
            "count" => {
                // Debug: how many elements match a selector on the current page.
                let sel = parts.collect::<Vec<_>>().join(" ");
                eprintln!("[count] {sel}: {}", b.count_in_page(&sel));
            }
            "visible" => {
                let sel = parts.collect::<Vec<_>>().join(" ");
                eprintln!("[visible] {sel}: {}", b.visible_in_page(&sel));
            }
            "tapsel" => {
                let sel = parts.collect::<Vec<_>>().join(" ");
                timed("tapsel", &mut b, &out, |b| {
                    if !b.tap_selector(&sel) {
                        eprintln!("[tapsel] {sel}: not found");
                    }
                });
            }
            "longsel" => {
                let sel = parts.collect::<Vec<_>>().join(" ");
                timed("longsel", &mut b, &out, |b| {
                    if !b.long_press_selector(&sel) {
                        eprintln!("[longsel] {sel}: not found");
                    }
                });
            }
            "tapkey" => {
                let arg = parts.next().unwrap_or("").to_string();
                let c = if arg == "space" { ' ' } else { arg.chars().next().unwrap_or(' ') };
                timed("tapkey", &mut b, &out, |b| {
                    if !b.tap_key(c) {
                        eprintln!("[tapkey] {arg}: not on this keyboard layer");
                    }
                });
            }
            "linkat" => {
                let x: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let y: f32 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
                eprintln!("[linkat] ({x:.0}, {y:.0}): {:?}", b.link_at_screen(x, y));
            }
            "where" => {
                let sel = parts.collect::<Vec<_>>().join(" ");
                match b.screen_center(&sel) {
                    Some((x, y)) => eprintln!("[where] {sel}: centre ({x:.0}, {y:.0})"),
                    None => eprintln!("[where] {sel}: not on screen"),
                }
            }
            "text" => {
                let sel = parts.collect::<Vec<_>>().join(" ");
                let text = b.text_in_page(&sel).unwrap_or_else(|| "(no match)".to_string());
                eprintln!("[text] {sel}: {text}");
            }
            "assert" => {
                let kind = parts.next().unwrap_or("");
                let expected = parts.collect::<Vec<_>>().join(" ");
                let ok = match kind {
                    "url" => b.url() == expected,
                    "title" => b.title() == expected,
                    "tabs" => expected.parse::<usize>().ok() == Some(b.tab_count()),
                    "back" => expected.parse::<bool>().ok() == Some(b.can_go_back()),
                    "forward" => expected.parse::<bool>().ok() == Some(b.can_go_forward()),
                    "scroll" => {
                        let range: Vec<f64> = expected.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                        range.len() == 2 && b.scroll_pos() >= range[0] && b.scroll_pos() <= range[1]
                    }
                    "visible" => b.screen_center(&expected).is_some_and(|(x, y)| x >= 0.0 && x < W as f32 && y >= 0.0 && y < H as f32),
                    _ => false,
                };
                if !ok {
                    return Err(io::Error::other(format!(
                        "assertion failed: {kind} {expected}; url={}, title={}, scroll={}, tabs={}, back={}, forward={}",
                        b.url(), b.title(), b.scroll_pos(), b.tab_count(), b.can_go_back(), b.can_go_forward()
                    )));
                }
                eprintln!("[assert] {kind} {expected}: PASS");
            }
            "quit" => break,
            "" => continue,
            other => {
                eprintln!("unknown command: {other}");
                continue;
            }
        }
        let _ = t0;
        if b.exit_requested() {
            eprintln!("exit requested");
            break;
        }
    }
    Ok(())
}

/// Run one action, wait for held-back images, then write the screen.
fn timed(label: &str, b: &mut Browser, out: &str, act: impl FnOnce(&mut Browser)) {
    let t0 = Instant::now();
    act(b);
    let action_ms = t0.elapsed().as_millis();
    let t1 = Instant::now();
    let update = b.frame();
    let frame_ms = t1.elapsed().as_millis();
    write(&update, out);
    // Images arrive after the text: keep the screen fresh while they load.
    let deadline = Instant::now() + Duration::from_secs(8);
    // KB_KEEP_FIRST=1 keeps the frame drawn right after the action (for checking notices).
    if std::env::var("KB_KEEP_FIRST").is_ok() {
        return;
    }
    while (b.images_loading() || b.notice_showing()) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(40));
        if let Some(u) = b.tick() {
            write(&u, out);
        }
    }
    eprintln!(
        "[{label}] action {action_ms} ms, frame {frame_ms} ms | {} | {} | scroll {:.0} / {:.0}",
        b.title(),
        b.url(),
        b.scroll_pos(),
        b.content_height()
    );
}

fn write(update: &Update, out: &str) {
    let png = encode_png(W, H, &update.gray).expect("encode");
    std::fs::write(out, png).expect("write frame");
}
