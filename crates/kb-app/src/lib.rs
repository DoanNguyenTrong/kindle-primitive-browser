//! Browser state and UI: navigation, history, bookmarks, saved pages, reader
//! mode, and the browser's own interface (address bar, bottom toolbar, menu,
//! QWERTY keyboard with caret, home screen). The chrome is HTML rendered by the
//! same engine as web pages, and every rendered layer is cached so that a tap
//! only re-renders what changed.
//!
//! Link kinds: `kb:<action>` runs a browser action; `kbp://…` is an internal
//! page; http(s) is a normal navigation.

use std::{
    fs,
    panic::AssertUnwindSafe,
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

mod ui;

use kb_engine::Page;
use kb_net::{store, Policy, SyncNet};

/// Address bar height (device pixels, 1264x1680 panel).
pub const TOP_H: u32 = 96;
/// Bottom toolbar height. Sits in thumb reach.
pub const BOT_H: u32 = 104;
/// Keyboard top edge in keyboard mode: the keys sit at the bottom of the screen.
/// The field and suggestions take the space above.
pub const KB_Y: u32 = 950;
/// Progress strip height.
pub const PROGRESS_H: u32 = 8;
/// Menu sheet height.
pub const MENU_H: u32 = 780;

/// Largest main document we accept.
const MAX_DOC_BYTES: u64 = 6 * 1024 * 1024;
/// Largest sub-resource (CSS, script, image). Raised for better page fidelity.
const SUBRESOURCE_MAX_BYTES: u64 = 4 * 1024 * 1024;
/// Seeded bookmarks on first run.
const DEFAULT_BOOKMARKS: &[(&str, &str)] = &[(
    "https://truyendich.space/doc-truyen/pham-nhan-tien-duyen",
    "Phàm Nhân Tiên Duyên",
)];
const HISTORY_CAP: usize = 300;
/// Page widths the reader can pick, in CSS pixels. Narrow is a phone layout; medium suits this screen.
pub const PAGE_WIDTHS: [u32; 3] = [412, 600, 800];
const DEFAULT_PAGE_WIDTH: u32 = 600;
/// Text sizes the reader steps through, in percent.
const TEXT_STEPS: [u32; 8] = [80, 90, 100, 110, 125, 150, 175, 200];

/// Progress callback during a load: `(fraction 0..1, label)`. Return `false` to
/// cancel the wait (used when the user taps while the page is still loading).
pub type Pump = Box<dyn FnMut(f32, &str) -> bool>;

/// How a region should be refreshed on the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Wave {
    /// Fast, low-detail (DU). Bars, keyboard, progress.
    Fast,
    /// Text-quality (GL16). Page content.
    Text,
    /// Full flash (GC16). Clears ghosting.
    Flash,
}

/// One frame plus what changed. `rows` is `[start, end)`; empty means nothing to draw.
#[derive(Clone, Debug)]
pub struct Update {
    pub gray: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub rows: (u32, u32),
    pub wave: Wave,
}

impl Update {
    /// Rows `[start, end)` as a standalone gray buffer.
    pub fn region(&self) -> Vec<u8> {
        let (a, b) = self.rows;
        let w = self.width as usize;
        self.gray[a as usize * w..b as usize * w].to_vec()
    }
}

#[derive(Debug)]
pub struct Cancelled;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Lower,
    Upper,
    Symbols,
}

/// A rendered chrome layer: its page (for hit testing) and its pixels.
struct Cached {
    key: String,
    page: Page,
    gray: Vec<u8>,
}

pub struct Browser {
    width: u32,
    height: u32,
    net: Arc<SyncNet>,
    pump: Option<Pump>,

    history: Vec<String>,
    scrolls: Vec<f64>,
    idx: usize,
    page: Option<Page>,
    page_gray: Vec<u8>,
    /// Vertical offset the cached page image was painted with.
    page_gray_offset: u32,
    page_dirty: bool,
    source: String,
    url: String,
    reader: bool,
    chrome: bool,
    sheet: Option<Sheet>,
    /// Tabs other than the active one (the active slot holds an empty placeholder).
    tabs: Vec<TabSave>,
    active: usize,
    /// Key of the text field being edited from a page, when the keyboard is in input mode.
    input_target: Option<u64>,
    settings: Settings,
    /// The page left most recently, kept live so Back is instant. Capped at one: memory stays at two pages.
    previous: Option<(String, Page)>,
    /// URL of the page on screen (to tell it apart from the previous one).
    page_url: String,
    toast: Option<(String, Instant)>,
    toast_cache: Option<(String, Vec<u8>)>,

    keyboard: bool,
    typed: Vec<char>,
    caret: usize,
    layer: Layer,
    suggestions: Vec<(String, String)>,
    field: Option<Cached>,
    keys: [Option<Cached>; 3],

    top: Option<Cached>,
    bottom: Option<Cached>,
    sheet_layer: Option<Cached>,

    exit: bool,
    navigating_history: bool,
    bookmarks: Vec<(String, String)>,
    visits: Vec<(String, String)>,
    /// Pending screen change: union of changed rows and the strongest waveform.
    change: Option<((u32, u32), Wave)>,
    images_seen: usize,
    last_image_refresh: Instant,
    /// Last composed screen, used to draw the tap highlight without re-rendering.
    last_gray: Vec<u8>,
    /// Rows of the highlight shown by `press`, to be restored on the next frame.
    pressed_rows: Option<(u32, u32)>,
    /// Receives a frame as soon as a navigation starts, so the address bar shows the new URL while the page loads.
    draw: Option<Box<dyn FnMut(Update)>>,
}

impl Browser {
    pub fn new(width: u32, height: u32) -> Browser {
        ensure_defaults();
        Browser {
            width,
            height,
            net: Arc::new(SyncNet::new(SUBRESOURCE_MAX_BYTES)),
            pump: None,
            history: Vec::new(),
            scrolls: Vec::new(),
            idx: 0,
            page: None,
            page_gray: Vec::new(),
            page_gray_offset: u32::MAX,
            page_dirty: true,
            source: String::new(),
            url: String::new(),
            reader: false,
            chrome: true,
            sheet: None,
            tabs: vec![TabSave::empty()],
            active: 0,
            input_target: None,
            settings: Settings::load(),
            previous: None,
            page_url: String::new(),
            toast: None,
            toast_cache: None,
            keyboard: false,
            typed: Vec::new(),
            caret: 0,
            layer: Layer::Lower,
            suggestions: Vec::new(),
            field: None,
            keys: [None, None, None],
            top: None,
            bottom: None,
            sheet_layer: None,
            exit: false,
            navigating_history: false,
            bookmarks: read_pairs(&data_file("bookmarks.tsv")),
            visits: read_pairs(&data_file("history.tsv")),
            change: None,
            images_seen: 0,
            last_image_refresh: Instant::now(),
            last_gray: Vec::new(),
            pressed_rows: None,
            draw: None,
        }
    }

    /// Install the sink that shows a frame during a load (the driver puts it on the panel).
    pub fn set_draw(&mut self, draw: Box<dyn FnMut(Update)>) {
        self.draw = Some(draw);
    }

    /// Elements matching `selector` on the page on screen (debugging and tests).
    pub fn count_in_page(&self, selector: &str) -> usize {
        self.page.as_ref().map_or(0, |p| p.count(selector))
    }

    /// Centre of the first displayed element matching `selector` on the current screen,
    /// in screen pixels: keyboard keys and the field while the keyboard is open, a sheet
    /// while one is shown, otherwise the page. `None` if nothing matches.
    pub fn screen_center(&mut self, selector: &str) -> Option<(f32, f32)> {
        let centre = |(x, y, w, h): (f32, f32, f32, f32)| (x + w / 2.0, y + h / 2.0);
        if self.keyboard {
            if let Some(b) = self.ensure_field().page.element_box(selector) {
                return Some(centre(b));
            }
            let (x, y) = centre(self.ensure_key(self.layer).page.element_box(selector)?);
            return Some((x, y + KB_Y as f32));
        }
        if let Some(sheet) = self.sheet {
            let (x, y) = centre(self.ensure_sheet(sheet).page.element_box(selector)?);
            return Some((x, y + (self.height - MENU_H) as f32));
        }
        let (x, y) = centre(self.page.as_ref()?.element_box(selector)?);
        Some((x, y + self.page_offset() as f32))
    }

