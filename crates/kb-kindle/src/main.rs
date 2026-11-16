//! Kindle driver. Reads touch and page-turn buttons from evdev, drives the
//! browser, and shows each frame with the device's FBInk.
//!
//! Files (all under /mnt/us/browser):
//!   start.txt  optional start URL (default: Hacker News)
//!   /var/tmp/kb-frame.bmp  last frame, for debugging (RAM, not kept across restarts)
//!   log         written by the launcher (stderr)

use std::{
    collections::VecDeque,
    cell::RefCell,
    fs::{self, File},
    io::Read,
    os::unix::io::AsRawFd,
    panic::{self, AssertUnwindSafe},
    process::Command,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
    },
    thread,
    time::{Duration, Instant},
};

mod device;
mod profile;

use kb_app::{Browser, Update, Wave, PROGRESS_H, TOP_H};

const DIR: &str = "/mnt/us/browser";
const FBINK: &str = "/mnt/us/libkh/bin/fbink";
/// The panel's native frame: upright portrait. The touch controller always reports in this frame,
/// whichever way the Kindle is turned, so touches are turned into the screen's frame here.
const NATIVE_W: u32 = profile::ACTIVE.screen_w;
const NATIVE_H: u32 = profile::ACTIVE.screen_h;

/// The turn of the Kindle: 0 upright, 2 upside down (the left-handed portrait). The Kindle has no
/// landscape, so the browser always draws the portrait screen.
static PANEL_ROTA: AtomicU32 = AtomicU32::new(0);

/// Turn a point from the panel's native frame into the screen's frame for the turn. This is the same
/// turn the picture gets, so a finger lands on what it touches in every orientation.
fn turn_point(rota: u32, x: f32, y: f32) -> (f32, f32) {
    if rota == 2 {
        (NATIVE_W as f32 - 1.0 - x, NATIVE_H as f32 - 1.0 - y)
    } else {
        (x, y)
    }
}

/// Whether the page-turn buttons swap their directions for this turn. They are on the top and bottom
/// edges: upside down, the button on the reader's "next" side moves, so forward and back trade places.
fn page_buttons_swapped(rota: u32) -> bool {
    rota == 2
}
/// Full-screen flash every N partial refreshes, to clear e-ink ghosting.

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0x00;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;
const KEY_PAGEUP: u16 = profile::ACTIVE.page_back_key;
const KEY_PAGEDOWN: u16 = profile::ACTIVE.page_forward_key;

/// The turn the framebuffer is drawn in, from FBInk's state (`fbink -e` prints `currentRota=N`): 0 upright,
/// 2 upside down. The screen's own direction, so the controls follow what the reader sees, not the
/// physical tilt. Landscape (1 and 3) is not supported and gives no turn.
fn turn_from_fbink(state: &str) -> Option<u32> {
    let value = state
        .split([';', '\n'])
        .find_map(|pair| pair.trim().strip_prefix("currentRota="))?
        .trim()
        .parse::<u32>()
        .ok()?;
    match value {
        0 => Some(0),
        2 => Some(2),
        _ => None,
    }
}

/// The turn the framework shows: its accelerometer property (`U` upright, `D` upside down, `R` and `L`
/// landscape, which is not supported and gives no turn). This is the orientation the reader sees.
fn turn_from_framework(value: &str) -> Option<u32> {
    match value.trim() {
        "U" => Some(0),
        "D" => Some(2),
        _ => None,
    }
}

fn framework_turn() -> Option<u32> {
    let out = Command::new("lipc-get-prop").args(["com.lab126.winmgr", "accelerometer"]).output().ok()?;
    turn_from_framework(&String::from_utf8_lossy(&out.stdout))
}

/// FBInk's framebuffer rotation, kept beside the framework's value in the log.
fn framebuffer_turn() -> Option<u32> {
    let out = Command::new(FBINK).arg("-e").output().ok()?;
    turn_from_fbink(&String::from_utf8_lossy(&out.stdout))
}

/// The turn now: the framework's, else the framebuffer's. Both are returned for the log.
fn display_turn() -> (Option<u32>, Option<u32>) {
    (framework_turn(), framebuffer_turn())
}

