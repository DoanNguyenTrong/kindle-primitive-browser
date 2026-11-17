//! Rendering engine: Blitz (Stylo CSS, Taffy layout, vello_cpu paint) plus
//! QuickJS page scripts, wrapped into a `Page` that scrolls, reports links,
//! handles clicks, and returns 8-bit grayscale frames for e-ink.

use std::{cell::RefCell, fs, rc::Rc, sync::Arc, thread, time::{Duration, Instant}};

use anyrender_vello_cpu::VelloCpuScenePainter;
use blitz_dom::{DocumentConfig, FontContext, NodeId};
use blitz_html::HtmlDocument;
use blitz_traits::{
    net::NetProvider,
    shell::{ColorScheme, Viewport},
};
use kb_js::{Click, Doc, HttpGet, Js};
use kb_net::{Policy, SyncNet};
use url::Url;

pub use kb_js::Click as ClickKind;

/// What a submitted form turns into.
/// A line of text on the page (`Page::text_lines`), in device pixels from the top of the document.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLine {
    /// Top and bottom of the glyph box (typographic ascent and descent; tall marks may reach a little past).
    pub top: f64,
    pub baseline: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submit {
    /// A GET request: navigate to this URL.
    Get(String),
    /// A POST request: send `fields` as form data to `url`.
    Post { url: String, fields: Vec<(String, String)> },
}

/// A text field the browser can edit: its node key, its current value, and whether it hides its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: u64,
    pub value: String,
    pub password: bool,
}

/// A `<select>` the reader can choose from: its key, a title, and its options as (label, selected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectField {
    pub key: u64,
    pub title: String,
    pub options: Vec<(String, bool)>,
}

/// The `<option>` elements of a select, in order (inside an `<optgroup>` too).
fn options_of(doc: &blitz_dom::BaseDocument, select: NodeId) -> Vec<NodeId> {
    let mut found = Vec::new();
    let Some(node) = doc.get_node(select) else { return found };
    for child in node.children.iter().copied() {
        let Some(c) = doc.get_node(child) else { continue };
        let Some(el) = c.element_data() else { continue };
        match &*el.name.local {
            "option" => found.push(child),
            "optgroup" => {
                for grand in c.children.iter().copied() {
                    if doc.get_node(grand).and_then(|g| g.element_data()).map_or(false, |e| &*e.name.local == "option") {
                        found.push(grand);
                    }
                }
            }
            _ => {}
        }
    }
    found
}

/// A local attribute name, for attributes the engine reads or writes itself.
fn local(name: &str) -> blitz_dom::QualName {
    blitz_dom::QualName::new(None, blitz_dom::Namespace::from(""), blitz_dom::LocalName::from(name))
}

/// Every radio button in the document whose name is `name`.
fn radios_named(doc: &blitz_dom::BaseDocument, name: &str) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut stack = vec![doc.root_node().id];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get_node(id) else { continue };
        if n.element_data().map_or(false, |e| &*e.name.local == "input")
            && n.attr(blitz_dom::local_name!("type")).map_or(false, |t| t.eq_ignore_ascii_case("radio"))
            && n.attr(blitz_dom::local_name!("name")) == Some(name)
        {
            found.push(id);
        }
        stack.extend(n.children.iter().copied());
    }
    found
}

/// Font files to load (explicit list: the Kindle has no system fonts and
/// fontconfig can't be cross-compiled). Families map onto CSS generics.
/// The browser's own copy of Noto (SIL Open Font License, see `assets/fonts/NOTO-LICENSE.txt`), installed with
/// the browser by `deploy/install-ssh.sh`. It does not depend on any other app on the Kindle.
pub const DEFAULT_FONTS: &[&str] = &[
    "/mnt/us/browser/fonts/NotoSans-Regular.ttf",
    "/mnt/us/browser/fonts/NotoSans-Bold.ttf",
    "/mnt/us/browser/fonts/NotoSans-Italic.ttf",
    "/mnt/us/browser/fonts/NotoSerif-Regular.ttf",
    "/mnt/us/browser/fonts/NotoSerif-Bold.ttf",
];

/// Largest script we will run.
const MAX_SCRIPT_BYTES: u64 = 2 * 1024 * 1024;

/// Fonts are read from `KB_FONTS` (comma list) when set, else `DEFAULT_FONTS`.
pub fn font_paths() -> Vec<String> {
    match std::env::var("KB_FONTS") {
        Ok(list) => list.split(',').map(str::to_owned).collect(),
        Err(_) => DEFAULT_FONTS.iter().map(|s| s.to_string()).collect(),
    }
}

thread_local! {
    static FONTS: RefCell<Option<FontContext>> = const { RefCell::new(None) };
}

/// Font collection, loaded from disk once per thread and cloned per page.
fn font_context() -> FontContext {
    FONTS.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                let mut ctx = FontContext::default();
                let existing: Vec<String> =
                    font_paths().into_iter().filter(|p| fs::metadata(p).is_ok()).collect();
                ctx.collection.load_fonts_from_paths(&existing);
                if let Some(id) = ctx.collection.family_id("Noto Sans") {
                    ctx.collection.set_generic_families(parley::GenericFamily::SansSerif, std::iter::once(id));
                    ctx.collection.set_generic_families(parley::GenericFamily::SystemUi, std::iter::once(id));
                }
                if let Some(id) = ctx.collection.family_id("Noto Serif") {
                    ctx.collection.set_generic_families(parley::GenericFamily::Serif, std::iter::once(id));
                }
                ctx
            })
            .clone()
    })
}

pub struct Page {
    doc: Doc,
    net: Arc<SyncNet>,
    js: Option<Js>,
    width: u32,
    height: u32,
    base: String,
    pub title: String,
    scroll_y: f64,
    /// Device pixels per CSS pixel. 1.0 for chrome and internal pages; web pages lay out at a phone width.
    scale: f32,
    content_h: f64,
    /// The text of password fields, by node key. The page shows stars instead, so the typed text is not
    /// drawn on the screen.
    secrets: std::collections::HashMap<u64, String>,
}

impl Page {
    /// Parse, run scripts, and lay out `html`. Sub-resources and scripts are
    /// fetched through the shared network layer.
    ///
    /// `pump` is called while waiting for resources. Returning `false` stops the
    /// wait and lays out whatever has arrived (used when the user taps during a load).
    pub fn new(
        html: &str,
        base_url: &str,
        width: u32,
        height: u32,
        net: Arc<SyncNet>,
        pump: &mut dyn FnMut() -> bool,
    ) -> Page {
        Page::new_scaled(html, base_url, width, height, net, pump, None)
    }