    /// Scroll the page so the first element matching `selector` sits in the middle of the
    /// visible area, as a reader would before tapping it. No-op for keyboard and sheets.
    pub fn reveal(&mut self, selector: &str) {
        if self.keyboard || self.sheet.is_some() {
            return;
        }
        let Some((_, y, _, h)) = self.page.as_ref().and_then(|p| p.element_box(selector)) else { return };
        let off = self.page_offset() as f32;
        let visible = (self.height - BOT_H) as f32 - off;
        let centre_on_screen = y + h / 2.0 + off;
        if centre_on_screen >= off && centre_on_screen <= (self.height - BOT_H) as f32 {
            return;
        }
        if let Some(page) = self.page.as_mut() {
            let doc_y = y + page.scroll_y() as f32;
            let target = (doc_y + h / 2.0 - visible / 2.0).max(0.0) as f64;
            let delta = target - page.scroll_y();
            page.scroll_by(delta);
            self.page_dirty = true;
            self.touch(0, self.height, Wave::Text);
        }
    }

    /// Tap the centre of the first element matching `selector`, scrolling it into view first.
    /// Returns false if no displayed element matches.
    pub fn tap_selector(&mut self, selector: &str) -> bool {
        self.reveal(selector);
        match self.screen_center(selector) {
            Some((x, y)) => {
                self.tap(x, y);
                true
            }
            None => false,
        }
    }

    /// Long press the centre of the first element matching `selector`.
    pub fn long_press_selector(&mut self, selector: &str) -> bool {
        self.reveal(selector);
        match self.screen_center(selector) {
            Some((x, y)) => {
                self.long_press(x, y);
                true
            }
            None => false,
        }
    }

    /// Tap a key of the on-screen keyboard by its character (`' '` for space).
    pub fn tap_key(&mut self, c: char) -> bool {
        let selector = format!("a[href=\"kb:c:{}\"]", c as u32);
        self.tap_selector(&selector)
    }

    /// Debug: what the page resolves at a screen point (link href, if any).
    pub fn link_at_screen(&self, x: f32, y: f32) -> Option<String> {
        let off = self.page_offset() as f32;
        self.page.as_ref()?.link_at(x, y - off)
    }

    /// Text of the first element matching `selector` on the page on screen (tests and debugging).
    pub fn text_in_page(&self, selector: &str) -> Option<String> {
        self.page.as_ref()?.text_of(selector)
    }

    /// Displayed elements matching `selector` on the page on screen (debugging and tests).
    pub fn visible_in_page(&self, selector: &str) -> usize {
        self.page.as_ref().map_or(0, |p| p.visible_count(selector))
    }

