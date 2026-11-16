//! Composing the screen: bars, sheets, notices and the keyboard, the cached layers, the notice
//! timer, and PNG output.

use super::*;
use crate::ui;

pub(super) fn blit(dst: &mut [u8], width: u32, y: u32, src: &[u8], rows: u32) {
    let w = width as usize;
    let start = y as usize * w;
    let n = (rows as usize * w)
        .min(src.len())
        .min(dst.len().saturating_sub(start));
    dst[start..start + n].copy_from_slice(&src[..n]);
}

pub(super) fn progress_strip(dst: &mut [u8], width: u32, y: u32, fraction: f32) {
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

impl Browser {
    /// Top of the keyboard's keys on this screen.
    pub(super) fn kb_y(&self) -> u32 {
        kb_top(self.height)
    }

    /// Called regularly by the driver (on idle). Brings in held-back images and
    /// clears a notice when its time is up.
    pub fn tick(&mut self) -> Option<Update> {
        let done = self
            .net
            .stats()
            .deferred_done
            .load(std::sync::atomic::Ordering::Relaxed);
        // Images trickle in one by one. Repaint at most every 1.5 s so the page doesn't flash.
        if done != self.images_seen
            && self.last_image_refresh.elapsed() >= Duration::from_millis(1500)
        {
            self.images_seen = done;
            self.last_image_refresh = Instant::now();
            // The last image arriving ends the load: refresh the whole screen once, cleanly.
            let wave = if self.images_loading() { Wave::Text } else { Wave::Flash };
            self.touch(0, self.height, wave);
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
        self.toast = Some((
            message.to_string(),
            Instant::now() + Duration::from_millis(2200),
        ));
        let (y0, y1) = toast_rows(self.height);
        self.touch(y0, y1, Wave::Fast);
    }

    /// Whether a notice is on screen (drivers keep ticking until it clears).
    pub fn notice_showing(&self) -> bool {
        self.toast.is_some()
    }

    pub(super) fn touch(&mut self, y0: u32, y1: u32, wave: Wave) {
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
        let mut gray = if self.keyboard {
            self.compose_keyboard()
        } else {
            self.compose_browse()
        };
        if self.settings.night {
            for px in gray.iter_mut() {
                *px = 255 - *px;
            }
        }
        self.last_gray = gray.clone();
        Update {
            gray,
            width: self.width,
            height: self.height,
            rows,
            wave,
        }
    }

    /// Push the current screen to the draw sink now (used while a load is still running).
    pub(super) fn emit_frame(&mut self) {
        if self.draw.is_none() {
            return;
        }
        let update = self.frame();
        if let Some(draw) = self.draw.as_mut() {
            draw(update);
        }
    }

    pub(super) fn compose_browse(&mut self) -> Vec<u8> {
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
            blit(
                &mut out,
                w,
                h - MENU_H + SHEET_TITLE_H,
                &layer,
                MENU_H - SHEET_TITLE_H,
            );
            let header = self.ensure_sheet_header(sheet).gray.clone();
            blit(&mut out, w, h - MENU_H, &header, SHEET_TITLE_H);
        } else if self.chrome {
            let top = self.ensure_top().gray.clone();
            blit(&mut out, w, 0, &top, TOP_H);
            let bottom = self.ensure_bottom().gray.clone();
            blit(&mut out, w, h - BOT_H, &bottom, BOT_H);
        }
        // The quick settings panel drops from the top edge over the bars and the page.
        if self.quick && !self.keyboard {
            let panel = self.ensure_quick().gray.clone();
            blit(&mut out, w, 0, &panel, QUICK_H);
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

    pub(super) fn toast_layer(&mut self, message: &str) -> Vec<u8> {
        if self
            .toast_cache
            .as_ref()
            .map_or(true, |(m, _)| m != message)
        {
            let html = ui::toast(message);
            let mut page = Page::new(
                &html,
                "kb://chrome/",
                self.width,
                88,
                self.net.clone(),
                &mut || true,
            );
            let gray = page.render_gray();
            self.toast_cache = Some((message.to_string(), gray));
        }
        self.toast_cache
            .as_ref()
            .map(|(_, g)| g.clone())
            .unwrap_or_default()
    }

    pub(super) fn compose_keyboard(&mut self) -> Vec<u8> {
        let (w, h) = (self.width, self.height);
        let mut out = vec![255u8; (w * h) as usize];
        // Address bar on top (the same height as the page's bar), suggestions, then the keys.
        let bar = self.ensure_field().gray.clone();
        blit(&mut out, w, 0, &bar, TOP_H);
        let list = self.ensure_suggestions().gray.clone();
        blit(&mut out, w, TOP_H, &list, self.kb_y() - TOP_H);
        let keys = self.ensure_key(self.layer).gray.clone();
        blit(&mut out, w, self.kb_y(), &keys, h - self.kb_y());
        out
    }

    pub(super) fn render_layer(&self, html: &str, base: &str, w: u32, h: u32) -> Cached {
        let mut page = Page::new(html, base, w, h, self.net.clone(), &mut || true);
        let gray = page.render_gray();
        Cached {
            key: String::new(),
            page,
            gray,
        }
    }

    pub(super) fn ensure_top(&mut self) -> &mut Cached {
        let key = format!("{}|{}", self.url, self.can_go_back());
        if self.top.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::top(&self.url);
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, TOP_H);
            c.key = key;
            self.top = Some(c);
        }
        self.top.as_mut().expect("top layer")
    }

    pub(super) fn ensure_bottom(&mut self) -> &mut Cached {
        let marked = self.bookmarks.iter().any(|(u, _)| *u == self.url);
        let key = format!(
            "{}{}{}{}|{}",
            self.can_go_back() as u8,
            self.can_go_forward() as u8,
            marked as u8,
            self.reader as u8,
            self.tabs.len().max(1)
        );
        if self.bottom.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::bottom(
                self.can_go_back(),
                self.can_go_forward(),
                marked,
                self.tabs.len().max(1),
            );
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, BOT_H);
            c.key = key;
            self.bottom = Some(c);
        }
        self.bottom.as_mut().expect("bottom layer")
    }

    /// The quick settings panel, rendered again only when a device value changes.
    pub(super) fn ensure_quick(&mut self) -> &mut Cached {
        let key = format!("{:?}", self.device_status);
        if self.quick_layer.as_ref().map_or(true, |c| c.key != key) {
            let html = ui::quick_panel(&self.device_status);
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, QUICK_H);
            c.key = key;
            self.quick_layer = Some(c);
        }
        self.quick_layer.as_mut().expect("quick layer")
    }

    pub(super) fn show_sheet(&mut self, sheet: Sheet) {
        self.sheet = Some(sheet);
        self.touch(self.height - MENU_H, self.height, Wave::Text);
    }

    /// The menu or tab sheet, rendered once per state.
    pub(super) fn ensure_sheet(&mut self, sheet: Sheet) -> &mut Cached {
        let cache_mb = store::cache_size() / (1024 * 1024);
        let marked = self.bookmarks.iter().any(|(u, _)| *u == self.url);
        let tabs: Vec<(String, String)> = self.tab_labels();
        let key = match sheet {
            Sheet::Menu => format!(
                "menu|{}|{}|{}|{}|{}|{}|{}|{}",
                self.reader,
                cache_mb,
                marked,
                self.url,
                self.settings.night,
                self.settings.text_pct,
                self.settings.page_width,
                self.settings.images
            ),
            Sheet::Tabs => format!("tabs|{}|{:?}", self.active, tabs),
            Sheet::Display => format!(
                "display|{}|{}|{}",
                self.settings.text_pct, self.settings.page_width, self.settings.night
            ),
        };
        if self.sheet_layer.as_ref().map_or(true, |c| c.key != key) {
            let html = match sheet {
                Sheet::Menu => ui::menu(
                    self.reader,
                    cache_mb,
                    marked,
                    &self.display_summary(),
                    self.settings.images,
                ),
                Sheet::Tabs => ui::tabs(&tabs, self.active),
                Sheet::Display => ui::display(
                    self.settings.text_pct,
                    self.settings.page_width,
                    self.settings.night,
                ),
            };
            let mut c =
                self.render_layer(&html, "kb://chrome/", self.width, MENU_H - SHEET_TITLE_H);
            c.key = key;
            self.sheet_layer = Some(c);
        }
        self.sheet_layer.as_mut().expect("sheet layer")
    }

    pub(super) fn ensure_sheet_header(&mut self, sheet: Sheet) -> &mut Cached {
        let title = match sheet {
            Sheet::Menu => crate::brand::menu_title(),
            Sheet::Tabs => format!(
                "{} open {}",
                self.tabs.len(),
                if self.tabs.len() == 1 { "tab" } else { "tabs" }
            ),
            Sheet::Display => "Display".to_string(),
        };
        if self.sheet_header.as_ref().map_or(true, |c| c.key != title) {
            // Only the menu has a control in its title strip (Exit). Other sheets close by a tap on
            // the page above them.
            let controls = if sheet == Sheet::Menu { ui::diagnostics_button() + &ui::exit_button() } else { String::new() };
            let html = ui::sheet_header(&title, &controls);
            let mut c = self.render_layer(&html, "kb://chrome/", self.width, SHEET_TITLE_H);
            c.key = title;
            self.sheet_header = Some(c);
        }
        self.sheet_header.as_mut().expect("sheet header")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_png_has_the_right_size_and_signature() {
        let gray = vec![200u8; 4 * 3];
        let png = encode_png(4, 3, &gray).expect("encode");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 33);
    }
}