    /// Like `new`, but with a CSS layout width: the page is laid out at `css_width` CSS
    /// pixels and scaled up to fill `width` device pixels, as a phone browser does. Sites
    /// then pick their mobile layout (their `md:`/`lg:` rules see a phone-sized viewport).
    pub fn new_scaled(
        html: &str,
        base_url: &str,
        width: u32,
        height: u32,
        net: Arc<SyncNet>,
        pump: &mut dyn FnMut() -> bool,
        css_width: Option<f32>,
    ) -> Page {
        let scale = css_width.map_or(1.0, |cw| (width as f32 / cw).max(0.5));
        let page_net = net.clone();
        let config = DocumentConfig {
            viewport: Some(Viewport::new(width, height, scale, ColorScheme::Light)),
            font_ctx: Some(font_context()),
            base_url: Some(base_url.to_string()),
            net_provider: Some(net.clone() as Arc<dyn NetProvider>),
            // Without a parser, `innerHTML` assignments are silently dropped.
            html_parser_provider: Some(Arc::new(blitz_html::HtmlProvider) as Arc<dyn blitz_dom::HtmlParserProvider>),
            ..Default::default()
        };
        let doc: Doc = Rc::new(RefCell::new(HtmlDocument::from_html(html, config)));
        let mut page = Page {
            doc: doc.clone(),
            net: page_net,
            js: None,
            width,
            height,
            base: base_url.to_string(),
            title: extract_title(html).unwrap_or_else(|| base_url.to_string()),
            scroll_y: 0.0,
            scale,
            content_h: height as f64,
            secrets: std::collections::HashMap::new(),
        };
        // Each step is contained: a layout panic on one step costs that step, not the page.
        contain(|| page.settle(pump));
        contain(|| page.run_scripts());
        contain(|| page.settle(pump));
        // Streamed server markup: content sits in hidden S:n blocks that should replace
        // the B:n placeholders. Do it here too, in case the page's own swap script failed.
        contain(|| page.swap_streamed_chunks());
        contain(|| page.settle(pump));
        contain(|| page.label_selects());
        page.title = page.live_title().unwrap_or(page.title);
        page.content_h = contain(|| page.measure());
        // Painting culls against the document viewport, so widen it to the full
        // document height (scroll-invariant) and lay out once more. Canvas stays one screen.
        if page.content_h > height as f64 {
            let window = Viewport::new(width, page.content_h.ceil() as u32, scale, ColorScheme::Light);
            contain(|| page.doc.borrow_mut().set_viewport(window));
            contain(|| page.settle(&mut || true));
            page.content_h = contain(|| page.measure());
        }
        page
    }