    /// Install the progress/cancel callback used while pages load.
    pub fn set_pump(&mut self, pump: Pump) {
        self.pump = Some(pump);
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn can_go_back(&self) -> bool {
        self.idx > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.idx + 1 < self.history.len()
    }

    pub fn exit_requested(&self) -> bool {
        self.exit
    }

    pub fn title(&self) -> &str {
        self.page.as_ref().map_or("", |p| p.title.as_str())
    }

    pub fn content_height(&self) -> f64 {
        self.page.as_ref().map_or(0.0, |p| p.content_height())
    }

    pub fn scroll_pos(&self) -> f64 {
        self.page.as_ref().map_or(0.0, |p| p.scroll_y())
    }

    /// Images still being fetched in the background.
    pub fn images_loading(&self) -> bool {
        let s = self.net.stats();
        s.deferred.load(std::sync::atomic::Ordering::Relaxed)
            > s.deferred_done.load(std::sync::atomic::Ordering::Relaxed)
    }

    // ---- navigation -------------------------------------------------------

    /// Navigate to a user-typed address, search, or internal page.
    pub fn open(&mut self, input: &str) {
        let target = normalize(input);
        self.remember_scroll();
        let prev = self.idx;
        let had_history = !self.history.is_empty();
        self.history.truncate(self.idx + usize::from(had_history));
        self.scrolls.truncate(self.history.len());
        self.history.push(target.clone());
        self.scrolls.push(0.0);
        self.idx = self.history.len() - 1;
        if self.load(&target, Policy::Network).is_err() {
            // Cancelled by the user before it arrived: undo the history entry.
            self.history.pop();
            self.scrolls.pop();
            self.idx = prev;
        }
    }

    pub fn back(&mut self) {
        if self.can_go_back() {
            self.remember_scroll();
            let prev = self.idx;
            self.idx -= 1;
            self.load_history_entry(prev);
        }
    }

    pub fn forward(&mut self) {
        if self.can_go_forward() {
            self.remember_scroll();
            let prev = self.idx;
            self.idx += 1;
            self.load_history_entry(prev);
        }
    }

    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            let url = self.url.clone();
            let _ = self.load(&url, Policy::Network);
        }
    }

    fn load_history_entry(&mut self, prev: usize) {
        let url = self.history[self.idx].clone();
        let saved = self.scrolls.get(self.idx).copied().unwrap_or(0.0);
        if self.restore_previous(&url) {
            if let Some(page) = self.page.as_mut() {
                page.scroll_by(saved);
            }
            return;
        }
        self.navigating_history = true;
        let outcome = self.load(&url, Policy::Cache(Duration::from_secs(60 * 60 * 24 * 7)));
        self.navigating_history = false;
        match outcome {
            Ok(()) => {
                if let Some(page) = self.page.as_mut() {
                    page.scroll_by(saved);
                }
            }
            Err(Cancelled) => self.idx = prev,
        }
    }

    /// Carry out a submitted form: GET navigates to the query URL, POST sends the fields.
    fn submit(&mut self, submit: kb_engine::Submit) {
        match submit {
            kb_engine::Submit::Get(url) => self.open(&url),
            kb_engine::Submit::Post { url, fields } => {
                self.remember_scroll();
                let prev = self.idx;
                let had_history = !self.history.is_empty();
                self.history.truncate(self.idx + usize::from(had_history));
                self.scrolls.truncate(self.history.len());
                self.history.push(url.clone());
                self.scrolls.push(0.0);
                self.idx = self.history.len() - 1;
                if self.post_document(&url, fields).is_err() {
                    self.history.pop();
                    self.scrolls.pop();
                    self.idx = prev;
                }
            }
        }
    }

    /// POST a form and show the response, with the same progress and cancel behaviour as a GET.
    fn post_document(&mut self, url: &str, fields: Vec<(String, String)>) -> Result<(), Cancelled> {
        self.url = url.to_string();
        self.keyboard = false;
        self.sheet = None;
        self.chrome = true;
        self.reader = false;
        self.page_dirty = true;
        let u = url.to_string();
        let outcome = self.run_job(move || kb_net::post_form(&u, &fields, MAX_DOC_BYTES))?;
        match outcome {
            Ok(f) => {
                self.url = f.url.clone();
                if let Some(last) = self.history.get_mut(self.idx) {
                    *last = f.url.clone();
                }
                let html = String::from_utf8_lossy(&f.body).into_owned();
                self.source = html.clone();
                self.build_page(&html, &f.url, true);
            }
            Err(e) => {
                eprintln!("[post] {url}: {e}");
                let html = ui::error("Could not load this page", &load_reason(&e), &e);
                self.source = html.clone();
                self.build_page(&html, url, false);
            }
        }
        self.touch(0, self.height, Wave::Text);
        Ok(())
    }

    /// Move the page on screen into the one-page cache.
    fn stash_current(&mut self) {
        if let Some(page) = self.page.take() {
            if !self.page_url.is_empty() && self.page_url != self.url {
                self.previous = Some((self.page_url.clone(), page));
            }
        }
        self.page_url = self.url.clone();
    }

    /// Restore the cached previous page, if it is the one we want. Returns true when used.
    fn restore_previous(&mut self, url: &str) -> bool {
        let Some((prev_url, _)) = &self.previous else { return false };
        if prev_url != url {
            return false;
        }
        let Some((_, page)) = self.previous.take() else { return false };
        self.stash_current();
        self.page = Some(page);
        self.page_url = url.to_string();
        self.url = url.to_string();
        self.keyboard = false;
        self.sheet = None;
        self.chrome = true;
        self.reader = false;
        self.page_dirty = true;
        self.touch(0, self.height, Wave::Text);
        true
    }

    fn remember_scroll(&mut self) {
        let pos = self.page.as_ref().map_or(0.0, |p| p.scroll_y());
        if let Some(slot) = self.scrolls.get_mut(self.idx) {
            *slot = pos;
        }
    }

    /// Load `url` into the page. Shows progress through the pump while waiting.
    fn load(&mut self, url: &str, policy: Policy) -> Result<(), Cancelled> {
        self.url = url.to_string();
        self.keyboard = false;
        self.sheet = None;
        self.chrome = true;
        self.reader = false;
        self.page_dirty = true;
        // Show the address bar with the new URL (and the progress strip under it) before waiting,
        // even when the bars were hidden by scrolling. Otherwise the strip lands on page content.
        self.touch(0, self.height, Wave::Text);
        self.emit_frame();

        let (html, base, is_web) = if let Some(rest) = url.strip_prefix("kbp://") {
            // Saved pages resolve relative links against their original address.
            let base = rest
                .strip_prefix("saved/")
                .and_then(|key| read_pairs(&data_file("saved.tsv")).into_iter().find(|(k, _)| k == key))
                .and_then(|(_, meta)| meta.split_once('\t').map(|(u, _)| u.to_string()))
                .unwrap_or_else(|| url.to_string());
            (self.internal_html(rest), base, false)
        } else {
            match self.fetch_with_progress(url, policy)? {
                Ok(f) => {
                    self.url = f.url.clone();
                    if let Some(last) = self.history.get_mut(self.idx) {
                        *last = f.url.clone();
                    }
                    let html = if !f.content_type.is_empty() && !f.content_type.contains("html") {
                        ui::error("Not a web page", "This link opens a file that is not a web page.", &f.content_type)
                    } else {
                        String::from_utf8_lossy(&f.body).into_owned()
                    };
                    (html, f.url, true)
                }
                Err(e) => {
                    eprintln!("[load] {url}: {e}");
                    (ui::error("Could not load this page", &load_reason(&e), &e), url.to_string(), false)
                }
            }
        };
        self.source = html.clone();
        self.build_page(&html, &base, is_web);
        self.touch(0, self.height, Wave::Text);
        Ok(())
    }

    /// Build the page, waiting for resources with progress. Images are deferred.
    fn build_page(&mut self, html: &str, base: &str, is_web: bool) {
        let prepared = prepare_html(html, &self.settings);
        let html = prepared.as_str();
        let stats = self.net.stats();
        let mut pump = self.pump.take();
        self.net.set_defer_images(true);
        let (w, h) = (self.width, self.height);
        // Web pages lay out at a phone width; internal pages and chrome keep device pixels.
        let css_width = if is_web { Some(self.settings.page_width as f32) } else { None };
        let net = self.net.clone();
        net.set_images_allowed(self.settings.images);
        let built = {
            let mut waiting = || {
                let req = stats.requested.load(std::sync::atomic::Ordering::Relaxed);
                let done = stats.done.load(std::sync::atomic::Ordering::Relaxed);
                let frac = if req == 0 { 0.3 } else { 0.3 + 0.6 * done as f32 / req as f32 };
                match pump.as_mut() {
                    Some(p) => p(frac, "Loading"),
                    None => true,
                }
            };
            std::panic::catch_unwind(AssertUnwindSafe(|| Page::new_scaled(html, base, w, h, net.clone(), &mut waiting, css_width)))
        };
        self.net.set_defer_images(false);
        self.pump = pump;
        let page = match built {
            Ok(p) => p,
            Err(_) => {
                let fallback = ui::error("Page error", "This page could not be shown.", "");
                Page::new(&fallback, base, w, h, self.net.clone(), &mut || true)
            }
        };
        page.start_deferred_images();
        self.stash_current();
        if is_web {
            self.record_visit(&self.url.clone(), page.title.as_str());
            kb_net::save_cookies();
        }
        self.page = Some(page);
    }

    /// Fetch a document on a worker thread so the UI can report progress and
    /// accept taps (a tap cancels the load).
    fn fetch_with_progress(
        &mut self,
        url: &str,
        policy: Policy,
    ) -> Result<Result<kb_net::Fetched, String>, Cancelled> {
        let u = url.to_string();
        self.run_job(move || kb_net::load(&u, MAX_DOC_BYTES, policy))
    }

    /// Run a document job (GET, POST) on a worker thread, reporting progress meanwhile.
    fn run_job(
        &mut self,
        job: impl FnOnce() -> Result<kb_net::Fetched, String> + Send + 'static,
    ) -> Result<Result<kb_net::Fetched, String>, Cancelled> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(job());
        });
        let start = Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(60)) {
                Ok(result) => return Ok(result),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let frac = (0.05 + start.elapsed().as_secs_f32() / 8.0 * 0.2).min(0.25);
                    if let Some(pump) = self.pump.as_mut() {
                        if !pump(frac, "Connecting") {
                            return Err(Cancelled);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Ok(Err("network thread stopped".to_string()));
                }
            }
        }
    }

    /// Called regularly by the driver (on idle). Brings in held-back images and
    /// clears a notice when its time is up.
    pub fn tick(&mut self) -> Option<Update> {
        let done = self.net.stats().deferred_done.load(std::sync::atomic::Ordering::Relaxed);
        // Images trickle in one by one. Repaint at most every 1.5 s so the page doesn't flash.
        if done != self.images_seen && self.last_image_refresh.elapsed() >= Duration::from_millis(1500) {
            self.images_seen = done;
            self.last_image_refresh = Instant::now();
            self.touch(0, self.height, Wave::Text);
            self.page_dirty = true;
            return Some(self.frame());
        }
        if let Some((_, until)) = &self.toast {
            if Instant::now() >= *until {
                self.toast = None;
                let (y0, y1) = toast_rows(self.height);
                self.touch(y0, y1, Wave::Fast);
                return Some(self.frame());
            }
        }
        None
    }

    /// Show a short notice above the bottom toolbar for about two seconds.
    pub fn notify(&mut self, message: &str) {
        self.toast = Some((message.to_string(), Instant::now() + Duration::from_millis(2200)));
        let (y0, y1) = toast_rows(self.height);
        self.touch(y0, y1, Wave::Fast);
    }

    /// Whether a notice is on screen (drivers keep ticking until it clears).
    pub fn notice_showing(&self) -> bool {
        self.toast.is_some()
    }

    // ---- input ------------------------------------------------------------

    pub fn scroll(&mut self, dy: f64) {
        if self.keyboard {
            return;
        }
        // An open sheet (menu, tabs) takes the swipe and scrolls its own list.
        if let Some(sheet) = self.sheet {
            let cached = self.ensure_sheet(sheet);
            cached.page.scroll_by(dy);
            cached.gray = cached.page.render_gray_at(0);
            self.touch(self.height - MENU_H, self.height, Wave::Text);
            return;
        }
        // Scrolling down hides the bars to give the page room; scrolling up brings them back.
        if dy > 8.0 && self.chrome {
            self.set_chrome(false);
        } else if dy < -8.0 && !self.chrome {
            self.set_chrome(true);
        }
        if let Some(page) = self.page.as_mut() {
            page.scroll_by(dy);
            self.page_dirty = true;
            self.touch(0, self.height, Wave::Text);
        }
    }

    /// Scroll by one screen (`dir` = +1 down, -1 up), keeping a little overlap.
    pub fn page_step(&mut self, dir: f64) {
        let step = (self.height - TOP_H) as f64 * 0.9 * dir;
        self.scroll(step);
    }

    /// Long press on a link opens it in a new tab.
    pub fn long_press(&mut self, x: f32, y: f32) {
        if self.keyboard || self.sheet.is_some() || y < TOP_H as f32 || y >= (self.height - BOT_H) as f32 {
            return;
        }
        let off = self.page_offset() as f32;
        let href = self.page.as_ref().and_then(|p| p.link_at(x, y - off));
        match href {
            Some(href) if !href.starts_with("kb:") => {
                self.new_tab();
                self.open(&href);
                self.notify("Opened in new tab");
            }
            _ => {}
        }
    }

    /// Step to the next (+1) or previous (-1) tab, wrapping around.
    pub fn tab_step(&mut self, dir: i32) {
        let n = self.tabs.len().max(1) as i32;
        if n <= 1 {
            return;
        }
        let next = (self.active as i32 + dir).rem_euclid(n) as usize;
        self.switch_tab(next);
        self.notify(&format!("Tab {} of {}", next + 1, n));
    }

    /// Run a browser action by name, e.g. `mark`, `menu`, `reader`, `home`.
    pub fn action(&mut self, name: &str) {
        self.chrome_action(&format!("kb:{name}"));
    }

    /// Handle a tap at device pixel (x, y).
    pub fn tap(&mut self, x: f32, y: f32) {
        let h = self.height as f32;
        if self.keyboard {
            if y < KB_Y as f32 {
                let field_box = self.ensure_field().page.link_box(x, y);
                if let Some(href) = self.ensure_field().page.link_at(x, y) {
                    if let Some(n) = href.strip_prefix("kb:pos:").and_then(|v| v.parse::<usize>().ok()) {
                        // Tapping a character puts the caret before it, or after it on the right half.
                        let right = field_box.map_or(false, |(bx, _, bw, _)| x > bx + bw / 2.0);
                        self.caret = (n + usize::from(right)).min(self.typed.len());
                        self.touch(0, KB_Y, Wave::Fast);
                    } else {
                        self.keyboard_action(&href);
                    }
                }
            } else if let Some(href) = self.ensure_key(self.layer).page.link_at(x, y - KB_Y as f32) {
                self.keyboard_action(&href);
            }
            return;
        }
        if let Some(sheet) = self.sheet {
            if y >= h - MENU_H as f32 {
                let href = self.ensure_sheet(sheet).page.link_at(x, y - (h - MENU_H as f32));
                eprintln!("[ui] sheet={sheet:?} tap ({x:.0},{y:.0}) -> {href:?}");
                if let Some(href) = href {
                    if !stays_open(&href) {
                        self.sheet = None;
                    }
                    self.chrome_action(&href);
                }
                self.touch(self.height - MENU_H, self.height, Wave::Text);
            } else {
                eprintln!("[ui] sheet={sheet:?} dismissed by tap above sheet at y={y:.0}");
                self.sheet = None;
                self.touch(self.height - MENU_H, self.height, Wave::Text);
            }
            return;
        }
        if self.chrome && y < TOP_H as f32 {
            if let Some(href) = self.ensure_top().page.link_at(x, y) {
                self.chrome_action(&href);
            }
            return;
        }
        if self.chrome && y >= (h - BOT_H as f32) {
            if let Some(href) = self.ensure_bottom().page.link_at(x, y - (h - BOT_H as f32)) {
                self.chrome_action(&href);
            }
            return;
        }
        // Page content: taps map back from the screen to the page's own coordinates.
        let y = y - self.page_offset() as f32;
        let Some(page) = self.page.as_mut() else { return };
        let outcome = page.click_at(x, y);
        if outcome == kb_engine::ClickKind::Prevented {
            self.page_dirty = true;
            self.touch(0, self.height, Wave::Text);
            return;
        }
        // Text fields open the keyboard for editing; submit buttons send their form.
        let field = self.page.as_ref().and_then(|p| p.input_at(x, y));
        if let Some((key, value)) = field {
            self.input_target = Some(key);
            self.keyboard = true;
            self.sheet = None;
            self.typed = value.chars().collect();
            self.caret = self.typed.len();
            self.layer = Layer::Lower;
            self.refresh_suggestions();
            self.touch(0, self.height, Wave::Text);
            return;
        }
        if let Some(submit) = self.page.as_ref().and_then(|p| p.form_submit_at(x, y)) {
            self.submit(submit);
            return;
        }
        let href = self.page.as_ref().and_then(|p| p.link_at(x, y));
        match href {
            Some(href) if href.starts_with("kb:") => self.chrome_action(&href),
            Some(href) => self.open(&href),
            None => {
                // Tapping empty page toggles the bars.
                let show = !self.chrome;
                self.set_chrome(show);
            }
        }
        if outcome != kb_engine::ClickKind::None {
            self.page_dirty = true;
            self.touch(0, self.height, Wave::Text);
        }
    }

    fn set_chrome(&mut self, visible: bool) {
        if self.chrome != visible {
            self.chrome = visible;
            // The page moves below the address bar when the bars are shown, so repaint it at text quality.
            self.page_dirty = true;
            self.touch(0, self.height, Wave::Text);
        }
    }

    /// Vertical offset of the page: below the address bar while the bars are shown.
    fn page_offset(&self) -> u32 {
        if self.chrome { TOP_H } else { 0 }
    }

    fn chrome_action(&mut self, href: &str) {
        eprintln!("[ui] action {href} (sheet={:?}, page={}, url={})", self.sheet, self.page.is_some(), self.url);
        if let Some(n) = href.strip_prefix("kb:rm:").and_then(|s| s.parse::<usize>().ok()) {
            if n < self.bookmarks.len() {
                self.bookmarks.remove(n);
                write_pairs(&data_file("bookmarks.tsv"), &self.bookmarks);
                self.refresh_internal();
            }
            return;
        }
        if let Some(n) = href.strip_prefix("kb:bmup:").and_then(|s| s.parse::<usize>().ok()) {
            if n > 0 && n < self.bookmarks.len() {
                self.bookmarks.swap(n - 1, n);
                write_pairs(&data_file("bookmarks.tsv"), &self.bookmarks);
                self.refresh_internal();
            }
            return;
        }
        if let Some(n) = href.strip_prefix("kb:bmdown:").and_then(|s| s.parse::<usize>().ok()) {
            if n + 1 < self.bookmarks.len() {
                self.bookmarks.swap(n, n + 1);
                write_pairs(&data_file("bookmarks.tsv"), &self.bookmarks);
                self.refresh_internal();
            }
            return;
        }
        if let Some(step) = href.strip_prefix("kb:font:") {
            self.step_text(if step == "+" { 1 } else { -1 });
            return;
        }
        if let Some(w) = href.strip_prefix("kb:width:").and_then(|s| s.parse::<u32>().ok()) {
            self.set_width(w);
            return;
        }
        if let Some(which) = href.strip_prefix("kb:homeshow:") {
            match which {
                "bm" => self.settings.home_bookmarks_open = !self.settings.home_bookmarks_open,
                _ => self.settings.home_recent_open = !self.settings.home_recent_open,
            }
            self.settings.save();
            self.refresh_internal();
            return;
        }
        if let Some(n) = href.strip_prefix("kb:tab:").and_then(|s| s.parse::<usize>().ok()) {
            self.switch_tab(n);
            return;
        }
        if let Some(n) = href.strip_prefix("kb:closetab:").and_then(|s| s.parse::<usize>().ok()) {
            self.close_tab(n);
            return;
        }
        if let Some(n) = href.strip_prefix("kb:sugg:").and_then(|s| s.parse::<usize>().ok()) {
            if let Some((url, _)) = self.suggestions.get(n).cloned() {
                self.keyboard = false;
                self.open(&url);
            }
            return;
        }
        match href {
            "kb:addr" => self.enter_keyboard(),
            "kb:reload" => self.reload(),
            "kb:back" => self.back(),
            "kb:fwd" => self.forward(),
            "kb:home" => self.open("kbp://home"),
            "kb:mark" => self.toggle_bookmark(),
            "kb:menu" => self.show_sheet(Sheet::Menu),
            "kb:tabs" => self.show_sheet(Sheet::Tabs),
            "kb:newtab" => self.new_tab(),
            "kb:closemenu" => {
                self.sheet = None;
                self.touch(self.height - MENU_H, self.height, Wave::Text);
            }
            "kb:reader" => {
                self.toggle_reader();
            }
            "kb:save" => self.save_offline(),
            "kb:marks" => self.open("kbp://bookmarks"),
            "kb:saved" => self.open("kbp://saved"),
            "kb:display" => self.show_sheet(Sheet::Display),
            "kb:images" => {
                self.settings.images = !self.settings.images;
                self.settings.save();
                // Rebuild the page: turning images on fetches them, turning them off drops them.
                self.reflow();
                // The menu shows the new state; a notice would cover it.
                if self.sheet.is_none() {
                    self.notify(if self.settings.images { "Images on" } else { "Images off" });
                }
            }
            "kb:night" => {
                self.settings.night = !self.settings.night;
                self.set_night(self.settings.night);
            }
            "kb:nighton" => self.set_night(true),
            "kb:nightoff" => self.set_night(false),
            "kb:clearhistory" => {
                self.visits.clear();
                write_pairs(&data_file("history.tsv"), &self.visits);
                self.touch(self.height - MENU_H, self.height, Wave::Text);
                self.notify("History cleared");
            }
            "kb:clearcache" => {
                store::cache_clear();
                self.touch(self.height - MENU_H, self.height, Wave::Text);
                self.sheet_layer = None;
                self.notify("Cache cleared");
            }
            "kb:exit" => {
                kb_net::save_cookies();
                self.exit = true;
            }
            _ => {}
        }
    }

    /// Move one step along `TEXT_STEPS` (-1 smaller, +1 larger) and reflow the page.
    fn step_text(&mut self, dir: i32) {
        let cur = TEXT_STEPS.iter().position(|&p| p == self.settings.text_pct).unwrap_or(2) as i32;
        let next = (cur + dir).clamp(0, TEXT_STEPS.len() as i32 - 1) as usize;
        if next as i32 == cur {
            return;
        }
        self.settings.text_pct = TEXT_STEPS[next];
        self.settings.save();
        self.reflow();
    }

    /// Choose the page width (one of `PAGE_WIDTHS`) and reflow the page.
    fn set_width(&mut self, width: u32) {
        if !PAGE_WIDTHS.contains(&width) || width == self.settings.page_width {
            return;
        }
        self.settings.page_width = width;
        self.settings.save();
        self.reflow();
    }

    /// Short label for the menu row: "Medium, 100%".
    fn display_summary(&self) -> String {
        format!("{}, {}%", width_name(self.settings.page_width), self.settings.text_pct)
    }

    fn set_night(&mut self, on: bool) {
        self.settings.night = on;
        self.settings.save();
        self.touch(0, self.height, Wave::Flash);
        if self.sheet.is_none() {
            self.notify(if on { "Night mode on" } else { "Night mode off" });
        }
    }

    /// Rebuild the page on screen from its source with the current settings, keeping the reading position.
    fn reflow(&mut self) {
        let url = self.url.clone();
        if url.is_empty() || url.starts_with("kbp://") || url.starts_with("kb://") || self.source.is_empty() {
            // Reloading an internal page clears the sheet; keep the one the user is adjusting.
            let sheet = self.sheet;
            self.refresh_internal();
            self.sheet = sheet;
            self.touch(0, self.height, Wave::Text);
            return;
        }
        let scroll = self.page.as_ref().map_or(0.0, |p| p.scroll_y());
        let source = self.source.clone();
        self.build_page(&source, &url, true);
        if let Some(page) = self.page.as_mut() {
            page.scroll_by(scroll);
        }
        self.page_dirty = true;
        self.touch(0, self.height, Wave::Text);
    }

    /// Re-open the internal page on screen (home, bookmarks) so it shows the latest data.
    /// Does not add a history entry.
    fn refresh_internal(&mut self) {
        let url = self.url.clone();
        if url.starts_with("kbp://") {
            let _ = self.load(&url, Policy::Network);
        }
    }

    // ---- keyboard -----------------------------------------------------------

    fn enter_keyboard(&mut self) {
        self.keyboard = true;
        self.sheet = None;
        self.input_target = None;
        let is_home = self.url.starts_with("kbp:");
        self.typed = if is_home { Vec::new() } else { self.url.chars().collect() };
        self.caret = self.typed.len();
        self.layer = Layer::Lower;
        self.refresh_suggestions();
        self.touch(0, self.height, Wave::Text);
    }

    fn keyboard_action(&mut self, href: &str) {
        let mut field_changed = true;
        let mut layer_changed = false;
        if let Some(code) = href.strip_prefix("kb:c:").and_then(|s| s.parse::<u32>().ok()) {
            if let Some(c) = char::from_u32(code) {
                self.typed.insert(self.caret, c);
                self.caret += 1;
            }
            // One-shot uppercase: after one capital letter, return to lowercase.
            if self.layer == Layer::Upper {
                self.layer = Layer::Lower;
                layer_changed = true;
            }
        } else {
            match href {
                "kb:shift" => {
                    self.layer = if self.layer == Layer::Upper { Layer::Lower } else { Layer::Upper };
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:sym" => {
                    self.layer = Layer::Symbols;
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:abc" => {
                    self.layer = Layer::Lower;
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:del" => {
                    if self.caret > 0 {
                        self.caret -= 1;
                        self.typed.remove(self.caret);
                    }
                }
                "kb:left" => {
                    self.caret = self.caret.saturating_sub(1);
                }
                "kb:right" => {
                    self.caret = (self.caret + 1).min(self.typed.len());
                }
                "kb:clear" => {
                    self.typed.clear();
                    self.caret = 0;
                }
                "kb:cancel" => {
                    self.keyboard = false;
                    self.input_target = None;
                    self.touch(0, self.height, Wave::Text);
                    return;
                }
                "kb:go" => {
                    let text: String = self.typed.iter().collect();
                    self.keyboard = false;
                    if let Some(key) = self.input_target.take() {
                        // Editing a page field: write the value back, submit its form if it has one.
                        let form = self.page.as_mut().and_then(|p| p.set_input_value(key, &text));
                        match form {
                            Some(submit) => self.submit(submit),
                            None => {
                                self.page_dirty = true;
                                self.touch(0, self.height, Wave::Text);
                            }
                        }
                    } else if !text.trim().is_empty() {
                        self.open(&text);
                    } else {
                        self.touch(0, self.height, Wave::Text);
                    }
                    return;
                }
                _ => {
                    field_changed = false;
                }
            }
        }
        if field_changed {
            self.refresh_suggestions();
        }
        if field_changed {
            self.touch(0, KB_Y, Wave::Fast);
        }
        if layer_changed {
            self.touch(KB_Y, self.height, Wave::Fast);
        }
    }

    fn refresh_suggestions(&mut self) {
        let q: String = self.typed.iter().collect::<String>().to_lowercase();
        let mut out: Vec<(String, String)> = Vec::new();
        for (u, t) in self.bookmarks.iter().chain(self.visits.iter()) {
            if out.len() >= 6 {
                break;
            }
            let hit = q.is_empty() || u.to_lowercase().contains(&q) || t.to_lowercase().contains(&q);
            if hit && !out.iter().any(|(x, _)| x == u) {
                out.push((u.clone(), t.clone()));
            }
        }
        self.suggestions = out;
    }

    // ---- bookmarks, history, reader, saved -------------------------------------

    fn toggle_bookmark(&mut self) {
        if self.url.is_empty() || self.url.starts_with("kbp:") {
            return;
        }
        if let Some(i) = self.bookmarks.iter().position(|(u, _)| *u == self.url) {
            self.bookmarks.remove(i);
        } else {
            self.bookmarks.push((self.url.clone(), self.title().to_string()));
        }
        write_pairs(&data_file("bookmarks.tsv"), &self.bookmarks);
        let added = self.bookmarks.iter().any(|(u, _)| *u == self.url);
        self.notify(if added { "Bookmark added" } else { "Bookmark removed" });
        self.touch(self.height - BOT_H, self.height, Wave::Fast);
    }

    fn record_visit(&mut self, url: &str, title: &str) {
        self.visits.retain(|(u, _)| u != url);
        self.visits.insert(0, (url.to_string(), title.to_string()));
        self.visits.truncate(HISTORY_CAP);
        write_pairs(&data_file("history.tsv"), &self.visits);
    }

    fn toggle_reader(&mut self) {
        if self.reader {
            let url = self.url.clone();
            let _ = self.load(&url, Policy::Cache(Duration::from_secs(60 * 60 * 24 * 7)));
            return;
        }
        let Some(html) = self.page.as_ref().and_then(|p| p.reader_html()) else { return };
        let base = self.url.clone();
        self.build_page(&html, &base, false);
        self.reader = true;
        self.touch(0, self.height, Wave::Text);
    }

    fn save_offline(&mut self) {
        if self.url.is_empty() || self.url.starts_with("kbp:") || self.source.is_empty() {
            return;
        }
        let key = format!("{:016x}", fnv(&self.url));
        let dir = data_file("saved");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join(format!("{key}.html")), &self.source);
        let mut index = read_pairs(&data_file("saved.tsv"));
        index.retain(|(k, _)| *k != key);
        index.push((key, format!("{}\t{}", self.url, self.title())));
        write_pairs(&data_file("saved.tsv"), &index);
        self.sheet_layer = None;
        self.touch(self.height - MENU_H, self.height, Wave::Text);
        self.notify("Saved for offline");
    }

    // ---- change tracking & frames ------------------------------------------

    fn touch(&mut self, y0: u32, y1: u32, wave: Wave) {
        self.change = Some(match self.change {
            None => ((y0, y1), wave),
            Some(((a, b), w)) => ((a.min(y0), b.max(y1)), w.max(wave)),
        });
    }

    /// The current screen as an update. Only the rows that changed are marked
    /// for refresh; the rest of the buffer is still complete.
    pub fn frame(&mut self) -> Update {
        // The highlight drawn by `press` must be taken back to normal here.
        if let Some((a, b)) = self.pressed_rows.take() {
            self.touch(a, b, Wave::Fast);
        }
        let (rows, wave) = self.change.take().unwrap_or(((0, self.height), Wave::Text));
        let mut gray = if self.keyboard { self.compose_keyboard() } else { self.compose_browse() };
        if self.settings.night {
            for px in gray.iter_mut() {
                *px = 255 - *px;
            }
        }
        self.last_gray = gray.clone();
        Update { gray, width: self.width, height: self.height, rows, wave }
    }

    /// Push the current screen to the draw sink now (used while a load is still running).
    fn emit_frame(&mut self) {
        if self.draw.is_none() {
            return;
        }
        let update = self.frame();
        if let Some(draw) = self.draw.as_mut() {
            draw(update);
        }
    }

    /// Tap feedback: the element under (x, y) turns black at once, so the press
    /// registers before the action finishes. Returns the highlighted frame, or
    /// `None` when the point is not on a tappable element.
    pub fn press(&mut self, x: f32, y: f32) -> Option<Update> {
        let (bx, by, bw, bh) = self.tappable_box(x, y)?;
        if self.last_gray.is_empty() {
            return None;
        }
        let w = self.width;
        let x0 = bx.max(0.0) as u32;
        let x1 = ((bx + bw) as u32).min(w);
        let y0 = by.max(0.0) as u32;
        let y1 = ((by + bh) as u32).min(self.height);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let mut gray = self.last_gray.clone();
        for row in y0..y1 {
            let base = (row * w) as usize;
            for px in &mut gray[base + x0 as usize..base + x1 as usize] {
                *px = 255 - *px;
            }
        }
        self.pressed_rows = Some((y0, y1));
        Some(Update { gray, width: w, height: self.height, rows: (y0, y1), wave: Wave::Fast })
    }

    /// Screen box of the link under a tap, following the same layers as `tap`.
    fn tappable_box(&mut self, x: f32, y: f32) -> Option<(f32, f32, f32, f32)> {
        let h = self.height as f32;
        if self.keyboard {
            if y < KB_Y as f32 {
                return self.ensure_field().page.link_box(x, y);
            }
            let (bx, by, bw, bh) = self.ensure_key(self.layer).page.link_box(x, y - KB_Y as f32)?;
            return Some((bx, by + KB_Y as f32, bw, bh));
        }
        if let Some(sheet) = self.sheet {
            if y >= h - MENU_H as f32 {
                let top = h - MENU_H as f32;
                let (bx, by, bw, bh) = self.ensure_sheet(sheet).page.link_box(x, y - top)?;
                return Some((bx, by + top, bw, bh));
            }
            return None;
        }
        if self.chrome && y < TOP_H as f32 {
            return self.ensure_top().page.link_box(x, y);
        }
        if self.chrome && y >= h - BOT_H as f32 {
            let top = h - BOT_H as f32;
            let (bx, by, bw, bh) = self.ensure_bottom().page.link_box(x, y - top)?;
            return Some((bx, by + top, bw, bh));
        }
        let off = self.page_offset() as f32;
        self.page.as_ref()?.link_box(x, y - off).map(|(bx, by, bw, bh)| (bx, by + off, bw, bh))
    }

    fn compose_browse(&mut self) -> Vec<u8> {
        let want = self.page_offset();
        if self.page_dirty || self.page_gray.is_empty() || self.page_gray_offset != want {
            self.page_gray = match self.page.as_mut() {
                Some(p) => p.render_gray_at(want),
                None => vec![255u8; (self.width * self.height) as usize],
            };
            self.page_gray_offset = want;
            self.page_dirty = false;
        }
        let mut out = self.page_gray.clone();
        let (w, h) = (self.width, self.height);
        if let Some(sheet) = self.sheet {
            let layer = self.ensure_sheet(sheet).gray.clone();
            blit(&mut out, w, h - MENU_H, &layer, MENU_H);
        } else if self.chrome {
            let top = self.ensure_top().gray.clone();
            blit(&mut out, w, 0, &top, TOP_H);
            let bottom = self.ensure_bottom().gray.clone();
            blit(&mut out, w, h - BOT_H, &bottom, BOT_H);
        }
        if let Some((message, _)) = self.toast.clone() {
            let layer = self.toast_layer(&message);
            let (y0, _) = toast_rows(self.height);
            blit(&mut out, w, y0, &layer, 88);
        }
        if self.images_loading() {
            let s = self.net.stats();
            let done = s.deferred_done.load(std::sync::atomic::Ordering::Relaxed) as f32;
            let total = s.deferred.load(std::sync::atomic::Ordering::Relaxed).max(1) as f32;
            let y = if self.chrome { TOP_H - PROGRESS_H } else { 0 };
            progress_strip(&mut out, w, y, done / total);
        }
        out
    }

    fn toast_layer(&mut self, message: &str) -> Vec<u8> {
        if self.toast_cache.as_ref().map_or(true, |(m, _)| m != message) {
            let html = ui::toast(message);
            let mut page = Page::new(&html, "kb://chrome/", self.width, 88, self.net.clone(), &mut || true);
            let gray = page.render_gray();
            self.toast_cache = Some((message.to_string(), gray));
        }
        self.toast_cache.as_ref().map(|(_, g)| g.clone()).unwrap_or_default()
    }

    fn compose_keyboard(&mut self) -> Vec<u8> {
        let (w, h) = (self.width, self.height);
        let mut out = vec![255u8; (w * h) as usize];
        let field = self.ensure_field().gray.clone();
        blit(&mut out, w, 0, &field, KB_Y);
        let keys = self.ensure_key(self.layer).gray.clone();
        blit(&mut out, w, KB_Y, &keys, h - KB_Y);
        out
    }

    // ---- cached layers --------------------------------------------------------

    fn render_layer(&self, html: &str, base: &str, w: u32, h: u32) -> Cached {
        let mut page = Page::new(html, base, w, h, self.net.clone(), &mut || true);
        let gray = page.render_gray();
        Cached { key: String::new(), page, gray }
    }

    fn ensure_top(&mut self) -> &mut Cached {
        let key = format!("{}|{}", self.url, self.can_go_back());
        if self.top.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::top(&self.url);
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, TOP_H);
            c.key = key;
            self.top = Some(c);
        }
        self.top.as_mut().expect("top layer")
    }

    fn ensure_bottom(&mut self) -> &mut Cached {
        let marked = self.bookmarks.iter().any(|(u, _)| *u == self.url);
        let key = format!("{}{}{}{}|{}", self.can_go_back() as u8, self.can_go_forward() as u8, marked as u8, self.reader as u8, self.tabs.len().max(1));
        if self.bottom.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::bottom(self.can_go_back(), self.can_go_forward(), marked, self.tabs.len().max(1));
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, BOT_H);
            c.key = key;
            self.bottom = Some(c);
        }
        self.bottom.as_mut().expect("bottom layer")
    }

    fn show_sheet(&mut self, sheet: Sheet) {
        self.sheet = Some(sheet);
        self.touch(self.height - MENU_H, self.height, Wave::Text);
    }

    /// The menu or tab sheet, rendered once per state.
    fn ensure_sheet(&mut self, sheet: Sheet) -> &mut Cached {
        let cache_mb = store::cache_size() / (1024 * 1024);
        let marked = self.bookmarks.iter().any(|(u, _)| *u == self.url);
        let tabs: Vec<(String, String)> = self.tab_labels();
        let key = match sheet {
            Sheet::Menu => format!(
                "menu|{}|{}|{}|{}|{}|{}|{}|{}",
                self.reader, cache_mb, marked, self.url, self.settings.night, self.settings.text_pct, self.settings.page_width, self.settings.images
            ),
            Sheet::Tabs => format!("tabs|{}|{:?}", self.active, tabs),
            Sheet::Display => format!("display|{}|{}|{}", self.settings.text_pct, self.settings.page_width, self.settings.night),
        };
        if self.sheet_layer.as_ref().map_or(true, |c| c.key != key) {
            let html = match sheet {
                Sheet::Menu => ui::menu(self.reader, cache_mb, marked, self.settings.night, &self.display_summary(), self.settings.images),
                Sheet::Tabs => ui::tabs(&tabs, self.active),
                Sheet::Display => ui::display(self.settings.text_pct, self.settings.page_width, self.settings.night),
            };
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, MENU_H);
            c.key = key;
            self.sheet_layer = Some(c);
        }
        self.sheet_layer.as_mut().expect("sheet layer")
    }

    /// (title, url) for each tab, the active one included.
    fn tab_labels(&self) -> Vec<(String, String)> {
        (0..self.tabs.len().max(1))
            .map(|i| {
                if i == self.active {
                    (self.title().to_string(), self.url.clone())
                } else {
                    let t = &self.tabs[i];
                    (t.title.clone(), t.url.clone())
                }
            })
            .collect()
    }

    /// Save the active tab's navigation state into its slot.
    fn stash_active(&mut self) {
        let title = self.title().to_string();
        self.remember_scroll();
        let state = TabSave {
            history: std::mem::take(&mut self.history),
            scrolls: std::mem::take(&mut self.scrolls),
            idx: self.idx,
            url: self.url.clone(),
            title,
        };
        if self.tabs.len() <= self.active {
            self.tabs.resize_with(self.active + 1, TabSave::empty);
        }
        self.tabs[self.active] = state;
    }

    fn switch_tab(&mut self, n: usize) {
        if n >= self.tabs.len() || n == self.active {
            self.sheet = None;
            return;
        }
        self.stash_active();
        let t = std::mem::replace(&mut self.tabs[n], TabSave::empty());
        self.history = t.history;
        self.scrolls = t.scrolls;
        self.idx = t.idx;
        self.url = t.url.clone();
        self.active = n;
        self.sheet = None;
        let url = t.url;
        if url.is_empty() {
            self.open("kbp://home");
        } else {
            let _ = self.load(&url, Policy::Cache(Duration::from_secs(60 * 60 * 24 * 7)));
        }
    }

    fn new_tab(&mut self) {
        self.stash_active();
        self.tabs.push(TabSave::empty());
        let n = self.tabs.len() - 1;
        self.switch_tab(n);
        if self.url.is_empty() {
            self.open("kbp://home");
        }
    }

    fn close_tab(&mut self, n: usize) {
        if self.tabs.len() <= 1 || n >= self.tabs.len() {
            return;
        }
        if n == self.active {
            let next = if n + 1 < self.tabs.len() { n + 1 } else { n - 1 };
            self.switch_tab(next);
            let removed = n;
            self.tabs.remove(removed);
            if removed < self.active {
                self.active -= 1;
            }
        } else {
            self.tabs.remove(n);
            if n < self.active {
                self.active -= 1;
            }
        }
        self.touch(self.height - MENU_H, self.height, Wave::Text);
    }

    fn ensure_field(&mut self) -> &mut Cached {
        let typed: String = self.typed.iter().collect();
        let sugg: String = self.suggestions.iter().map(|(u, _)| u.as_str()).collect::<Vec<_>>().join("\n");
        let input = self.input_target.is_some();
        let key = format!("{typed}|{}|{sugg}|{input}", self.caret);
        if self.field.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::field(&self.typed, self.caret, &self.suggestions, input);
            let mut c = self.render_layer(&html, "kb://keyboard/", self.width, KB_Y);
            c.key = key;
            self.field = Some(c);
        }
        self.field.as_mut().expect("field layer")
    }

    fn ensure_key(&mut self, layer: Layer) -> &mut Cached {
        let idx = layer as usize;
        if self.keys[idx].is_none() {
            let html = ui::keyboard_for(layer);
            let c = self.render_layer(&html, "kb://keyboard/", self.width, self.height - KB_Y);
            self.keys[idx] = Some(c);
        }
        self.keys[idx].as_mut().expect("key layer")
    }

    // ---- internal pages -------------------------------------------------------

    fn internal_html(&self, rest: &str) -> String {
        if let Some(key) = rest.strip_prefix("saved/") {
            return self.saved_source(key);
        }
        match rest {
            "home" => ui::home(
                &self.bookmarks,
                &self.visits,
                self.settings.home_bookmarks_open,
                self.settings.home_recent_open,
            ),
            "bookmarks" => ui::bookmarks(&self.bookmarks),
            "saved" => {
                let items: Vec<ui::ListItem> = read_pairs(&data_file("saved.tsv"))
                    .iter()
                    .map(|(key, meta)| {
                        let (url, title) = meta.split_once('\t').unwrap_or((meta, meta));
                        ui::ListItem {
                            href: format!("kbp://saved/{key}"),
                            label: title.to_string(),
                            sub: url.to_string(),
                            action: None,
                        }
                    })
                    .collect();
                ui::list("Saved pages", &items, "No saved pages yet. Open Menu and tap Save for offline.")
            }
            _ => ui::error("Page not found", "This browser page does not exist.", rest),
        }
    }


    fn saved_source(&self, key: &str) -> String {
        if !key.chars().all(|c| c.is_ascii_hexdigit()) {
            return ui::error("Saved page", "This link is not valid.", "");
        }
        fs::read_to_string(data_file("saved").join(format!("{key}.html")))
            .unwrap_or_else(|_| ui::error("Saved page", "This saved copy is no longer on the device.", ""))
    }
}

