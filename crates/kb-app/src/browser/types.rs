//! Types shared by the browser: drawing updates, the cached layers, page and tab state, sheets.

use kb_engine::Page;

pub type Pump = Box<dyn FnMut(f32, &str) -> bool>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Wave {
    /// Fast, low-detail (DU). Bars, keyboard, progress.
    Fast,
    /// Text-quality (GL16). Page content.
    Text,
    /// Full flash (GC16). Clears ghosting.
    Flash,
}

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

pub(super) struct Cached {
    pub(super) key: String,
    pub(super) page: Page,
    pub(super) gray: Vec<u8>,
}

pub(super) struct PreviousPage {
    pub(super) url: String,
    pub(super) page: Page,
    pub(super) source: String,
    pub(super) reader_source: Option<String>,
    pub(super) is_web: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Sheet {
    Menu,
    Tabs,
    Display,
}

#[derive(Clone, Default, Debug)]
pub(super) struct TabSave {
    pub(super) history: Vec<String>,
    pub(super) scrolls: Vec<f64>,
    pub(super) idx: usize,
    pub(super) url: String,
    pub(super) title: String,
}

impl TabSave {
    pub(super) fn empty() -> TabSave {
        TabSave::default()
    }
}