/// Follow the screen's turn: once a second, and each change sets the turn that touch and the page
/// buttons use. The log records both sources, so a disagreement with the text on screen shows.
fn display_turn_watch() {
    loop {
        thread::sleep(Duration::from_secs(1));
        let (framework, framebuffer) = display_turn();
        let Some(turn) = framework.or(framebuffer) else { continue };
        if PANEL_ROTA.swap(turn, Ordering::Relaxed) != turn {
            log(&format!("display turned: rota {turn} (framework {framework:?}, framebuffer {framebuffer:?})"));
        }
    }
}

/// `_IOR('E', 0x40 + code, struct input_absinfo)` for the 24-byte absinfo.
const fn eviocgabs(code: u16) -> libc::c_ulong {
    (0x8000_0000u32 | (24 << 16) | (0x45 << 8) | (0x40 + code as u32)) as libc::c_ulong
}

#[repr(C)]
#[derive(Default)]
struct AbsInfo {
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

/// Matches the kernel's `struct input_event`. The Kindle kernel is 32-bit, so
/// its timeval fields are 32-bit regardless of what the libc crate's time_t is.
#[repr(C)]
struct InputEvent {
    tv_sec: i32,
    tv_usec: i32,
    kind: u16,
    code: u16,
    value: i32,
}

#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<InputEvent>() == 16);

#[derive(Debug)]
enum Ev {
    /// A swipe down that starts at the top edge: quick settings.
    QuickSettings,
    Tap(f32, f32),
    LongTap(f32, f32),
    TabStep(i32),
    Scroll(f64),
    Page(f64),
    Back,
    Forward,
}

/// A touch that starts this close to the top edge (device pixels) is an edge swipe.
const EDGE_H: f32 = 40.0;

fn log(msg: &str) {
    eprintln!("[kb] {msg}");
}

/// Find an input device by its kernel name (e.g. "cyttsp5_mt", "gpio-keys").
fn find_input(name_hint: &str) -> Option<String> {
    for entry in fs::read_dir("/sys/class/input").ok()?.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.starts_with("event") {
            continue;
        }
        let name = fs::read_to_string(entry.path().join("device/name")).unwrap_or_default();
        if name.trim().contains(name_hint) {
            return Some(format!("/dev/input/{fname}"));
        }
    }
    None
}

fn abs_range(file: &File, code: u16) -> Option<AbsInfo> {
    let mut info = AbsInfo::default();
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), eviocgabs(code) as _, &mut info as *mut AbsInfo) };
    (rc == 0).then_some(info)
}

fn scale(raw: i32, info: &AbsInfo, out: u32) -> f32 {
    let span = (info.maximum - info.minimum).max(1) as f32;
    ((raw - info.minimum) as f32 / span * (out as f32 - 1.0)).clamp(0.0, out as f32 - 1.0)
}