// ---- drawing helpers ------------------------------------------------------

/// Copy `rows` rows of `src` (full width) into `dst` starting at row `y`.
fn blit(dst: &mut [u8], width: u32, y: u32, src: &[u8], rows: u32) {
    let w = width as usize;
    let start = y as usize * w;
    let n = (rows as usize * w).min(src.len()).min(dst.len().saturating_sub(start));
    dst[start..start + n].copy_from_slice(&src[..n]);
}

/// Thin progress bar at row `y`: filled to `fraction`, outline elsewhere.
fn progress_strip(dst: &mut [u8], width: u32, y: u32, fraction: f32) {
    let w = width as usize;
    let fill = (fraction.clamp(0.0, 1.0) * width as f32) as usize;
    for row in 0..PROGRESS_H as usize {
        let base = (y as usize + row) * w;
        if base + w > dst.len() {
            break;
        }
        for x in 0..w {
            dst[base + x] = if x < fill { 0 } else { 200 };
        }
    }
}

/// Encode an 8-bit grayscale frame as PNG.
pub fn encode_png(width: u32, height: u32, gray: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(png::ColorType::Grayscale);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(gray).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

// ---- HTML for chrome -------------------------------------------------------




/// Which sheet slides up over the bottom of the screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sheet {
    Menu,
    Tabs,
    Display,
}

