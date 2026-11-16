//! The browser: state and its public API. Behaviour is split by responsibility across the
//! submodules, each holding part of `impl Browser`.

use std::{
    fs,
    panic::AssertUnwindSafe,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

use kb_engine::Page;
use kb_net::{store, Policy, SyncNet};

use crate::addr::*;
use crate::html::*;
use crate::layout::*;
use crate::settings::*;
use crate::storage::*;

mod actions;
mod driver;
mod frame;
mod input;
mod internal;
mod keyboard;
mod library;
mod loading;
mod nav;
mod tabs;
mod types;

use actions::stays_open;
use types::{Cached, PreviousPage, Sheet, TabSave};

pub use frame::encode_png;
pub(crate) use keyboard::{keyboard_rows, Key, Layer};
pub use types::{Cancelled, Pump, Update, Wave};

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
    reader_source: Option<String>,
    page_is_web: bool,
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
    previous: Option<PreviousPage>,
    /// URL of the page on screen (to tell it apart from the previous one).
    page_url: String,
    toast: Option<(String, Instant)>,
    toast_cache: Option<(String, Vec<u8>)>,

    keyboard: bool,
    typed: Vec<char>,
    caret: usize,
    /// The whole address is selected (set when the address bar opens on a page): the next
    /// key replaces it, and the suggestions show the bookmarks and recent pages.
    select_all: bool,
    layer: Layer,
    /// Address suggestions: (url, title, is_bookmark). Bookmarks are listed first.
    suggestions: Vec<(String, String, bool)>,
    field: Option<Cached>,
    suggest: Option<Cached>,
    keys: [Option<Cached>; 3],

    top: Option<Cached>,
    bottom: Option<Cached>,
    sheet_layer: Option<Cached>,
    sheet_header: Option<Cached>,

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
            reader_source: None,
            page_is_web: false,
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
            select_all: false,
            layer: Layer::Lower,
            suggestions: Vec::new(),
            field: None,
            suggest: None,
            keys: [None, None, None],
            top: None,
            bottom: None,
            sheet_layer: None,
            sheet_header: None,
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

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
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
}
