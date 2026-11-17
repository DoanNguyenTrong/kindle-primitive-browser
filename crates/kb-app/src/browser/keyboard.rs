//! On-screen keyboard and address field: key layers, typing, the caret, and address suggestions.

use super::*;
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    Lower,
    Upper,
    Symbols,
    /// Less common symbols and currency signs (the "#+=" layer).
    More,
}

pub(crate) struct Key {
    pub(crate) label: String,
    pub(crate) href: String,
    pub(crate) width: f32,
    pub(crate) class: &'static str,
}

pub(super) fn key(label: &str, href: &str, width: f32) -> Key {
    Key {
        label: label.to_string(),
        href: href.to_string(),
        width,
        class: "",
    }
}

pub(super) fn char_key(c: char) -> Key {
    key(&c.to_string(), &format!("kb:c:{}", c as u32), 1.0)
}

pub(crate) fn keyboard_rows(layer: Layer, go: &str) -> Vec<Vec<Key>> {
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
            Key {
                class: "go",
                ..key(go, "kb:go", 1.5)
            },
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
                bottom(key("#+=", "kb:more", 1.5)),
            ]
        }
        Layer::More => {
            let mut row4 = vec![key("ABC", "kb:abc", 1.5)];
            row4.extend("@&$".chars().map(char_key));
            row4.push(key("Del", "kb:del", 1.5));
            vec![
                "[]{}#%^*+=".chars().map(char_key).collect(),
                "_\\|~<>€£¥•".chars().map(char_key).collect(),
                ".,?!'\"/:;".chars().map(char_key).collect(),
                row4,
                bottom(key("123", "kb:sym", 1.5)),
            ]
        }
    }
}

impl Browser {
    /// Drop a selection before an edit: the selected text is replaced, not appended to.
    fn replace_selection(&mut self) {
        if self.select_all {
            self.typed.clear();
            self.caret = 0;
            self.select_all = false;
        }
    }

    pub(super) fn enter_keyboard(&mut self) {
        self.password_entry = false;
        self.find_entry = false;
        self.masked = false;
        self.keyboard = true;
        self.sheet = None;
        self.input_target = None;
        let is_home = self.url.starts_with("kbp:");
        self.typed = if is_home {
            Vec::new()
        } else {
            self.url.chars().collect()
        };
        self.caret = self.typed.len();
        self.select_all = !is_home;
        self.layer = Layer::Lower;
        self.refresh_suggestions();
        self.touch(0, self.height, Wave::Text);
    }