/// Navigation state of a tab that is not showing.
#[derive(Clone, Default, Debug)]
struct TabSave {
    history: Vec<String>,
    scrolls: Vec<f64>,
    idx: usize,
    url: String,
    title: String,
}

impl TabSave {
    fn empty() -> TabSave {
        TabSave::default()
    }
}




/// One key: label, action href, and width in units (a row is about ten units wide).
pub(crate) struct Key {
    pub(crate) label: String,
    pub(crate) href: String,
    pub(crate) width: f32,
    pub(crate) class: &'static str,
}

fn key(label: &str, href: &str, width: f32) -> Key {
    Key { label: label.to_string(), href: href.to_string(), width, class: "" }
}

fn char_key(c: char) -> Key {
    key(&c.to_string(), &format!("kb:c:{}", c as u32), 1.0)
}

/// Rows for the keyboard layers. Proportions follow the common phone/e-reader
/// arrangement: letter rows, wide Shift and Backspace, and a bottom row with
/// the layer switch, arrows, a wide space bar and Go.
pub(crate) fn keyboard_rows(layer: Layer) -> Vec<Vec<Key>> {
    let up = layer == Layer::Upper;
    let letters = |s: &str| -> Vec<Key> {
        s.chars()
            .map(|c| char_key(if up { c.to_ascii_uppercase() } else { c }))
            .collect()
    };
    let bottom = |switch: Key| -> Vec<Key> {
        vec![
            switch,
            key("‹", "kb:left", 1.0),
            key("space", "kb:c:32", 3.0),
            key("›", "kb:right", 1.0),
            key(".", "kb:c:46", 1.0),
            Key { class: "go", ..key("Go", "kb:go", 1.5) },
        ]
    };
    match layer {
        Layer::Lower | Layer::Upper => {
            let shift = Key {
                class: if up { "on" } else { "" },
                ..key("Shift", "kb:shift", 1.5)
            };
            let mut row3 = vec![shift];
            row3.extend(letters("zxcvbnm"));
            row3.push(key("Del", "kb:del", 1.5));
            vec![
                letters("1234567890"),
                letters("qwertyuiop"),
                letters("asdfghjkl"),
                row3,
                bottom(key("123", "kb:sym", 1.5)),
            ]
        }
        Layer::Symbols => {
            let mut row4 = vec![key("ABC", "kb:abc", 1.5)];
            row4.extend(".,()?!'\"".chars().map(char_key));
            row4.push(key("Del", "kb:del", 1.5));
            vec![
                "!@#$%^&*()".chars().map(char_key).collect(),
                "`~_+-=[]{}".chars().map(char_key).collect(),
                "\\|/:;\"<>".chars().map(char_key).collect(),
                row4,
                bottom(key("ABC", "kb:abc", 1.5)),
            ]
        }
    }
}