/// Touch reader: taps, vertical drags (scroll), horizontal swipes (back/forward).
fn touch_loop(path: String, tx: Sender<Ev>) {
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => return log(&format!("cannot open {path}: {e}")),
    };
    let rx_info = abs_range(&file, ABS_MT_POSITION_X);
    let ry_info = abs_range(&file, ABS_MT_POSITION_Y);
    log(&format!("touch {path}: x={:?} y={:?}", rx_info.as_ref().map(|i| (i.minimum, i.maximum)), ry_info.as_ref().map(|i| (i.minimum, i.maximum))));
    let (Some(xi), Some(yi)) = (rx_info, ry_info) else {
        return log("touch ranges unavailable");
    };

    let (mut raw_x, mut raw_y) = (0i32, 0i32);
    let mut active = false;
    let mut start: Option<(f32, f32)> = None;
    let mut last: (f32, f32) = (0.0, 0.0);
    let mut down_at = Instant::now();

    loop {
        let ev = match read_event(&mut file) {
            Some(e) => e,
            None => return log("touch read failed"),
        };
        match (ev.kind, ev.code) {
            (EV_ABS, ABS_MT_TRACKING_ID) => {
                if ev.value >= 0 {
                    active = true;
                    start = None;
                    down_at = Instant::now();

                } else if active {
                    active = false;
                    let end = last;
                    if let Some((sx, sy)) = start {
                        let (dx, dy) = (end.0 - sx, end.1 - sy);
                        let held = down_at.elapsed();
                        // A finger never lands perfectly still: allow a generous slop before
                        // a touch counts as a swipe. Taps and long presses are told apart by
                        // how long the finger stayed down.
                        let still = dx.abs() < 40.0 && dy.abs() < 40.0;
                        let on_bar = sy < TOP_H as f32;
                        let ev = if still && held < Duration::from_millis(800) {
                            Some(Ev::Tap(end.0, end.1))
                        } else if still {
                            Some(Ev::LongTap(end.0, end.1))
                        } else if sy < EDGE_H && dy > 120.0 && dy.abs() > dx.abs() * 1.5 {
                            // A swipe down from the very top edge opens quick settings.
                            Some(Ev::QuickSettings)
                        } else if on_bar && dx.abs() > 180.0 && dx.abs() > dy.abs() * 1.5 {
                            // Swipe along the address bar moves between tabs.
                            Some(Ev::TabStep(if dx > 0.0 { -1 } else { 1 }))
                        } else if dy.abs() > dx.abs() * 1.2 && dy.abs() > 80.0 {
                            Some(Ev::Scroll(-(dy as f64)))
                        } else if dx.abs() > dy.abs() * 1.5 && dx.abs() > 180.0 {
                            Some(if dx > 0.0 { Ev::Back } else { Ev::Forward })
                        } else {
                            None
                        };
                        if let Some(e) = ev {
                            let _ = tx.send(e);
                        }
                    }
                }
            }
            (EV_ABS, ABS_MT_POSITION_X) => raw_x = ev.value,
            (EV_ABS, ABS_MT_POSITION_Y) => raw_y = ev.value,
            (EV_SYN, SYN_REPORT) => {
                if active {
                    let rota = PANEL_ROTA.load(Ordering::Relaxed);
                    let (nx, ny) = (scale(raw_x, &xi, NATIVE_W), scale(raw_y, &yi, NATIVE_H));
                    last = turn_point(rota, nx, ny);
                    if start.is_none() {
                        start = Some(last);
                        log(&format!(
                            "touch down raw=({raw_x},{raw_y}) native=({nx:.0},{ny:.0}) rota={rota} screen=({:.0},{:.0})",
                            last.0, last.1
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Page-turn buttons (gpio-keys). Press only; repeats ignored.
fn button_loop(path: String, tx: Sender<Ev>) {
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => return log(&format!("cannot open {path}: {e}")),
    };
    log(&format!("buttons {path} open"));
    loop {
        let Some(ev) = read_event(&mut file) else {
            return log("button read failed");
        };
        if ev.kind == EV_KEY {
            log(&format!("key code={} value={}", ev.code, ev.value));
            if ev.value == 1 {
                let swapped = page_buttons_swapped(PANEL_ROTA.load(Ordering::Relaxed));
                let forward = if swapped { -1.0 } else { 1.0 };
                let out = match ev.code {
                    KEY_PAGEDOWN => Some(Ev::Page(forward)),
                    KEY_PAGEUP => Some(Ev::Page(-forward)),
                    _ => None,
                };
                if let Some(e) = out {
                    let _ = tx.send(e);
                }
            }
        }
    }
}

fn read_event(file: &mut File) -> Option<InputEvent> {
    let mut buf = [0u8; std::mem::size_of::<InputEvent>()];
    file.read_exact(&mut buf).ok()?;
    // Safe: InputEvent is repr(C) plain data and buf has the same size.
    Some(unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const InputEvent) })
}

/// Drawing to the panel. Regions are cropped to the rows that changed, and the
/// waveform follows the kind of change (fast for chrome, text quality for pages).
struct Display {
    last_progress: Instant,
    /// The e-ink waveforms chosen in the browser's Display sheet.
    page_wave: String,
    ui_wave: String,
}

impl Display {
    fn show(&mut self, u: &Update) {
        let (y0, y1) = u.rows;
        if y1 <= y0 {
            return;
        }
        let flash = u.wave == Wave::Flash;
        // A full flash is always GC16. Otherwise the reader's own choice: page content and bars/taps.
        let wave = if flash {
            "GC16"
        } else {
            match u.wave {
                Wave::Fast => self.ui_wave.as_str(),
                _ => self.page_wave.as_str(),
            }
        };
        let region = u.region();
        blit_region(u.width, y0, y1 - y0, &region, wave, flash);
    }

    /// Progress strip under the address bar. Cheap, and at most ten times a second.
    fn progress(&mut self, fraction: f32) {
        if self.last_progress.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.last_progress = Instant::now();
        let w = NATIVE_W;
        let mut strip = vec![200u8; (w * PROGRESS_H) as usize];
        let fill = (fraction.clamp(0.0, 1.0) * w as f32) as usize;
        for row in 0..PROGRESS_H as usize {
            for x in 0..fill {
                strip[row * w as usize + x] = 0;
            }
        }
        blit_region(w, TOP_H - PROGRESS_H, PROGRESS_H, &strip, "DU", false);
    }
}

/// The frame file fbink reads. It lives in RAM (/var is tmpfs), so a page turn does not write to the
/// storage partition. An 8-bit grayscale BMP needs no compression, which makes it much faster to produce
/// than a PNG; fbink reads it directly.
const FRAME_FILE: &str = "/var/tmp/kb-frame.bmp";

/// Show `rows` rows of gray pixels at row `y` with the given fbink waveform.
fn blit_region(width: u32, y: u32, rows: u32, gray: &[u8], wave: &str, flash: bool) {
    let bmp = gray_bmp(width, rows, gray);
    let path = FRAME_FILE;
    if let Err(e) = fs::write(path, bmp) {
        return log(&format!("write frame: {e}"));
    }
    let mut cmd = Command::new(FBINK);
    cmd.arg("-g").arg(format!("file={path},x=0,y={y}"));
    cmd.args(["-W", wave]);
    if flash {
        cmd.arg("-f");
    }
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => log(&format!("fbink exit {s}")),
        Err(e) => log(&format!("fbink: {e}")),
    }
}

/// An 8-bit grayscale BMP of `width` x `rows` pixels. `gray` holds the rows top to bottom, one byte per pixel.
/// The rows are stored bottom-up, as BMP files usually are, with each row padded to four bytes.
fn gray_bmp(width: u32, rows: u32, gray: &[u8]) -> Vec<u8> {
    let stride = (width as usize + 3) & !3;
    let pixels = stride * rows as usize;
    let palette = 256 * 4;
    let offset = 14 + 40 + palette;
    let size = offset + pixels;
    let mut out = Vec::with_capacity(size);
    // File header.
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(offset as u32).to_le_bytes());
    // Info header (BITMAPINFOHEADER).
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(rows as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]); // no compression, sizes and resolution left at zero
    // Grey palette.
    for v in 0..=255u8 {
        out.extend_from_slice(&[v, v, v, 0]);
    }
    // Pixels, bottom row first.
    let row_len = width as usize;
    let padding = [0u8; 3];
    for row in (0..rows as usize).rev() {
        let start = row * row_len;
        let line = gray.get(start..start + row_len).unwrap_or(&[]);
        out.extend_from_slice(line);
        out.extend_from_slice(&padding[..stride - line.len()]);
    }
    out
}

/// State shared by the input loop and the browser's progress pump.
struct Shared {
    rx: Receiver<Ev>,
    pending: VecDeque<Ev>,
    display: Display,
}

/// Resident memory of this process in kB, from /proc (0 where /proc is absent).
fn rss_kb() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(0)
}