    pub(super) fn keyboard_action(&mut self, href: &str) {
        // A suggestion row sits in the field area but is a browser action: open it.
        if href.starts_with("kb:sugg:") {
            self.chrome_action(href);
            return;
        }
        let mut field_changed = true;
        let mut layer_changed = false;
        if let Some(code) = href
            .strip_prefix("kb:c:")
            .and_then(|s| s.parse::<u32>().ok())
        {
            self.replace_selection();
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
                    self.layer = if self.layer == Layer::Upper {
                        Layer::Lower
                    } else {
                        Layer::Upper
                    };
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:sym" => {
                    self.layer = Layer::Symbols;
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:more" => {
                    self.layer = Layer::More;
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:abc" => {
                    self.layer = Layer::Lower;
                    layer_changed = true;
                    field_changed = false;
                }
                "kb:del" => {
                    if self.select_all {
                        self.replace_selection();
                    } else if self.caret > 0 {
                        self.caret -= 1;
                        self.typed.remove(self.caret);
                    }
                }
                "kb:left" => {
                    self.select_all = false;
                    self.caret = self.caret.saturating_sub(1);
                }
                "kb:right" => {
                    self.select_all = false;
                    self.caret = (self.caret + 1).min(self.typed.len());
                }
                "kb:clear" => {
                    self.typed.clear();
                    self.caret = 0;
                    self.select_all = false;
                }
                "kb:cancel" => {
                    self.keyboard = false;
                    self.input_target = None;
                    self.masked = false;
                    self.password_entry = false;
                    self.find_entry = false;
                    self.typed.clear();
                    self.touch(0, self.height, Wave::Text);
                    return;
                }
                "kb:go" => {
                    let text: String = self.typed.iter().collect();
                    self.keyboard = false;
                    if self.password_entry {
                        self.password_entry = false;
                        self.typed.clear();
                        self.set_ssh_password(&text);
                        return;
                    }
                    if self.find_entry {
                        self.find_entry = false;
                        self.run_find(&text);
                        return;
                    }
                    if let Some(key) = self.input_target.take() {
                        // Done: write the value back into the field. The form is sent by its own submit button.
                        self.masked = false;
                        if let Some(page) = self.page.as_mut() {
                            page.set_input_value(key, &text);
                        }
                        self.mark_page_dirty();
                        self.touch(0, self.height, Wave::Text);
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
            self.touch(0, self.kb_y(), Wave::Text);
        }
        if layer_changed {
            self.touch(self.kb_y(), self.height, Wave::Text);
        }
    }

    /// Bookmarks first, then recent pages, that match the typed text. Every match is kept up to
    /// `SUGGESTION_LIMIT`; the list scrolls, so a long history stays reachable.
    pub(super) fn refresh_suggestions(&mut self) {
        const SUGGESTION_LIMIT: usize = 60;
        if self.password_entry || self.find_entry {
            self.suggestions.clear();
            return;
        }
        // A selected address is not a query yet: list the bookmarks and recent pages instead.
        let q: String = if self.select_all {
            String::new()
        } else {
            self.typed.iter().collect::<String>().to_lowercase()
        };
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut out: Vec<(String, String, bool)> = Vec::new();
        for (list, bookmarked) in [(&self.bookmarks, true), (&self.visits, false)] {
            for (u, t) in list.iter() {
                if out.len() >= SUGGESTION_LIMIT {
                    break;
                }
                let hit = q.is_empty() || u.to_lowercase().contains(&q) || t.to_lowercase().contains(&q);
                if hit && seen.insert(u.as_str()) {
                    out.push((u.clone(), t.clone(), bookmarked));
                }
            }
        }
        self.suggestions = out;
    }

    /// The address bar layer, shown above the suggestions while the keyboard is open.
    pub(super) fn ensure_field(&mut self) -> &mut Cached {
        // A password is shown as stars, and its text never becomes part of the cache key.
        let (shown, key): (Vec<char>, String) = if self.password_entry || self.masked {
            let stars = vec!['*'; self.typed.len()];
            let key = format!("pw:{}|{}|{}", self.typed.len(), self.caret, self.select_all);
            (stars, key)
        } else {
            let typed: String = self.typed.iter().collect();
            (self.typed.clone(), format!("{typed}|{}|{}", self.caret, self.select_all))
        };
        if self.field.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::field_bar(&shown, self.caret, self.select_all);
            let mut c = self.render_layer(&html, "kb://keyboard/", self.width, TOP_H);
            c.key = key;
            self.field = Some(c);
        }
        self.field.as_mut().expect("field layer")
    }

    /// The suggestion list layer, between the address bar and the keys. It keeps its scroll
    /// position until the suggestions change.
    pub(super) fn ensure_suggestions(&mut self) -> &mut Cached {
        let key: String = self.suggestions.iter().map(|(u, _, _)| u.as_str()).collect::<Vec<_>>().join("\n");
        if self.suggest.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::field_list(&self.suggestions);
            let mut c = self.render_layer(&html, "kb://keyboard/", self.width, self.kb_y() - TOP_H);
            c.key = key;
            self.suggest = Some(c);
        }
        self.suggest.as_mut().expect("suggestion layer")
    }

    /// What the Go key says: Done ends a page field (its form is not submitted), Set ends the SSH password.
    pub(super) fn go_label(&self) -> &'static str {
        if self.password_entry {
            "Set"
        } else if self.find_entry {
            "Find"
        } else if self.input_target.is_some() {
            "Done"
        } else {
            "Go"
        }
    }

    pub(super) fn ensure_key(&mut self, layer: Layer) -> &mut Cached {
        let label = self.go_label();
        if self.keys_label != label {
            // The key layers were drawn with another label: draw them again.
            self.keys = [None, None, None, None];
            self.keys_label = label;
        }
        let idx = layer as usize;
        if self.keys[idx].is_none() {
            let html = ui::keyboard_for(layer, self.go_label(), self.height - self.kb_y());
            let c = self.render_layer(&html, "kb://keyboard/", self.width, self.height - self.kb_y());
            self.keys[idx] = Some(c);
        }
        self.keys[idx].as_mut().expect("key layer")
    }
}