// ---- URL and storage helpers ----------------------------------------------

/// Turn typed input into a URL: bare words become a search, dotted names get https.
pub fn normalize(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("kbp://") {
        return s.to_string();
    }
    if s.contains(' ') || !s.contains('.') {
        return format!("https://lite.duckduckgo.com/lite/?q={}", percent(s));
    }
    format!("https://{s}")
}

/// Name of a page width, as shown to the reader.
pub fn width_name(width: u32) -> &'static str {
    match width {
        412 => "Narrow",
        800 => "Wide",
        _ => "Medium",
    }
}

/// Sheet actions that change a setting or a list in place. The sheet stays open so the new state is visible.
/// Navigation, tab switches and "done" close it.
fn stays_open(href: &str) -> bool {
    matches!(
        href,
        "kb:night" | "kb:nighton" | "kb:nightoff" | "kb:images" | "kb:mark" | "kb:save" | "kb:clearcache" | "kb:clearhistory" | "kb:display"
    ) || href.starts_with("kb:font:")
        || href.starts_with("kb:width:")
        || href.starts_with("kb:closetab:")
}

/// Toast position: just above the bottom toolbar.
fn toast_rows(height: u32) -> (u32, u32) {
    let y1 = height - BOT_H - 16;
    (y1 - 88, y1)
}

