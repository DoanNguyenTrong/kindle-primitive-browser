//! Internal pages (`kbp://home`, `kbp://bookmarks`, `kbp://saved`, `kbp://diagnostics`) built from browser data.

use super::*;
use crate::ui;

impl Browser {
    pub(super) fn internal_html(&self, rest: &str) -> String {
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
            "diagnostics" => ui::diagnostics(self.diagnostics.as_deref(), self.fix_report.as_ref(), super::diagnostics::ssh_running()),
            "saved" => {
                let items: Vec<ui::ListItem> = read_pairs(&data_file("saved.tsv"))
                    .iter()
                    .map(|(key, meta)| {
                        let (url, title) = split_saved_meta(meta);
                        let title = if title.is_empty() { url } else { title };
                        ui::ListItem {
                            href: format!("kbp://saved/{key}"),
                            label: title.to_string(),
                            sub: url.to_string(),
                            action: None,
                        }
                    })
                    .collect();
                ui::list(
                    "Saved pages",
                    &items,
                    "No saved pages yet. Open Menu and tap Save for offline.",
                )
            }
            _ => ui::error("Page not found", "This browser page does not exist.", rest),
        }
    }

    pub(super) fn saved_source(&self, key: &str) -> String {
        if !key.chars().all(|c| c.is_ascii_hexdigit()) {
            return ui::error("Saved page", "This link is not valid.", "");
        }
        fs::read_to_string(data_file("saved").join(format!("{key}.html"))).unwrap_or_else(|_| {
            ui::error(
                "Saved page",
                "This saved copy is no longer on the device.",
                "",
            )
        })
    }
}
