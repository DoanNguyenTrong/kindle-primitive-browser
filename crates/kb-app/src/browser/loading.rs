//! Loading documents: fetching and building a page, forms (POST), fragments, reader view, and
//! recording visits.

use super::*;
use crate::ui;

pub(super) fn load_reason(err: &str) -> String {
    if err.contains("HTTP 404") {
        "The address was not found on the server (404).".into()
    } else if err.contains("HTTP") {
        format!(
            "The server answered with an error ({}).",
            err.trim_start_matches("HTTP ")
        )
    } else if err.contains("request failed") {
        "No connection to the site. Check the Wi-Fi and try again.".into()
    } else if err.contains("resolves to this device") {
        "The device sends this site to its own address (a hosts-file or DNS override), so it cannot be reached. Removing that override restores access.".into()
    } else if err.contains("read failed") {
        "The page is too large to open on this device.".into()
    } else {
        "The page could not be loaded.".into()
    }
}

impl Browser {
    /// POST a form and show the response, with the same progress and cancel behaviour as a GET.
    pub(super) fn post_document(
        &mut self,
        url: &str,
        fields: Vec<(String, String)>,
    ) -> Result<(), Cancelled> {
        self.url = url.to_string();
        self.keyboard = false;
        self.sheet = None;
        self.chrome = true;
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
                self.build_page(&html, &f.url, true);
                self.record_web_visit();
            }
            Err(e) => {
                eprintln!("[post] {url}: {e}");
                let html = ui::error("Could not load this page", &load_reason(&e), &e);
                self.build_page(&html, url, false);
            }
        }
        self.touch(0, self.height, Wave::Text);
        Ok(())
    }

    /// Load `url` into the page. Shows progress through the pump while waiting.
    pub(super) fn load(&mut self, url: &str, policy: Policy) -> Result<(), Cancelled> {
        self.url = url.to_string();
        self.keyboard = false;
        self.sheet = None;
        self.chrome = true;
        self.page_dirty = true;
        // Show the address bar with the new URL (and the progress strip under it) before waiting,
        // even when the bars were hidden by scrolling. Otherwise the strip lands on page content.
        self.touch(0, self.height, Wave::Text);
        self.emit_frame();

        let (html, base, is_web) = if let Some(rest) = url.strip_prefix("kbp://") {
            // Saved pages resolve relative links against their original address.
            let base = rest
                .strip_prefix("saved/")
                .and_then(|key| {
                    read_pairs(&data_file("saved.tsv"))
                        .into_iter()
                        .find(|(k, _)| k == key)
                })
                .map(|(_, meta)| split_saved_meta(&meta).0.to_string())
                .unwrap_or_else(|| url.to_string());
            (self.internal_html(rest), base, false)
        } else {
            match self.fetch_with_progress(url, policy)? {
                Ok(mut f) => {
                    // HTTP does not send the fragment; keep it when the response URI omits it.
                    if let (Ok(requested), Ok(mut final_url)) =
                        (url::Url::parse(url), url::Url::parse(&f.url))
                    {
                        if final_url.fragment().is_none() {
                            final_url.set_fragment(requested.fragment());
                            f.url = final_url.to_string();
                        }
                    }
                    self.url = f.url.clone();
                    if let Some(last) = self.history.get_mut(self.idx) {
                        *last = f.url.clone();
                    }
                    let html = if !f.content_type.is_empty() && !f.content_type.contains("html") {
                        ui::error(
                            "Not a web page",
                            "This link opens a file that is not a web page.",
                            &f.content_type,
                        )
                    } else {
                        String::from_utf8_lossy(&f.body).into_owned()
                    };
                    (html, f.url, true)
                }
                Err(e) => {
                    eprintln!("[load] {url}: {e}");
                    (
                        ui::error("Could not load this page", &load_reason(&e), &e),
                        url.to_string(),
                        false,
                    )
                }
            }
        };
        self.build_page(&html, &base, is_web);
        if is_web {
            self.record_web_visit();
            self.pending_nav = crate::html::meta_refresh_target(&html, &base);
        }
        if let Some(fragment) = url::Url::parse(&self.url)
            .ok()
            .and_then(|u| u.fragment().map(str::to_owned))
        {
            if let Some(page) = self.page.as_mut() {
                page.scroll_to_fragment(&fragment);
            }
        }
        self.follow_script_navigation();
        self.save_session();
        // A finished page gets one full refresh: the panel then shows the whole page cleanly.
        self.touch(0, self.height, Wave::Flash);
        Ok(())
    }

    /// Build the page, waiting for resources with progress. Images are deferred.
    pub(super) fn build_page(&mut self, html: &str, base: &str, is_web: bool) {
        let source = html.to_string();
        let prepared = prepare_html(html, &self.settings);
        // Stylesheets for print or the dark scheme are not applied to this screen.
        let prepared = if is_web { crate::html::drop_unmatched_stylesheet_links(&prepared) } else { prepared };
        let html = prepared.as_str();
        let stats = self.net.stats();
        let mut pump = self.pump.take();
        self.net.set_defer_images(true);
        let (w, h) = (self.width, self.height);
        // Web pages lay out at a phone width; internal pages and chrome keep device pixels.
        let css_width = if is_web {
            Some(self.settings.page_width as f32)
        } else {
            None
        };
        let net = self.net.clone();
        // The browser's own pages (home with its logo) always show their images.
        net.set_images_allowed(self.settings.images || !is_web);
        let built = {
            let mut waiting = || {
                let req = stats.requested.load(std::sync::atomic::Ordering::Relaxed);
                let done = stats.done.load(std::sync::atomic::Ordering::Relaxed);
                let frac = if req == 0 {
                    0.3
                } else {
                    0.3 + 0.6 * done as f32 / req as f32
                };
                match pump.as_mut() {
                    Some(p) => p(frac, "Loading"),
                    None => true,
                }
            };
            std::panic::catch_unwind(AssertUnwindSafe(|| {
                Page::new_scaled(html, base, w, h, net.clone(), &mut waiting, css_width)
            }))
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
        self.stash_current(&self.url.clone());
        self.source = source;
        self.reader_source = None;
        self.reader = false;
        self.page_is_web = is_web;
        self.page = Some(page);
    }

    /// A script asked for another address (`location.href = …`, `location.replace()`), or the page is a
    /// meta-refresh redirect: load that address in place of this one. Returns true when it did. Up to
    /// five hops, so a redirect loop stops. A fragment-only change moves within the page instead.
    pub(super) fn follow_script_navigation(&mut self) -> bool {
        if let Some(next) = self.page.as_mut().and_then(|p| p.take_navigation()) {
            self.pending_nav = Some(next);
        }
        let Some(next) = self.pending_nav.take() else { return false };
        if self.redirect_depth >= 5 {
            return false;
        }
        if same_document(&self.url, &next) && self.url != next {
            self.show_fragment(&next);
            return false;
        }
        self.redirect_depth += 1;
        let _ = self.load(&next, Policy::Network);
        self.redirect_depth -= 1;
        true
    }

    /// Record a web page the reader navigated to (history and cookies). Rebuilds such as
    /// reflow or reader view lay the page out again without recording it a second time.
    pub(super) fn record_web_visit(&mut self) {
        let url = self.url.clone();
        let title = self.title().to_string();
        self.record_visit(&url, &title);
        kb_net::save_cookies();
    }

    /// Fetch a document on a worker thread so the UI can report progress and
    /// accept taps (a tap cancels the load).
    pub(super) fn fetch_with_progress(
        &mut self,
        url: &str,
        policy: Policy,
    ) -> Result<Result<kb_net::Fetched, String>, Cancelled> {
        let u = url.to_string();
        self.run_job(move || kb_net::load(&u, MAX_DOC_BYTES, policy))
    }

    /// Run a document job (GET, POST) on a worker thread, reporting progress meanwhile.
    pub(super) fn run_job(
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

    pub(super) fn toggle_reader(&mut self) {
        if self.reader {
            let url = self.url.clone();
            let _ = self.load(&url, Policy::Cache(Duration::from_secs(60 * 60 * 24 * 7)));
            return;
        }
        let Some(html) = self.page.as_ref().and_then(|p| p.reader_html()) else {
            return;
        };
        let base = self.url.clone();
        let source = self.source.clone();
        // The reader view is article markup from a web page: lay it out at the chosen width.
        self.build_page(&html, &base, true);
        self.source = source;
        self.reader_source = Some(html);
        self.reader = true;
        self.touch(0, self.height, Wave::Text);
    }
}