/// Reading settings and home-screen layout, kept in data/settings.tsv.
#[derive(Clone, Debug)]
struct Settings {
    text_pct: u32,
    night: bool,
    /// CSS width web pages are laid out at (one of `PAGE_WIDTHS`).
    page_width: u32,
    home_bookmarks_open: bool,
    home_recent_open: bool,
    /// Whether images are fetched and drawn. Off saves memory and bandwidth for text reading.
    images: bool,
}

impl Settings {
    fn load() -> Settings {
        let pairs = read_pairs(&data_file("settings.tsv"));
        let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let flag = |k: &str, default: bool| get(k).map_or(default, |v| v == "1");
        let width = get("page_width").and_then(|v| v.parse::<u32>().ok()).unwrap_or(DEFAULT_PAGE_WIDTH);
        Settings {
            text_pct: get("text_pct").and_then(|v| v.parse().ok()).unwrap_or(100),
            night: flag("night", false),
            page_width: if PAGE_WIDTHS.contains(&width) { width } else { DEFAULT_PAGE_WIDTH },
            home_bookmarks_open: flag("home_bookmarks_open", true),
            home_recent_open: flag("home_recent_open", true),
            images: flag("images", true),
        }
    }

    fn save(&self) {
        let flag = |b: bool| if b { "1" } else { "0" }.to_string();
        write_pairs(
            &data_file("settings.tsv"),
            &[
                ("text_pct".to_string(), self.text_pct.to_string()),
                ("night".to_string(), flag(self.night)),
                ("page_width".to_string(), self.page_width.to_string()),
                ("home_bookmarks_open".to_string(), flag(self.home_bookmarks_open)),
                ("home_recent_open".to_string(), flag(self.home_recent_open)),
                ("images".to_string(), flag(self.images)),
            ],
        );
    }
}

