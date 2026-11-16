//! Reading settings: text size, page width, night mode, images and the home screen layout.
//! Stored in `settings.tsv`.

use crate::storage::{data_file, read_pairs, write_pairs};

pub const PAGE_WIDTHS: [u32; 3] = [412, 600, 800];

pub(crate) const DEFAULT_PAGE_WIDTH: u32 = 600;

pub(crate) const TEXT_STEPS: [u32; 8] = [80, 90, 100, 110, 125, 150, 175, 200];

pub fn width_name(width: u32) -> &'static str {
    match width {
        412 => "Narrow",
        800 => "Wide",
        _ => "Medium",
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) text_pct: u32,
    pub(crate) night: bool,
    /// CSS width web pages are laid out at (one of `PAGE_WIDTHS`).
    pub(crate) page_width: u32,
    pub(crate) home_bookmarks_open: bool,
    pub(crate) home_recent_open: bool,
    /// Whether images are fetched and drawn. Off saves memory and bandwidth for text reading.
    pub(crate) images: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            text_pct: 100,
            night: false,
            page_width: DEFAULT_PAGE_WIDTH,
            home_bookmarks_open: true,
            home_recent_open: true,
            images: true,
        }
    }
}

impl Settings {
    pub(crate) fn load() -> Settings {
        let pairs = read_pairs(&data_file("settings.tsv"));
        let get = |k: &str| {
            pairs
                .iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
        };
        let flag = |k: &str, default: bool| get(k).map_or(default, |v| v == "1");
        let width = get("page_width")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_PAGE_WIDTH);
        Settings {
            text_pct: get("text_pct").and_then(|v| v.parse().ok()).unwrap_or(100),
            night: flag("night", false),
            page_width: if PAGE_WIDTHS.contains(&width) {
                width
            } else {
                DEFAULT_PAGE_WIDTH
            },
            home_bookmarks_open: flag("home_bookmarks_open", true),
            home_recent_open: flag("home_recent_open", true),
            images: flag("images", true),
        }
    }

    pub(crate) fn save(&self) {
        let flag = |b: bool| if b { "1" } else { "0" }.to_string();
        write_pairs(
            &data_file("settings.tsv"),
            &[
                ("text_pct".to_string(), self.text_pct.to_string()),
                ("night".to_string(), flag(self.night)),
                ("page_width".to_string(), self.page_width.to_string()),
                (
                    "home_bookmarks_open".to_string(),
                    flag(self.home_bookmarks_open),
                ),
                ("home_recent_open".to_string(), flag(self.home_recent_open)),
                ("images".to_string(), flag(self.images)),
            ],
        );
    }
}