    /// Move each hidden `<div id="S:n">` chunk into the `<template id="B:n">` slot that
    /// it belongs to, then remove both. Used by streamed server rendering.
    fn swap_streamed_chunks(&mut self) {
        let mut doc = self.doc.borrow_mut();
        let mut templates: std::collections::HashMap<String, NodeId> = std::collections::HashMap::new();
        let mut chunks: Vec<(String, NodeId)> = Vec::new();
        let mut stack = vec![doc.root_element().id];
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            stack.extend(node.children.iter().copied());
            let Some(el) = node.element_data() else { continue };
            let Some(eid) = node.attr(blitz_dom::local_name!("id")) else { continue };
            let tag: &str = &el.name.local;
            if tag == "template" && eid.starts_with("B:") {
                templates.insert(eid[2..].to_string(), id);
            } else if tag == "div" && eid.starts_with("S:") {
                chunks.push((eid[2..].to_string(), id));
            }
        }
        for (key, chunk) in chunks {
            let Some(&slot) = templates.get(&key) else { continue };
            let children = match doc.get_node(chunk) {
                Some(n) => n.children.clone(),
                None => continue,
            };
            let mut m = doc.mutate();
            if !children.is_empty() {
                m.insert_nodes_before(slot, &children);
            }
            m.remove_and_drop_node(slot);
            m.remove_and_drop_node(chunk);
        }
    }

    /// Text of the first element matching `selector` (for checks and debugging).
    pub fn text_of(&self, selector: &str) -> Option<String> {
        let doc = self.doc.borrow();
        let id = doc.query_selector(selector).ok().flatten()?;
        Some(doc.get_node(id)?.text_content())
    }

    /// Box (x, y, w, h) in device pixels of the first displayed element matching
    /// `selector`, relative to the page's top-left (scroll applied). For tests and taps.
    pub fn element_box(&self, selector: &str) -> Option<(f32, f32, f32, f32)> {
        let doc = self.doc.borrow();
        let ids = doc.query_selector_all(selector).ok()?;
        let k = self.scale;
        for id in ids {
            let node = doc.get_node(id)?;
            let size = node.final_layout().size;
            if size.width <= 0.0 || size.height <= 0.0 {
                continue;
            }
            let pos = node.absolute_position(0.0, 0.0);
            return Some((pos.x * k, pos.y * k - self.scroll_y as f32, size.width * k, size.height * k));
        }
        None
    }

    /// Number of matching elements that have a layout box (displayed), for checks.
    pub fn visible_count(&self, selector: &str) -> usize {
        let doc = self.doc.borrow();
        doc.query_selector_all(selector)
            .map(|ids| {
                ids.iter()
                    .filter(|id| {
                        doc.get_node(**id).map_or(false, |n| {
                            let size = n.final_layout().size;
                            size.width > 0.0 && size.height > 0.0
                        })
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Number of elements matching a CSS selector (for checks and debugging).
    pub fn count(&self, selector: &str) -> usize {
        self.doc.borrow().query_selector_all(selector).map(|v| v.len()).unwrap_or(0)
    }

    /// The `<title>` as it is now (scripts may have changed it).
    fn live_title(&self) -> Option<String> {
        let doc = self.doc.borrow();
        let id = doc.query_selector("title").ok().flatten()?;
        let text = doc.get_node(id)?.text_content();
        let text = text.trim();
        (!text.is_empty()).then(|| text.chars().take(120).collect())
    }

    /// Execute page scripts in document order, then run their timers.
    fn run_scripts(&mut self) {
        let scripts = self.script_nodes();
        if scripts.is_empty() {
            return;
        }
        let http: HttpGet = Rc::new(|method: &str, url: &str, flat: &[String], body: Option<String>| {
            let headers: Vec<(String, String)> = flat.chunks(2).filter(|p| p.len() == 2).map(|p| (p[0].clone(), p[1].clone())).collect();
            kb_net::request(method, url, &headers, body.as_deref(), MAX_SCRIPT_BYTES).ok().map(|r| {
                vec![r.status.to_string(), r.url, r.body, r.content_type]
            })
        });
        let js = match Js::new(self.doc.clone(), &self.base, http,
            self.width as f32 / self.scale, self.height as f32 / self.scale, self.scale) {
            Ok(js) => js,
            Err(e) => {
                eprintln!("[js] {e}");
                return;
            }
        };
        // External scripts are fetched together (a few at a time), then run in document order.
        let wanted: Vec<Option<String>> = scripts
            .iter()
            .map(|(_, src, kind, _)| {
                src.as_ref()
                    .filter(|_| kind_is_script(kind.as_deref()))
                    .and_then(|s| Url::parse(&self.base).and_then(|b| b.join(s)).ok())
                    .map(|u| u.to_string())
            })
            .collect();
        let fetched = fetch_scripts(&wanted);
        for ((inline, src, kind, node), result) in scripts.into_iter().zip(fetched) {
            let code = match (&src, inline) {
                (Some(_), _) => match result {
                    Some((u, Ok(f))) if f.content_type.to_ascii_lowercase().contains("html") => {
                        // A server can answer a script address with a page (an error or a login
                        // page). Running that as JavaScript only produces a syntax error.
                        eprintln!("[js] {u}: not a script ({})", f.content_type);
                        continue;
                    }
                    Some((_, Ok(f))) => String::from_utf8_lossy(&f.body).into_owned(),
                    Some((u, Err(e))) => {
                        eprintln!("[js] {u}: {e}");
                        continue;
                    }
                    None => continue,
                },
                (None, text) => text,
            };
            if !kind_is_script(kind.as_deref()) || code.trim().is_empty() {
                continue;
            }
            // While it runs, the script is `document.currentScript` (bundles read their own address from it).
            js.set_current_script(Some(node));
            js.run(&code, src.as_deref().unwrap_or("inline"));
            js.set_current_script(None);
        }
        js.run_timers();
        self.js = Some(js);
    }

    /// (inline text, src attribute, type attribute) for each script element, in order.
    fn script_nodes(&self) -> Vec<(String, Option<String>, Option<String>, u64)> {
        let doc = self.doc.borrow();
        let mut out = Vec::new();
        let mut stack = vec![doc.root_element().id];
        let mut ordered = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            if node.element_data().map_or(false, |e| &*e.name.local == "script") {
                ordered.push(id);
            }
            stack.extend(node.children.iter().rev().copied());
        }
        for id in ordered {
            let Some(node) = doc.get_node(id) else { continue };
            let src = node.attr(blitz_dom::local_name!("src")).map(str::to_string);
            let kind = node.attr(blitz_dom::local_name!("type")).map(str::to_string);
            out.push((node.text_content(), src, kind, id.as_u64()));
        }
        out
    }

    /// Drain sub-resource messages and restyle, waiting (bounded) for stylesheets.
    /// Stops early when `pump` returns false.
    fn settle(&mut self, pump: &mut dyn FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            {
                let mut doc = self.doc.borrow_mut();
                doc.handle_messages();
                doc.resolve(0.0);
                if !doc.has_pending_critical_resources() {
                    break;
                }
            }
            if Instant::now() >= deadline || !pump() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let mut doc = self.doc.borrow_mut();
        doc.handle_messages();
        doc.resolve(0.0);
    }

    /// Fetch the images that were held back while the text was laid out,
    /// ordered top-to-bottom so the ones the reader sees first arrive first.
    pub fn start_deferred_images(&self) {
        let order = self.image_order();
        self.net.release_ordered(&order);
    }

    /// Image sources in document order of their position on the page (top first).
    pub fn image_order(&self) -> Vec<String> {
        let doc = self.doc.borrow();
        let mut found: Vec<(f32, String)> = Vec::new();
        let mut stack = vec![doc.root_element().id];
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            stack.extend(node.children.iter().rev().copied());
            let Some(el) = node.element_data() else { continue };
            if &*el.name.local != "img" {
                continue;
            }
            let Some(src) = node.attr(blitz_dom::local_name!("src")) else { continue };
            let Some(url) = self.resolve_src(src) else { continue };
            found.push((node.absolute_position(0.0, 0.0).y, url));
        }
        found.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        found.into_iter().map(|(_, u)| u).collect()
    }

    fn resolve_src(&self, src: &str) -> Option<String> {
        let base = Url::parse(&self.base).ok()?;
        base.join(src).ok().map(|u| u.to_string())
    }

    /// Document height: the lowest bottom edge of any laid-out box. (Taffy's
    /// root scroll_height() doesn't see overflow from the body, so walk the tree.)
    fn measure(&self) -> f64 {
        let doc = self.doc.borrow();
        let root = doc.root_element().id;
        let s = self.scale as f64;
        let mut bottom = self.height as f64 / s;
        let mut stack = vec![(root, 0.0f64)];
        while let Some((id, parent_y)) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            // Text nodes carry no layout; their parent element's box covers them.
            if !node.is_element() {
                continue;
            }
            let layout = node.final_layout();
            let y = parent_y + layout.location.y as f64;
            bottom = bottom.max(y + layout.size.height as f64);
            stack.extend(node.children.iter().map(|&c| (c, y)));
        }
        bottom * s
    }

    /// Every line of text on the page as last laid out (after a paint), top to bottom, in device pixels from
    /// the top of the document: the
    /// glyph box (typographic ascent and descent) and the baseline. Lines of different boxes on the same row
    /// (columns) are one entry. The positions follow the layout tree as the painter does: each box from its
    /// layout parent, and an inline root's lines from its content box.
    pub fn text_lines(&self) -> Vec<TextLine> {
        let doc = self.doc.borrow();
        let s = self.scale as f64;
        let mut lines = Vec::new();
        let mut stack = vec![(doc.root_element().id, 0.0f64)];
        while let Some((id, parent_y)) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            if !node.is_element() && !node.flags.is_inline_root() {
                continue;
            }
            let layout = node.final_layout();
            let y = parent_y + layout.location.y as f64;
            if node.flags.is_inline_root() && layout.size.height > 0.0 {
                if let Some(text) = node.element_data().and_then(|e| e.inline_layout_data.as_ref()) {
                    // Line metrics are in layout units (CSS px times the layout's scale).
                    let unit = text.layout.scale().max(f32::EPSILON) as f64;
                    let origin = y + (layout.padding.top + layout.border.top) as f64 + text.block_offset as f64;
                    for line in text.layout.lines() {
                        let m = line.metrics();
                        if m.advance <= 0.0 {
                            continue;
                        }
                        lines.push(TextLine {
                            top: (origin + m.content_block_min_coord as f64 / unit) * s,
                            baseline: (origin + m.baseline as f64 / unit) * s,
                            bottom: (origin + m.content_block_max_coord as f64 / unit) * s,
                        });
                    }
                }
            }
            let children = node.layout_children.borrow();
            let kids: Vec<NodeId> = match children.as_ref() {
                Some(c) => c.iter().copied().collect(),
                None => node.children.iter().copied().collect(),
            };
            stack.extend(kids.into_iter().map(|c| (c, y)));
        }
        lines.sort_by(|a, b| a.baseline.total_cmp(&b.baseline));
        // Lines on the same row (side by side) count once, covering both glyph boxes.
        let mut merged: Vec<TextLine> = Vec::with_capacity(lines.len());
        for l in lines {
            match merged.last_mut() {
                Some(prev) if (l.baseline - prev.baseline).abs() < 2.0 => {
                    prev.top = prev.top.min(l.top);
                    prev.bottom = prev.bottom.max(l.bottom);
                }
                _ => merged.push(l),
            }
        }
        merged
    }

    pub fn max_scroll(&self) -> f64 {
        (self.content_h - self.height as f64).max(0.0)
    }

    /// Full document height in device pixels (after layout).
    pub fn content_height(&self) -> f64 {
        self.content_h
    }

    pub fn scroll_y(&self) -> f64 {
        self.scroll_y
    }

    /// Scroll by `dy` device pixels, clamped to the document.
    pub fn scroll_by(&mut self, dy: f64) {
        self.scroll_to(self.scroll_y + dy);
    }

    /// Restore an absolute reading position in device pixels.
    pub fn scroll_to(&mut self, y: f64) {
        if !y.is_finite() {
            return;
        }
        self.scroll_y = y.clamp(0.0, self.max_scroll());
        self.doc.borrow_mut().set_viewport_scroll(blitz_dom::Point { x: 0.0, y: self.scroll_y / self.scale as f64 });
    }

    /// Jump to a URL fragment without reparsing the document or running its scripts.
    /// Compare attributes directly: IDs may contain punctuation, Unicode or CSS syntax.
    /// Where each match of `query` sits on the page, in device pixels from the top of the page (not
    /// scrolled). A match is the deepest element whose text contains the query, ignoring case. At most 500.
    pub fn find_matches(&self, query: &str) -> Vec<f64> {
        let q = query.to_lowercase();
        if q.trim().is_empty() {
            return Vec::new();
        }
        let doc = self.doc.borrow();
        let mut found = Vec::new();
        let mut stack = vec![doc.root_node().id];
        // Depth-first in document order: a parent's text is checked before its children, and a match is the
        // deepest element that contains the query.
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            order.push(id);
            stack.extend(node.children.iter().rev().copied());
        }
        for id in order {
            if found.len() >= 500 {
                break;
            }
            let Some(node) = doc.get_node(id) else { continue };
            if node.element_data().is_none() {
                continue;
            }
            if !node.text_content().to_lowercase().contains(&q) {
                continue;
            }
            // Skip the element when one of its children already holds the match.
            let child_has = node.children.iter().any(|c| {
                doc.get_node(*c)
                    .map_or(false, |cn| cn.element_data().is_some() && cn.text_content().to_lowercase().contains(&q))
            });
            if child_has {
                continue;
            }
            let size = node.final_layout().size;
            if size.width <= 0.0 || size.height <= 0.0 {
                continue;
            }
            found.push(node.absolute_position(0.0, 0.0).y as f64 * self.scale as f64);
        }
        found
    }

    /// Scroll so a device-pixel position from `find_matches` is comfortably on screen.
    pub fn scroll_to_match(&mut self, y: f64) {
        self.scroll_to((y - 160.0).max(0.0));
    }

    pub fn scroll_to_fragment(&mut self, fragment: &str) -> bool {
        let name = percent_encoding::percent_decode_str(fragment).decode_utf8_lossy();
        let target = if name.is_empty() {
            Some(0.0)
        } else {
            let doc = self.doc.borrow();
            let find = |selector: &str, attr: blitz_dom::LocalName| {
                doc.query_selector_all(selector).ok()?.into_iter().find_map(|id| {
                    let node = doc.get_node(id)?;
                    (node.attr(attr.clone()) == Some(name.as_ref()))
                        .then(|| node.absolute_position(0.0, 0.0).y as f64 * self.scale as f64)
                })
            };
            find("[id]", blitz_dom::local_name!("id"))
                .or_else(|| find("a[name]", blitz_dom::local_name!("name")))
                .or_else(|| name.eq_ignore_ascii_case("top").then_some(0.0))
        };
        match target {
            Some(y) => { self.scroll_to(y); true }
            None => false,
        }
    }

    /// Paint the viewport at the current scroll position as 8-bit gray (row-major).
    pub fn render_gray(&mut self) -> Vec<u8> {
        self.render_gray_at(0)
    }

    /// Paint with the page moved down by `y_offset` device pixels. The browser
    /// uses this to keep the top of a page below the address bar.
    pub fn render_gray_at(&mut self, y_offset: u32) -> Vec<u8> {
        let t0 = std::time::Instant::now();
        self.settle(&mut || true);
        let t_settle = t0.elapsed();
        let mut painter = VelloCpuScenePainter::new(self.width as u16, self.height as u16);
        {
            let mut doc = self.doc.borrow_mut();
            blitz_paint::paint_scene(&mut painter, &mut doc, self.scale as f64, self.width, self.height, 0, y_offset);
        }
        let pixmap = painter.finish();
        let t_paint = t0.elapsed() - t_settle;
        let px = pixmap.data_as_u8_slice();
        let mut gray = Vec::with_capacity((self.width * self.height) as usize);
        for p in px.chunks_exact(4) {
            // Premultiplied RGBA composited over white, then luma in integer arithmetic (the
            // device's processor is slow at floating point).
            let inv = 255u16 - p[3] as u16;
            let r = (p[0] as u16 + inv).min(255) as u32;
            let g = (p[1] as u16 + inv).min(255) as u32;
            let b = (p[2] as u16 + inv).min(255) as u32;
            gray.push(((77 * r + 150 * g + 29 * b + 128) >> 8) as u8);
        }
        eprintln!(
            "[render] settle {:?}, paint {:?}, gray {:?}",
            t_settle,
            t_paint,
            t0.elapsed() - t_settle - t_paint
        );
        gray
    }

    /// Deliver a click at viewport point (x, y) to script listeners.
    /// Returns `Prevented` when script cancelled the default action.
    /// Send an event (`input`, `change`, `submit`, ...) to the page's listeners on `node`. Returns what the
    /// listeners did; `Prevented` means one called preventDefault.
    fn fire(&mut self, node: NodeId, kind: &str) -> Click {
        let Some(js) = self.js.as_ref() else { return Click::None };
        let outcome = js.event(node, kind);
        if outcome != Click::None {
            self.settle(&mut || true);
            self.content_h = self.measure();
        }
        outcome
    }

    pub fn click_at(&mut self, x: f32, y: f32) -> Click {
        let Some(js) = self.js.as_ref() else { return Click::None };
        let target = self.doc.borrow().element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale);
        let Some(target) = target else { return Click::None };
        let outcome = js.click(target);
        if outcome != Click::None {
            self.settle(&mut || true);
            self.content_h = self.measure();
        }
        outcome
    }

    /// Absolute URL of the link under viewport point (x, y), if any.
    /// Returns `kb:` actions and `kbp://` pages unchanged.
    pub fn link_at(&self, x: f32, y: f32) -> Option<String> {
        let doc = self.doc.borrow();
        let mut id: NodeId = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale)?;
        loop {
            let node = doc.get_node(id)?;
            if let Some(el) = node.element_data() {
                if &*el.name.local == "a" {
                    if let Some(href) = node.attr(blitz_dom::local_name!("href")) {
                        return self.resolve(href);
                    }
                }
            }
            id = node.parent?;
        }
    }

    /// Box `(x, y, w, h)` in viewport pixels of the link under (x, y), for the tap
    /// highlight. `None` when the point isn't on a link.
    pub fn link_box(&self, x: f32, y: f32) -> Option<(f32, f32, f32, f32)> {
        let doc = self.doc.borrow();
        let mut id: NodeId = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale)?;
        loop {
            let node = doc.get_node(id)?;
            if node.element_data().map_or(false, |e| &*e.name.local == "a") {
                let pos = node.absolute_position(0.0, 0.0);
                let size = node.final_layout().size;
                let k = self.scale;
                return Some((pos.x * k, pos.y * k - self.scroll_y as f32, size.width * k, size.height * k));
            }
            id = node.parent?;
        }
    }

    /// The text field under (x, y), if any. Disabled and read-only fields cannot be edited, so they are not
    /// returned.
    pub fn input_at(&self, x: f32, y: f32) -> Option<Field> {
        let doc = self.doc.borrow();
        let mut id: NodeId = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale)?;
        loop {
            let node = doc.get_node(id)?;
            if let Some(el) = node.element_data() {
                let tag: &str = &el.name.local;
                if tag == "textarea" || (tag == "input" && is_text_type(node.attr(blitz_dom::local_name!("type")))) {
                    if node.attr(blitz_dom::local_name!("disabled")).is_some()
                        || node.attr(blitz_dom::local_name!("readonly")).is_some()
                    {
                        return None;
                    }
                    let password = node.attr(blitz_dom::local_name!("type")).map_or(false, |t| t.eq_ignore_ascii_case("password"));
                    let value = if password {
                        self.secrets.get(&id.as_u64()).cloned().unwrap_or_default()
                    } else {
                        input_value(node)
                    };
                    return Some(Field { key: id.as_u64(), value, password });
                }
                if tag == "a" || tag == "button" {
                    return None;
                }
            }
            id = node.parent?;
        }
    }

    /// Store `value` in a text field. Committing a field does not submit its form.
    pub fn set_input_value(&mut self, key: u64, value: &str) {
        let id = NodeId::from_u64(key);
        self.store_input_value(id, value);
        // Scripts hear about the new value when it is committed (not on each key).
        self.fire(id, "input");
        self.fire(id, "change");
    }

    fn store_input_value(&mut self, id: NodeId, value: &str) {
        let key = id.as_u64();
        let password = {
            let doc = self.doc.borrow();
            doc.get_node(id)
                .and_then(|n| n.attr(blitz_dom::local_name!("type")))
                .map_or(false, |t| t.eq_ignore_ascii_case("password"))
        };
        let shown = if password {
            self.secrets.insert(key, value.to_string());
            "*".repeat(value.chars().count())
        } else {
            value.to_string()
        };
        let mut doc = self.doc.borrow_mut();
        let name = blitz_dom::QualName::new(None, blitz_dom::Namespace::from(""), blitz_dom::LocalName::from("value"));
        doc.mutate().set_attribute(id, name, &shown);
    }

    /// The `<select>` under (x, y), if it is enabled: its options, and which one is chosen (the first when none
    /// is marked, as browsers do).
    pub fn select_at(&self, x: f32, y: f32) -> Option<SelectField> {
        let doc = self.doc.borrow();
        let mut id: NodeId = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale)?;
        loop {
            let node = doc.get_node(id)?;
            if let Some(el) = node.element_data() {
                let tag: &str = &el.name.local;
                if tag == "select" {
                    if node.attr(blitz_dom::local_name!("disabled")).is_some() {
                        return None;
                    }
                    let options = options_of(&doc, id);
                    if options.is_empty() {
                        return None;
                    }
                    let any_selected = options.iter().any(|o| doc.get_node(*o).map_or(false, |n| n.attr(blitz_dom::local_name!("selected")).is_some()));
                    let title = node
                        .attr(blitz_dom::local_name!("name"))
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "Choose".to_string());
                    let options = options
                        .iter()
                        .enumerate()
                        .map(|(i, o)| {
                            let n = doc.get_node(*o);
                            let label = n.map(|n| n.text_content().trim().to_string()).unwrap_or_default();
                            let picked = match n {
                                Some(n) if any_selected => n.attr(blitz_dom::local_name!("selected")).is_some(),
                                _ => i == 0,
                            };
                            (label, picked)
                        })
                        .collect();
                    return Some(SelectField { key: id.as_u64(), title, options });
                }
                if tag == "a" || tag == "button" || tag == "input" || tag == "textarea" {
                    return None;
                }
            }
            id = node.parent?;
        }
    }

    /// Choose option `index` of the select `key`: it becomes the one marked selected, and the select shows its label.
    pub fn select_pick(&mut self, key: u64, index: usize) {
        let select = NodeId::from_u64(key);
        {
            let mut doc = self.doc.borrow_mut();
            let options = options_of(&doc, select);
            for (i, option) in options.iter().enumerate() {
                if i == index {
                    doc.mutate().set_attribute(*option, local("selected"), "true");
                } else {
                    doc.mutate().clear_attribute(*option, local("selected"));
                }
            }
        }
        self.show_select_label(select);
        self.fire(select, "input");
        self.fire(select, "change");
    }

    /// Give every select the label of its chosen option, so the page can draw it (see `prepare_html`).
    fn label_selects(&mut self) {
        let selects: Vec<NodeId> = {
            let doc = self.doc.borrow();
            doc.query_selector_all("select").map(|v| v.into_iter().collect()).unwrap_or_default()
        };
        for id in selects {
            self.show_select_label(id);
        }
    }

    /// The label of the chosen option (the first when none is marked) as the select's `data-kb-label`.
    fn show_select_label(&mut self, select: NodeId) {
        let mut doc = self.doc.borrow_mut();
        let options = options_of(&doc, select);
        let any_selected = options.iter().any(|o| doc.get_node(*o).map_or(false, |n| n.attr(blitz_dom::local_name!("selected")).is_some()));
        let label = options
            .iter()
            .enumerate()
            .find(|(i, o)| if any_selected { doc.get_node(**o).map_or(false, |n| n.attr(blitz_dom::local_name!("selected")).is_some()) } else { *i == 0 })
            .and_then(|(_, o)| doc.get_node(*o).map(|n| n.text_content().trim().to_string()))
            .unwrap_or_default();
        doc.mutate().set_attribute(select, local("data-kb-label"), &label);
    }

    /// Toggle a checkbox, or select a radio button (clearing the others of its group), at (x, y).
    /// Returns true when the page changed (then `input` and `change` are sent). Disabled controls do not change.
    pub fn toggle_choice_at(&mut self, x: f32, y: f32) -> bool {
        match self.toggle_choice_node(x, y) {
            Some(id) => {
                self.fire(id, "input");
                self.fire(id, "change");
                true
            }
            None => false,
        }
    }

    /// The checkbox or radio at (x, y), after its state changed. None when nothing changed.
    fn toggle_choice_node(&self, x: f32, y: f32) -> Option<NodeId> {
        let mut doc = self.doc.borrow_mut();
        let mut id: NodeId = match doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale) {
            Some(id) => id,
            None => return None,
        };
        loop {
            let Some(node) = doc.get_node(id) else { return None };
            if let Some(el) = node.element_data() {
                let tag: &str = &el.name.local;
                if tag == "input" {
                    let kind = node.attr(blitz_dom::local_name!("type")).unwrap_or("").to_ascii_lowercase();
                    if matches!(kind.as_str(), "checkbox" | "radio") {
                        if node.attr(blitz_dom::local_name!("disabled")).is_some() {
                            return None;
                        }
                        let checked = node.attr(blitz_dom::local_name!("checked")).is_some();
                        let name = node.attr(blitz_dom::local_name!("name")).map(str::to_string);
                        if kind == "radio" {
                            if checked {
                                return None; // a checked radio stays checked
                            }
                            if let Some(name) = name.as_deref() {
                                for other in radios_named(&doc, name) {
                                    doc.mutate().clear_attribute(other, local("checked"));
                                }
                            }
                            doc.mutate().set_attribute(id, local("checked"), "true");
                        } else if checked {
                            doc.mutate().clear_attribute(id, local("checked"));
                        } else {
                            doc.mutate().set_attribute(id, local("checked"), "true");
                        }
                        return Some(id);
                    }
                }
                if tag == "a" || tag == "button" || tag == "textarea" {
                    return None;
                }
            }
            match node.parent {
                Some(parent) => id = parent,
                None => return None,
            }
        }
    }

    /// Send the `submit` event for the form of the submit button under (x, y). Returns false when a script
    /// cancelled it, so the form must not be sent. True when there is no such form or nothing cancelled it.
    pub fn form_submit_allowed(&mut self, x: f32, y: f32) -> bool {
        let form = {
            let doc = self.doc.borrow();
            let Some(mut id) = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale) else { return true };
            let button = loop {
                let Some(node) = doc.get_node(id) else { return true };
                if let Some(el) = node.element_data() {
                    let tag: &str = &el.name.local;
                    let is_submit = tag == "button" && node.attr(blitz_dom::local_name!("type")).map_or(true, |t| t == "submit")
                        || (tag == "input" && node.attr(blitz_dom::local_name!("type")) == Some("submit"));
                    if is_submit {
                        break id;
                    }
                    if tag == "a" {
                        return true;
                    }
                }
                match node.parent {
                    Some(p) => id = p,
                    None => return true,
                }
            };
            let mut form = button;
            loop {
                let Some(n) = doc.get_node(form) else { return true };
                if n.element_data().map_or(false, |e| &*e.name.local == "form") {
                    break form;
                }
                match n.parent {
                    Some(p) => form = p,
                    None => return true,
                }
            }
        };
        self.fire(form, "submit") != Click::Prevented
    }

    /// If the tap landed on a submit button (or a form-submit input), the GET URL
    /// of its form, with the current field values.
    pub fn form_submit_at(&self, x: f32, y: f32) -> Option<Submit> {
        let id = {
            let doc = self.doc.borrow();
            let mut id: NodeId = doc.element_from_point(x / self.scale, (y + self.scroll_y as f32) / self.scale)?;
            loop {
                let node = doc.get_node(id)?;
                if let Some(el) = node.element_data() {
                    let tag: &str = &el.name.local;
                    let is_submit = tag == "button" && node.attr(blitz_dom::local_name!("type")).map_or(true, |t| t == "submit")
                        || (tag == "input" && node.attr(blitz_dom::local_name!("type")) == Some("submit"));
                    if is_submit {
                        break id;
                    }
                    if tag == "a" {
                        return None;
                    }
                }
                id = node.parent?;
            }
        };
        self.form_url_for(id, None)
    }

    /// Query URL of the form enclosing `node`. `override_value` replaces one field's value.
    fn form_url_for(&self, node: NodeId, override_value: Option<(NodeId, &str)>) -> Option<Submit> {
        let doc = self.doc.borrow();
        // Climb to the enclosing <form>.
        let mut form = node;
        loop {
            let n = doc.get_node(form)?;
            if n.element_data().map_or(false, |e| &*e.name.local == "form") {
                break;
            }
            form = n.parent?;
        }
        let form_node = doc.get_node(form)?;
        let action = form_node.attr(blitz_dom::local_name!("action")).unwrap_or("");
        let method = form_node.attr(blitz_dom::local_name!("method")).unwrap_or("get");
        let is_get = !method.eq_ignore_ascii_case("post");
        // Collect named fields in document order.
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut stack = vec![form];
        let mut ordered = Vec::new();
        while let Some(id) = stack.pop() {
            let Some(n) = doc.get_node(id) else { continue };
            ordered.push(id);
            stack.extend(n.children.iter().rev().copied());
        }
        for id in ordered {
            let Some(n) = doc.get_node(id) else { continue };
            let Some(el) = n.element_data() else { continue };
            let tag: &str = &el.name.local;
            if tag == "select" {
                let Some(name) = n.attr(blitz_dom::local_name!("name")) else { continue };
                if n.attr(blitz_dom::local_name!("disabled")).is_some() {
                    continue;
                }
                let options = options_of(&doc, id);
                let any_selected = options.iter().any(|o| doc.get_node(*o).map_or(false, |o| o.attr(blitz_dom::local_name!("selected")).is_some()));
                let chosen = options
                    .iter()
                    .enumerate()
                    .find(|(i, o)| if any_selected { doc.get_node(**o).map_or(false, |o| o.attr(blitz_dom::local_name!("selected")).is_some()) } else { *i == 0 })
                    .and_then(|(_, o)| doc.get_node(*o));
                if let Some(option) = chosen {
                    let value = option
                        .attr(blitz_dom::local_name!("value"))
                        .map(str::to_string)
                        .unwrap_or_else(|| option.text_content().trim().to_string());
                    pairs.push((name.to_string(), value));
                }
                continue;
            }
            if !matches!(tag, "input" | "textarea") {
                continue;
            }
            let Some(name) = n.attr(blitz_dom::local_name!("name")) else { continue };
            if n.attr(blitz_dom::local_name!("disabled")).is_some() {
                continue;
            }
            let kind = n.attr(blitz_dom::local_name!("type")).unwrap_or("text");
            if matches!(kind, "submit" | "button" | "image" | "reset" | "file") {
                continue;
            }
            if matches!(kind, "checkbox" | "radio") {
                if n.attr(blitz_dom::local_name!("checked")).is_none() {
                    continue;
                }
                let value = n.attr(blitz_dom::local_name!("value")).unwrap_or("on");
                pairs.push((name.to_string(), value.to_string()));
                continue;
            }
            let value = match override_value {
                Some((key, v)) if key == id => v.to_string(),
                _ => match self.secrets.get(&id.as_u64()) {
                    Some(secret) if kind.eq_ignore_ascii_case("password") => secret.clone(),
                    _ => input_value(n),
                },
            };
            pairs.push((name.to_string(), value));
        }
        let mut target = Url::parse(&self.base).ok()?.join(if action.is_empty() { &self.base } else { action }).ok()?;
        if !matches!(target.scheme(), "http" | "https") {
            return None;
        }
        if !is_get {
            return Some(Submit::Post { url: target.to_string(), fields: pairs });
        }
        {
            let mut q = target.query_pairs_mut();
            q.clear();
            for (k, v) in &pairs {
                q.append_pair(k, v);
            }
        }
        Some(Submit::Get(target.to_string()))
    }

    pub fn resolve(&self, href: &str) -> Option<String> {
        // Browser actions and internal pages pass through unchanged.
        if href.starts_with("kb:") || href.starts_with("kbp://") {
            return Some(href.to_string());
        }
        let base = Url::parse(&self.base).ok()?;
        let joined = base.join(href).ok()?;
        matches!(joined.scheme(), "http" | "https").then(|| joined.to_string())
    }

    /// Reader mode: rebuild the main article as clean HTML (headings and
    /// paragraphs only). Returns `None` when no article-like block is found.
    pub fn reader_html(&self) -> Option<String> {
        let doc = self.doc.borrow();
        let root = doc.root_element().id;
        let mut best: Option<(f64, NodeId)> = None;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else { continue };
            stack.extend(node.children.iter().copied());
            let Some(tag) = node.element_data().map(|e| e.name.local.to_string()) else { continue };
            if !matches!(tag.as_str(), "article" | "main" | "div" | "section" | "td") {
                continue;
            }
            let text_len = node.text_content().trim().len() as f64;
            let link_len = link_text_len(&doc, id) as f64;
            // Favour long text that isn't mostly navigation links.
            let score = text_len - 2.0 * link_len;
            if text_len > 300.0 && best.map_or(true, |(b, _)| score > b) {
                best = Some((score, id));
            }
        }
        let (_, container) = best?;
        let mut blocks = Vec::new();
        collect_blocks(&doc, container, &mut blocks);
        if blocks.is_empty() {
            return None;
        }
        let body: String = blocks
            .iter()
            .map(|(t, text)| format!("<{t}>{}</{t}>\n", esc_html(text)))
            .collect();
        Some(format!(
            r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title><style>
body{{margin:0;padding:36px 44px;font-family:serif;font-size:34px;line-height:1.5;color:#000;background:#fff}}
h1{{font-size:50px;line-height:1.2;margin:0 0 30px}} h2,h3,h4{{font-size:40px;margin:36px 0 14px}}
p{{margin:0 0 26px}} blockquote{{margin:0 0 26px 40px;font-style:italic}}
pre{{white-space:pre-wrap;font-size:28px}}
</style></head><body>{body}</body></html>"#,
            esc_html(&self.title)
        ))
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// Make `url` the page's location after an in-page navigation or a restore. Only a web
    /// address becomes the base for relative links and resources: an internal address (kbp://)
    /// keeps the base of the page it shows, so a saved page still loads its own resources.
    /// The address a script asked this page to load (`location.href = …`), once.
    pub fn take_navigation(&self) -> Option<String> {
        self.js.as_ref().and_then(|js| js.take_navigation())
    }

    /// The address a script set with history.pushState or replaceState, if any (taken once).
    pub fn take_url_changes(&self) -> Vec<(String, bool, String)> {
        self.js.as_ref().map(|js| js.take_url_changes()).unwrap_or_default()
    }

    /// Deliver popstate with `state_json` to the page's scripts (the page's own history entry is the one going back to).
    pub fn pop_state(&mut self, state_json: &str) {
        if let Some(js) = self.js.as_ref() {
            js.pop_state(state_json);
        }
        self.settle(&mut || true);
        self.content_h = self.measure();
    }

    pub fn set_location(&mut self, url: &str) {
        if url.starts_with("http://") || url.starts_with("https://") {
            self.base = url.to_string();
        }
        if let Some(js) = self.js.as_ref() {
            js.set_location(url);
        }
    }
}

/// Run one step of page construction. The layout engine panics on some stale or unusual document
/// states; such a step is skipped and the page is still shown.
fn contain<T: Default>(step: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(step)).unwrap_or_default()
}