/// Writes a fatal signal's number and fault address to stderr (the browser log) before the process ends.
/// Only `write(2)` is used here, which is safe inside a signal handler.
extern "C" fn on_fatal(sig: libc::c_int, info: *mut libc::siginfo_t, _ctx: *mut libc::c_void) {
    fn put(bytes: &[u8]) {
        unsafe { libc::write(2, bytes.as_ptr() as *const libc::c_void, bytes.len()) };
    }
    fn hex(mut value: usize, out: &mut [u8; 18]) -> &[u8] {
        let digits = b"0123456789abcdef";
        let mut at = out.len();
        loop {
            at -= 1;
            out[at] = digits[value & 15];
            value >>= 4;
            if value == 0 {
                break;
            }
        }
        out[at - 2] = b'0';
        out[at - 1] = b'x';
        &out[at - 2..]
    }
    let addr = if info.is_null() { 0 } else { unsafe { (*info).si_addr() as usize } };
    let mut buf = [0u8; 18];
    put(b"\n[crash] fatal signal ");
    put(&[b'0' + (sig as u8 / 10), b'0' + (sig as u8 % 10)]);
    put(b" at address ");
    put(hex(addr, &mut buf));
    put(b"\n");
    // Restore the default action and raise it again, so the process ends as it would have.
    unsafe {
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

fn install_crash_log() {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_fatal as *const () as usize;
        action.sa_flags = libc::SA_SIGINFO;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGBUS, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGSEGV, &action, std::ptr::null_mut());
    }
}