/// Add page-level CSS to the document's head: image placeholders and text size.
fn prepare_html(html: &str, settings: &Settings) -> String {
    let mut css = String::from("<style>img{background:#ececec}");
    if settings.text_pct != 100 {
        css.push_str(&format!("html{{zoom:{}%}}", settings.text_pct));
    }
    css.push_str("</style>");
    let lower = html.to_ascii_lowercase();
    if let Some(i) = lower.find("<head") {
        if let Some(j) = html[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], css, &html[at..]);
        }
    }
    // No head element: put the style right after the doctype (keeps standards mode).
    if let Some(i) = lower.find("<!doctype") {
        if let Some(j) = html[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], css, &html[at..]);
        }
    }
    format!("{css}{html}")
}

pub(crate) fn split_url(url: &str) -> (String, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), String::new()),
    }
}

fn data_file(name: &str) -> PathBuf {
    store::data_dir().join(name)
}

/// First run: seed the default bookmarks.
fn ensure_defaults() {
    // One-time migration: add the default bookmarks once, so devices that already
    // have a bookmark list get them too, and removing one later doesn't bring it back.
    const MARKER: &str = "defaults_v2";
    let marker = data_file(MARKER);
    if marker.exists() {
        return;
    }
    let path = data_file("bookmarks.tsv");
    let mut list = read_pairs(&path);
    for (u, t) in DEFAULT_BOOKMARKS {
        if !list.iter().any(|(existing, _)| existing == u) {
            list.push((u.to_string(), t.to_string()));
        }
    }
    write_pairs(&path, &list);
    if let Some(dir) = marker.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&marker, "1\n");
}

/// Read tab-separated `key<TAB>value` lines.
fn read_pairs(path: &PathBuf) -> Vec<(String, String)> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(a, b)| (a.to_string(), b.to_string())))
        .collect()
}

fn write_pairs(path: &PathBuf, pairs: &[(String, String)]) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = pairs
        .iter()
        .map(|(a, b)| format!("{}\t{}\n", a.replace(['\t', '\n'], " "), b.replace(['\t', '\n'], " ")))
        .collect();
    let _ = fs::write(path, text);
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}


fn percent(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}



/// Plain-language reason for a failed load (the technical text stays in the log).
fn load_reason(err: &str) -> String {
    if err.contains("HTTP 404") {
        "The address was not found on the server (404).".into()
    } else if err.contains("HTTP") {
        format!("The server answered with an error ({}).", err.trim_start_matches("HTTP "))
    } else if err.contains("request failed") {
        "No connection to the site. Check the Wi-Fi and try again.".into()
    } else if err.contains("read failed") {
        "The page is too large to open on this device.".into()
    } else {
        "The page could not be loaded.".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_words_become_searches_and_dotted_names_get_https() {
        assert_eq!(normalize("https://a.org/x"), "https://a.org/x");
        assert_eq!(normalize("kbp://home"), "kbp://home");
        assert_eq!(normalize("example.com"), "https://example.com");
        assert!(normalize("kindle browser").starts_with("https://lite.duckduckgo.com/lite/?q=kindle%20browser"));
        assert!(normalize("kindle").starts_with("https://lite.duckduckgo.com/lite/?q=kindle"));
    }

    #[test]
    fn url_splits_into_host_and_path_for_the_address_bar() {
        let (host, path) = split_url("https://truyendich.space/doc-truyen/x?y=1");
        assert_eq!(host, "truyendich.space");
        assert_eq!(path, "/doc-truyen/x?y=1");
        assert_eq!(split_url("https://example.org"), ("example.org".to_string(), String::new()));
    }

    #[test]
    fn page_css_goes_into_the_head_so_standards_mode_is_kept() {
        let settings = Settings { text_pct: 125, night: false };
        let out = prepare_html("<!doctype html><html><head><title>t</title></head><body></body></html>", &settings);
        assert!(out.starts_with("<!doctype html><html><head><style>"));
        assert!(out.contains("zoom:125%"));
        let plain = prepare_html("<!doctype html><p>x</p>", &Settings { text_pct: 100, night: false });
        assert!(plain.starts_with("<!doctype html><style>"));
    }

    #[test]
    fn encoded_png_has_the_right_size_and_signature() {
        let gray = vec![200u8; 4 * 3];
        let png = encode_png(4, 3, &gray).expect("encode");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 33);
    }

    #[test]
    fn percent_encoding_leaves_unreserved_characters_alone() {
        assert_eq!(percent("a b/c-d_e.f~g"), "a%20b%2Fc-d_e.f~g");
    }
}