/// Text-like `<input type>` values we edit on the on-screen keyboard.
fn is_text_type(kind: Option<&str>) -> bool {
    matches!(kind.unwrap_or("text").to_ascii_lowercase().as_str(), "" | "text" | "search" | "url" | "email" | "tel" | "number" | "password")
}

/// Current value of a field: the editor's text if it has one, else the value attribute.
fn input_value(node: &blitz_dom::Node) -> String {
    if let Some(data) = node.element_data().and_then(|e| e.text_input_data()) {
        return data.editor.text().to_string();
    }
    node.attr(blitz_dom::local_name!("value")).unwrap_or("").to_string()
}

/// Whether a `<script type=…>` is classic JavaScript we can run.
/// How many external scripts are fetched at the same time.
const SCRIPT_FETCH_WAVE: usize = 6;

/// Fetch the script addresses given (None: nothing to fetch), a wave at a time, in order.
fn fetch_scripts(urls: &[Option<String>]) -> Vec<Option<(String, Result<kb_net::Fetched, String>)>> {
    let mut out: Vec<Option<(String, Result<kb_net::Fetched, String>)>> = (0..urls.len()).map(|_| None).collect();
    let jobs: Vec<(usize, String)> = urls
        .iter()
        .enumerate()
        .filter_map(|(i, u)| u.clone().map(|u| (i, u)))
        .collect();
    for wave in jobs.chunks(SCRIPT_FETCH_WAVE) {
        let done: Vec<(usize, String, Result<kb_net::Fetched, String>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = wave
                .iter()
                .map(|(i, u)| {
                    let (i, u) = (*i, u.clone());
                    scope.spawn(move || {
                        let r = kb_net::load(&u, MAX_SCRIPT_BYTES, Policy::Cache(Duration::from_secs(600)));
                        (i, u, r)
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        for (i, u, r) in done {
            out[i] = Some((u, r));
        }
    }
    out
}

fn kind_is_script(kind: Option<&str>) -> bool {
    match kind.map(|k| k.trim().to_ascii_lowercase()) {
        None => true,
        // ES modules need an import loader, which this browser doesn't have: skip them.
        Some(k) => matches!(k.as_str(), "" | "text/javascript" | "application/javascript" | "text/ecmascript"),
    }
}

/// Total characters of text inside `<a>` elements under `id` (navigation density).
fn link_text_len(doc: &HtmlDocument, id: NodeId) -> usize {
    let mut total = 0;
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        let Some(node) = doc.get_node(n) else { continue };
        if node.element_data().map_or(false, |e| &*e.name.local == "a") {
            total += node.text_content().trim().len();
            continue;
        }
        stack.extend(node.children.iter().copied());
    }
    total
}

/// Headings, paragraphs, quotes and preformatted blocks under `id`, in order.
fn collect_blocks(doc: &HtmlDocument, id: NodeId, out: &mut Vec<(&'static str, String)>) {
    let Some(node) = doc.get_node(id) else { return };
    if let Some(el) = node.element_data() {
        let tag: &str = &el.name.local;
        let mapped = match tag {
            "h1" => Some("h1"),
            "h2" => Some("h2"),
            "h3" | "h4" | "h5" | "h6" => Some("h3"),
            "p" => Some("p"),
            "blockquote" => Some("blockquote"),
            "pre" => Some("pre"),
            _ => None,
        };
        if let Some(t) = mapped {
            let text = node.text_content();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                out.push((t, text));
            }
            return;
        }
        if matches!(tag, "script" | "style" | "nav" | "aside" | "footer" | "header" | "form") {
            return;
        }
    }
    for &child in &node.children {
        collect_blocks(doc, child, out);
    }
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Minimal `<title>` extraction (case-insensitive, no entity decoding).
pub fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = start + lower[start..].find('>')? + 1;
    let close = open_end + lower[open_end..].find("</title>")?;
    let title = html[open_end..close].trim();
    (!title.is_empty()).then(|| title.chars().take(120).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lines_are_where_the_text_is_painted() {
        // The repository's copy of the fonts; the device reads its own copy in /mnt/us/browser/fonts. Fonts load once per thread.
        let font = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/NotoSans-Regular.ttf");
        std::env::set_var("KB_FONTS", font);
        let para = "Nói xong, hắn lấy ra một chiếc hộp ngọc. Sau khi mở nắp, một luồng hàn khí tỏa ra, bên trong lộ ra năm hạt sen đỏ như máu. ";
        let html = format!(
            "<!doctype html><html><head><style>body{{margin:12px;font:20px sans-serif;line-height:1.8}}p{{margin:0 0 1lh}}</style></head><body><h2>Chương 1</h2>{}</body></html>",
            (0..12).map(|i| format!("<p>{}</p>", para.repeat(1 + i % 3))).collect::<String>()
        );
        let mut page = Page::new_scaled(&html, "https://example.org/", 600, 800,
            Arc::new(SyncNet::new(1024)), &mut || true, Some(300.0));
        // Lines are read from the layout, which the first paint settles (the browser always paints first).
        page.render_gray();
        let lines = page.text_lines();
        assert!(lines.len() > 30, "{} lines", lines.len());
        // With paragraphs one line apart, the body text sits on one grid: every gap is a whole number of lines.
        let body: Vec<f64> = lines.iter().skip(1).map(|l| l.baseline).collect();
        let pitch = body[1] - body[0];
        for pair in body.windows(2) {
            let n = (pair[1] - pair[0]) / pitch;
            assert!((n - n.round()).abs() < 0.02, "gap {} is not whole lines of {pitch}", pair[1] - pair[0]);
        }
        // The painted text: rows with ink, as bands; each band's dense part ends at its baseline.
        page.scroll_to(700.0);
        let gray = page.render_gray();
        let (w, h) = (600usize, 800usize);
        let ink: Vec<usize> = (0..h).map(|y| gray[y * w..(y + 1) * w].iter().filter(|&&p| p < 128).count()).collect();
        let mut painted = Vec::new();
        let mut y = 0;
        while y < h {
            if ink[y] == 0 {
                y += 1;
                continue;
            }
            let start = y;
            while y < h && ink[y] > 0 {
                y += 1;
            }
            let band = &ink[start..y];
            let peak = *band.iter().max().unwrap();
            let last = band.iter().rposition(|&c| c * 2 > peak).unwrap();
            // Whole bands only (not cut by the screen edges).
            if start > 0 && y < h {
                painted.push((start + last) as f64);
            }
        }
        assert!(painted.len() > 5);
        for b in painted {
            let near = lines.iter().map(|l| (l.baseline - 700.0 - b).abs()).fold(f64::MAX, f64::min);
            assert!(near <= 2.0, "painted baseline {b} is {near} rows from the nearest layout line");
        }
    }

    #[test]
    fn internal_location_keeps_the_web_base_for_relative_links() {
        let html = r#"<!doctype html><p><a id="a" href="next.html">next</a></p>"#;
        let mut page = Page::new_scaled(html, "https://example.org/docs/", 600, 800,
            Arc::new(SyncNet::new(1024)), &mut || true, Some(300.0));
        page.set_location("kbp://saved/abc123");
        assert_eq!(page.resolve("next.html").as_deref(), Some("https://example.org/docs/next.html"));
        page.set_location("https://example.org/other/page.html");
        assert_eq!(page.resolve("next.html").as_deref(), Some("https://example.org/other/next.html"));
    }

    #[test]
    fn fragment_targets_support_encoded_ids_named_anchors_and_absolute_scroll() {
        let html = r#"<!doctype html><html><head><style>body{margin:0}div{height:1000px}h2{margin:0;height:100px}</style></head><body>
            <div></div><h2 id="chương+2:reading">Section</h2><a name="legacy"></a><div></div><div></div></body></html>"#;
        let mut page = Page::new_scaled(html, "https://example.org/article", 600, 800,
            Arc::new(SyncNet::new(1024)), &mut || true, Some(300.0));
        assert!(page.scroll_to_fragment("ch%C6%B0%C6%A1ng+2:reading"));
        assert_eq!(page.scroll_y(), 2000.0);
        page.scroll_to(500.0);
        page.scroll_to(500.0);
        assert_eq!(page.scroll_y(), 500.0);
        assert!(page.scroll_to_fragment("legacy"));
        assert!(page.scroll_y() >= 2200.0);
        let saved = page.scroll_y();
        assert!(!page.scroll_to_fragment("missing"));
        assert_eq!(page.scroll_y(), saved);
        assert!(page.scroll_to_fragment("TOP"));
        assert_eq!(page.scroll_y(), 0.0);
        page.scroll_to(400.0);
        assert!(page.scroll_to_fragment(""));
        assert_eq!(page.scroll_y(), 0.0);
        assert_eq!(page.resolve("#part").as_deref(), Some("https://example.org/article#part"));
    }

    #[test]
    fn title_is_found_case_insensitively_and_trimmed() {
        assert_eq!(extract_title("<HTML><TITLE>  Hello  </TITLE></HTML>").as_deref(), Some("Hello"));
        assert_eq!(extract_title("<title></title>"), None);
        assert_eq!(extract_title("<p>no title</p>"), None);
    }

    #[test]
    fn only_classic_scripts_run() {
        assert!(kind_is_script(None));
        assert!(kind_is_script(Some("text/javascript")));
        assert!(kind_is_script(Some(" Application/JavaScript ")));
        assert!(!kind_is_script(Some("module")));
        assert!(!kind_is_script(Some("application/ld+json")));
    }

    #[test]
    fn text_fields_are_recognised_by_type() {
        assert!(is_text_type(None));
        assert!(is_text_type(Some("search")));
        assert!(!is_text_type(Some("checkbox")));
        assert!(!is_text_type(Some("submit")));
    }
}