fn main() {
    install_crash_log();
    log("start");
    let start = fs::read_to_string(format!("{DIR}/start.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "kbp://home".into());

    let (tx, rx) = channel();
    if let Some(path) = find_input(profile::ACTIVE.touch_name) {
        let t = tx.clone();
        thread::spawn(move || touch_loop(path, t));
    } else {
        log("no touch device found");
    }
    if let Some(path) = find_input("gpio-keys") {
        let t = tx.clone();
        thread::spawn(move || button_loop(path, t));
    } else {
        log("no page-button device found");
    }

    let shared = Rc::new(RefCell::new(Shared {
        rx,
        pending: VecDeque::new(),
        display: Display { last_progress: Instant::now() - Duration::from_secs(1), page_wave: "GC16".into(), ui_wave: "DU".into() },
    }));

    // While a page loads, the progress strip is drawn and any tap cancels the wait,
    // so the input is handled right away instead of after the page finishes.
    let pump_state = shared.clone();
    let (framework, framebuffer) = display_turn();
    let turn = framework.or(framebuffer).unwrap_or(0);
    PANEL_ROTA.store(turn, Ordering::Relaxed);
    log(&format!("turn at start: rota {turn} (framework {framework:?}, framebuffer {framebuffer:?})"));
    thread::spawn(display_turn_watch);
    let mut b = Browser::new(NATIVE_W, NATIVE_H);
    b.set_pump(Box::new(move |fraction, label| {
        let mut s = pump_state.borrow_mut();
        let mut interrupted = false;
        while let Ok(ev) = s.rx.try_recv() {
            s.pending.push_back(ev);
            // While the document itself is still downloading, input waits for it.
            // Only the resource phase is cut short by a tap.
            interrupted = label != "Connecting";
        }
        if interrupted {
            return false;
        }
        s.display.progress(fraction);
        true
    }));

    // A navigation paints the address bar at once (the browser calls this before fetching).
    let draw_state = shared.clone();
    b.set_draw(Box::new(move |u| draw_state.borrow_mut().display.show(&u)));

    // The Kindle's own frontlight, warmth, battery and Wi-Fi for the quick settings panel.
    b.set_device(Box::new(device::KindleDevice));

    let t0 = Instant::now();
    // Resumes the tabs saved when the browser last left; `start` only when there are none.
    b.resume(&start);
    let u = b.frame();
    shared.borrow_mut().display.show(&u);
    log(&format!("ready in {:?}: {}", t0.elapsed(), b.url()));

    loop {
        // The waveforms the reader chose in its Display sheet (they can change between events).
        {
            let (page, ui) = b.wave_names();
            let mut s = shared.borrow_mut();
            s.display.page_wave = page;
            s.display.ui_wave = ui;
        }
        let next = shared.borrow_mut().pending.pop_front();
        let ev = match next {
            Some(ev) => ev,
            None => {
                let received = shared.borrow().rx.recv_timeout(Duration::from_millis(400));
                match received {
                    Ok(ev) => ev,
                    Err(RecvTimeoutError::Timeout) => {
                        // Idle: bring in finished images and clear notices whose time is up.
                        if let Some(u) = b.tick() {
                            shared.borrow_mut().display.show(&u);
                        }
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        };
        let t_action = Instant::now();
        log(&format!("event {ev:?}"));
        // Tap feedback first: the element turns black before the action runs.
        if let Ev::Tap(x, y) | Ev::LongTap(x, y) = ev {
            if let Some(u) = b.press(x, y) {
                shared.borrow_mut().display.show(&u);
            }
        }
        // A panic in page code must not take the whole browser down.
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            match ev {
                Ev::Tap(x, y) => b.tap(x, y),
                Ev::LongTap(x, y) => b.long_press(x, y),
                Ev::QuickSettings => b.quick_settings(),
                Ev::TabStep(dir) => b.tab_step(dir),
                Ev::Scroll(dy) => b.scroll(dy),
                Ev::Page(d) => b.page_step(d),
                Ev::Back => b.back(),
                Ev::Forward => b.forward(),
            }
            let action_ms = t_action.elapsed();
            let t_frame = Instant::now();
            let update = b.frame();
            log(&format!(
                "action {action_ms:?}, frame {:?}, rows {:?}, wave {:?}, rss {} kB",
                t_frame.elapsed(),
                update.rows,
                update.wave,
                rss_kb()
            ));
            update
        }));
        match outcome {
            Ok(update) => shared.borrow_mut().display.show(&update),
            Err(_) => log("recovered from a panic while handling an event"),
        }
        if b.exit_requested() {
            log("exit requested");
            // A plain clear, no flash: the framework draws the library right after, which saves the flash time.
            let _ = Command::new(FBINK).args(["-c"]).status();
            break;
        }
    }
    // Shutdown in steps, so a crash in the last one shows which step it was.
    log("closing: dropping the browser");
    drop(b);
    log("closing: browser dropped");
    drop(shared);
    log("bye");
}

#[cfg(test)]
mod bmp_tests {
    use super::*;

    #[test]
    fn gray_bmp_has_the_right_size_and_rows_bottom_up() {
        // 3 pixels wide: each row is padded to 4 bytes. Row 0 (top) is 1 2 3, row 1 is 4 5 6.
        let bmp = gray_bmp(3, 2, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(&bmp[0..2], b"BM");
        let offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap()) as usize;
        assert_eq!(offset, 14 + 40 + 1024);
        assert_eq!(bmp.len(), offset + 8);
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()) as usize, bmp.len());
        assert_eq!(&bmp[offset..offset + 4], &[4, 5, 6, 0]);
        assert_eq!(&bmp[offset + 4..offset + 8], &[1, 2, 3, 0]);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upside_down_turns_the_touch_frame_and_upright_does_not() {
        let (w, h) = (NATIVE_W as f32 - 1.0, NATIVE_H as f32 - 1.0);
        assert_eq!(turn_point(0, 0.0, 0.0), (0.0, 0.0));
        // Left-handed portrait: the top-left of the panel is the bottom-right of the screen.
        assert_eq!(turn_point(2, 0.0, 0.0), (w, h));
        assert_eq!(turn_point(2, w, h), (0.0, 0.0));
    }

    #[test]
    fn page_buttons_trade_places_only_upside_down() {
        assert!(!page_buttons_swapped(0));
        assert!(page_buttons_swapped(2));
    }

    #[test]
    fn the_framework_turn_is_upright_or_upside_down_and_landscape_gives_none() {
        assert_eq!(turn_from_framework("U\n"), Some(0));
        assert_eq!(turn_from_framework("D"), Some(2));
        assert_eq!(turn_from_framework("R"), None);
        assert_eq!(turn_from_framework("L"), None);
    }

    #[test]
    fn the_framebuffer_turn_is_upright_or_upside_down_and_landscape_gives_none() {
        assert_eq!(turn_from_fbink("FBINK_VERSION='1.x';viewWidth=1264;currentRota=0;viewHeight=1680"), Some(0));
        assert_eq!(turn_from_fbink("viewWidth=1264;currentRota=2;viewHeight=1680\n"), Some(2));
        assert_eq!(turn_from_fbink("currentRota=1;viewWidth=1680"), None);
        assert_eq!(turn_from_fbink("currentRota=3"), None);
        assert_eq!(turn_from_fbink("viewWidth=1264"), None);
    }
}
