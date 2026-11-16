//! JavaScript for web pages: QuickJS (via rquickjs) with a small DOM API over
//! blitz-dom. Scripts run once at load, timers run after them within a budget,
//! and clicks dispatch to listeners registered from script.
//!
//! Limits: 24 MB heap, 3 s per script/timer slice (interrupt handler), and
//! `fetch` is synchronous under a Promise. Not implemented: `innerHTML` reads,
//! `document.write`, modules and imports, WebSockets, XHR.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use blitz_dom::{LocalName, Namespace, QualName};
use blitz_html::HtmlDocument;
use blitz_traits::node_id::NodeId;
use rquickjs::{context::EvalOptions, Context, Ctx, Function, Object, Runtime};

const PRELUDE: &str = include_str!("prelude.js");
const MEMORY_LIMIT: usize = 48 * 1024 * 1024;
/// Time budget for one script or one timer pass.
pub const SLICE: Duration = Duration::from_secs(3);

/// Shared document handle. The page and the script engine both hold one.
pub type Doc = Rc<RefCell<HtmlDocument>>;

/// Result of dispatching a click into script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// No listener on the path: the caller handles it (links, etc.).
    None,
    /// Listeners ran; the default action should still happen.
    Handled,
    /// A listener called preventDefault; skip the default action.
    Prevented,
}

pub struct Js {
    rt: Runtime,
    ctx: Context,
    deadline: Rc<Cell<Instant>>,
}

/// Page fetch hook: `(url) -> Option<[status, final_url, body]>`.
/// Sends a request for a page script: (method, url, headers as name/value pairs, body) -> [status, final url, body].
pub type HttpGet = Rc<dyn Fn(&str, &str, &[String], Option<String>) -> Option<Vec<String>>>;

impl Js {
    /// Create a script engine for `doc`, with `url` as the page location.
    pub fn new(doc: Doc, url: &str, http: HttpGet, width: f32, height: f32, scale: f32) -> Result<Js, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        rt.set_memory_limit(MEMORY_LIMIT);
        let deadline = Rc::new(Cell::new(Instant::now() + SLICE));
        let d = deadline.clone();
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > d.get())));
        let ctx = Context::full(&rt).map_err(|e| e.to_string())?;
        let url = url.to_string();
        ctx.with(|ctx| -> rquickjs::Result<()> {
            install_natives(&ctx, doc, http)?;
            ctx.globals().set("__kbUrl", url)?;
            ctx.globals().set("__kbCssWidth", width)?;
            ctx.globals().set("__kbCssHeight", height)?;
            ctx.globals().set("__kbDevicePixelRatio", scale)?;
            ctx.eval_with_options::<(), _>(PRELUDE, sloppy("prelude"))?;
            Ok(())
        })
        .map_err(|e| format!("js init failed: {e}"))?;
        Ok(Js { rt, ctx, deadline })
    }

    /// Run one script. Errors are reported to stderr and do not stop the page.
    pub fn run(&self, code: &str, name: &str) {
        self.deadline.set(Instant::now() + SLICE);
        self.ctx.with(|ctx| {
            if let Err(e) = ctx.eval_with_options::<(), _>(code, sloppy(name)) {
                report(&ctx, name, &e.to_string());
            }
        });
        self.drain_jobs();
    }

    /// Run queued timers within one time slice.
    pub fn run_timers(&self) {
        self.deadline.set(Instant::now() + SLICE);
        self.ctx.with(|ctx| {
            // Pages wait for DOMContentLoaded/load before finishing their setup.
            if let Err(e) = ctx.eval::<i32, _>("(typeof __fireLoadEvents === 'function' && __fireLoadEvents(), __kb.runTimers())") {
                report(&ctx, "timers", &e.to_string());
            }
        });
        self.drain_jobs();
    }

    /// Make `node` the script that is running (`document.currentScript`), or none when `None`.
    pub fn set_current_script(&self, node: Option<u64>) {
        self.ctx.with(|ctx| {
            let result = (|| -> rquickjs::Result<()> {
                let kb: Object = ctx.globals().get("__kb")?;
                let set: Function = kb.get("setCurrentScript")?;
                set.call((node.map_or(-1.0, |n| n as f64),))
            })();
            if let Err(e) = result {
                report(&ctx, "currentScript", &e.to_string());
            }
        });
    }

    /// Keep location reads consistent after the browser follows a fragment link.
    pub fn set_location(&self, url: &str) {
        self.deadline.set(Instant::now() + SLICE);
        self.ctx.with(|ctx| {
            let result = (|| -> rquickjs::Result<()> {
                let kb: Object = ctx.globals().get("__kb")?;
                let update: Function = kb.get("setLocation")?;
                update.call((url,))
            })();
            if let Err(e) = result {
                report(&ctx, "location", &e.to_string());
            }
        });
    }

    /// Dispatch a click at `node` through its ancestors.
    pub fn click(&self, node: NodeId) -> Click {
        self.deadline.set(Instant::now() + SLICE);
        let id = node.as_u64() as f64;
        let code = self.ctx.with(|ctx| -> rquickjs::Result<i32> {
            let kb: Object = ctx.globals().get("__kb")?;
            let f: Function = kb.get("click")?;
            f.call((id,))
        });
        self.drain_jobs();
        match code {
            Ok(1) => Click::Handled,
            Ok(2) => Click::Prevented,
            _ => Click::None,
        }
    }

    /// Run pending promise jobs (microtasks) to completion.
    fn drain_jobs(&self) {
        while let Ok(true) = self.rt.execute_pending_job() {}
    }
}

/// Web pages are sloppy-mode scripts (implicit globals, `this` in functions).
fn sloppy(name: &str) -> EvalOptions {
    let mut opts = EvalOptions::default();
    opts.strict = false;
    opts.filename = Some(name.to_string());
    opts
}

fn report(ctx: &Ctx<'_>, name: &str, msg: &str) {
    // The pending exception is an Error object: pull out message and first stack line.
    let exc = ctx.catch();
    let detail = match exc.as_object() {
        Some(o) => {
            let message: String = o.get("message").unwrap_or_default();
            let stack: String = o.get("stack").unwrap_or_default();
            // The first frames show where the error happened (script URL and line).
            let frames: Vec<&str> = stack.lines().take(3).map(str::trim).collect();
            format!("{message} [{}]", frames.join(" | "))
        }
        None => exc.as_string().and_then(|s| s.to_string().ok()).unwrap_or_default(),
    };
    eprintln!("[js] {name}: {msg}: {}", detail.trim());
}

/// A copy of `src` (its element name and attributes, and its children when `deep`), detached from
/// the page. Text nodes are copied as text.
fn clone_subtree(doc: &mut HtmlDocument, src: NodeId, deep: bool) -> Option<NodeId> {
    let (text, element, children) = {
        let node = doc.get_node(src)?;
        let text = node.text_data().map(|t| t.content.to_string());
        let element = node
            .element_data()
            .map(|e| (e.name.clone(), e.attrs.iter().cloned().collect::<Vec<_>>()));
        let children: Vec<NodeId> = if deep { node.children.iter().copied().collect() } else { Vec::new() };
        (text, element, children)
    };
    if let Some(content) = text {
        return Some(doc.mutate().create_text_node(&content));
    }
    let (name, attrs) = element?;
    let copy = doc.mutate().create_element(name, attrs);
    for child in children {
        if let Some(c) = clone_subtree(doc, child, true) {
            doc.mutate().append_children(copy, &[c]);
        }
    }
    Some(copy)
}

/// Run a DOM operation. The layout engine panics on a node id that is no longer in the document;
/// such an operation does nothing, and the page keeps running.
fn guarded<T: Default>(op: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(op)).unwrap_or_default()
}

fn node_of(v: f64) -> Option<NodeId> {
    (v >= 0.0 && v.is_finite()).then(|| NodeId::from_u64(v as u64))
}

fn html_name(local: &str) -> QualName {
    QualName::new(None, Namespace::from("http://www.w3.org/1999/xhtml"), LocalName::from(local))
}

fn attr_name(local: &str) -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(local))
}

fn install_natives(ctx: &Ctx<'_>, doc: Doc, http: HttpGet) -> rquickjs::Result<()> {
    let n = Object::new(ctx.clone())?;

    // ---- queries ----
    let d = doc.clone();
    n.set("qs", Function::new(ctx.clone(), move |sel: String, scope: f64| -> Option<f64> {
        guarded(|| {
        let doc = d.borrow();
        let found = match node_of(scope) {
            Some(s) => doc.query_selector_in(s, &sel).ok().flatten(),
            None => doc.query_selector(&sel).ok().flatten(),
        };
        found.map(|id| id.as_u64() as f64)
        })
    })?)?;

    let d = doc.clone();
    n.set("qsa", Function::new(ctx.clone(), move |sel: String, scope: f64| -> Vec<f64> {
        guarded(|| {
        let doc = d.borrow();
        let found = match node_of(scope) {
            Some(s) => doc.query_selector_all_in(s, &sel).ok(),
            None => doc.query_selector_all(&sel).ok(),
        };
        found.map(|v| v.iter().map(|id| id.as_u64() as f64).collect()).unwrap_or_default()
        })
    })?)?;

    let d = doc.clone();
    n.set("byId", Function::new(ctx.clone(), move |id: String| -> Option<f64> {
        guarded(|| {
        d.borrow().get_element_by_id(&id).map(|n| n.as_u64() as f64)
        })
    })?)?;

    // ---- reads ----
    let d = doc.clone();
    n.set("tag", Function::new(ctx.clone(), move |id: f64| -> Option<String> {
        guarded(|| {
        let doc = d.borrow();
        let node = doc.get_node(node_of(id)?)?;
        Some(match node.element_data() {
            Some(el) => el.name.local.to_string().to_ascii_uppercase(),
            None => "#TEXT".to_string(),
        })
        })
    })?)?;

    let d = doc.clone();
    n.set("getText", Function::new(ctx.clone(), move |id: f64| -> Option<String> {
        guarded(|| {
        let doc = d.borrow();
        Some(doc.get_node(node_of(id)?)?.text_content())
        })
    })?)?;

    let d = doc.clone();
    n.set("getAttr", Function::new(ctx.clone(), move |id: f64, name: String| -> Option<String> {
        guarded(|| {
        let doc = d.borrow();
        let node = doc.get_node(node_of(id)?)?;
        node.attr(LocalName::from(name.as_str())).map(str::to_string)
        })
    })?)?;

    let d = doc.clone();
    n.set("parent", Function::new(ctx.clone(), move |id: f64| -> Option<f64> {
        guarded(|| {
        let doc = d.borrow();
        doc.get_node(node_of(id)?)?.parent.map(|p| p.as_u64() as f64)
        })
    })?)?;

    let d = doc.clone();
    n.set("children", Function::new(ctx.clone(), move |id: f64| -> Vec<f64> {
        guarded(|| {
        let doc = d.borrow();
        let Some(node) = node_of(id).and_then(|i| doc.get_node(i)) else { return Vec::new() };
        node.children
            .iter()
            .filter(|c| doc.get_node(**c).map_or(false, |n| n.is_element()))
            .map(|c| c.as_u64() as f64)
            .collect()
        })
    })?)?;

    let d = doc.clone();
    n.set("matches", Function::new(ctx.clone(), move |id: f64, sel: String| -> bool {
        guarded(|| {
        let doc = d.borrow();
        match node_of(id) {
            Some(node) => doc.matches_selector(node, &sel).unwrap_or(false),
            None => false,
        }
        })
    })?)?;

    // Next (dir = 1) or previous (dir = -1) element sibling.
    let d = doc.clone();
    n.set("sibling", Function::new(ctx.clone(), move |id: f64, dir: f64| -> Option<f64> {
        guarded(|| {
        let doc = d.borrow();
        let node = doc.get_node(node_of(id)?)?;
        let parent = doc.get_node(node.parent?)?;
        let idx = parent.children.iter().position(|c| *c == node.id)?;
        let mut i = idx as isize;
        loop {
            i += if dir < 0.0 { -1 } else { 1 };
            if i < 0 || i as usize >= parent.children.len() {
                return None;
            }
            let c = parent.children[i as usize];
            if doc.get_node(c).map_or(false, |n| n.is_element()) {
                return Some(c.as_u64() as f64);
            }
        }
        })
    })?)?;

    // ---- writes ----
    let d = doc.clone();
    n.set("create", Function::new(ctx.clone(), move |tag: String| -> f64 {
        guarded(|| {
        let mut doc = d.borrow_mut();
        let id = doc.mutate().create_element(html_name(&tag.to_ascii_lowercase()), Vec::new());
        id.as_u64() as f64
        })
    })?)?;

    let d = doc.clone();
    n.set("clone", Function::new(ctx.clone(), move |id: f64, deep: bool| -> f64 {
        guarded(|| {
        let Some(src) = node_of(id) else { return -1.0 };
        let mut doc = d.borrow_mut();
        clone_subtree(&mut doc, src, deep).map_or(-1.0, |c| c.as_u64() as f64)
        })
    })?)?;

    let d = doc.clone();
    n.set("text", Function::new(ctx.clone(), move |text: String| -> f64 {
        guarded(|| {
        let mut doc = d.borrow_mut();
        let id = doc.mutate().create_text_node(&text);
        id.as_u64() as f64
        })
    })?)?;

    let d = doc.clone();
    n.set("setText", Function::new(ctx.clone(), move |id: f64, text: String| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        let mut doc = d.borrow_mut();
        let is_text = doc.get_node(id).map_or(false, |n| n.is_text_node());
        let mut m = doc.mutate();
        if is_text {
            // A text node holds its own text: replace the text, not its (non-existent) children.
            m.set_node_text(id, &text);
            return;
        }
        m.remove_and_drop_all_children(id);
        let t = m.create_text_node(&text);
        m.append_children(id, &[t]);
        })
    })?)?;

    // Child nodes as scripts see them: elements and text, in document order.
    let d = doc.clone();
    n.set("childNodes", Function::new(ctx.clone(), move |id: f64| -> Vec<f64> {
        guarded(|| {
            let doc = d.borrow();
            let Some(node) = node_of(id).and_then(|i| doc.get_node(i)) else { return Vec::new() };
            node.children
                .iter()
                .filter(|c| doc.get_node(**c).map_or(false, |n| n.is_element() || n.is_text_node()))
                .map(|c| c.as_u64() as f64)
                .collect()
        })
    })?)?;

    // DOM node type: 1 element, 3 text.
    let d = doc.clone();
    n.set("nodeType", Function::new(ctx.clone(), move |id: f64| -> f64 {
        guarded(|| {
            let doc = d.borrow();
            match node_of(id).and_then(|i| doc.get_node(i)) {
                Some(n) if n.is_text_node() => 3.0,
                Some(n) if n.is_element() => 1.0,
                _ => 8.0,
            }
        })
    })?)?;

    let d = doc.clone();
    n.set("setHtml", Function::new(ctx.clone(), move |id: f64, html: String| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_inner_html(id, &html);
        })
    })?)?;

    let d = doc.clone();
    n.set("setAttr", Function::new(ctx.clone(), move |id: f64, name: String, value: String| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_attribute(id, attr_name(&name), &value);
        })
    })?)?;

    let d = doc.clone();
    n.set("delAttr", Function::new(ctx.clone(), move |id: f64, name: String| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().clear_attribute(id, attr_name(&name));
        })
    })?)?;

    let d = doc.clone();
    n.set("setStyle", Function::new(ctx.clone(), move |id: f64, prop: String, value: String| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_style_property(id, &prop, &value);
        })
    })?)?;

    let d = doc.clone();
    n.set("append", Function::new(ctx.clone(), move |parent: f64, child: f64| {
        guarded(|| {
        let (Some(p), Some(c)) = (node_of(parent), node_of(child)) else { return };
        // A node the page dropped (its old children after innerHTML) is gone: nothing to append.
        let mut doc = d.borrow_mut();
        if doc.get_node(p).is_none() || doc.get_node(c).is_none() {
            return;
        }
        doc.mutate().append_children(p, &[c]);
        })
    })?)?;

    let d = doc.clone();
    n.set("insertBefore", Function::new(ctx.clone(), move |parent: f64, child: f64, before: f64| {
        guarded(|| {
        let (Some(p), Some(c)) = (node_of(parent), node_of(child)) else { return };
        let mut doc = d.borrow_mut();
        if doc.get_node(p).is_none() || doc.get_node(c).is_none() {
            return;
        }
        let mut m = doc.mutate();
        match node_of(before) {
            Some(b) => m.insert_nodes_before(b, &[c]),
            None => m.append_children(p, &[c]),
        }
        })
    })?)?;

    let d = doc.clone();
    n.set("remove", Function::new(ctx.clone(), move |id: f64| {
        guarded(|| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().remove_node(id);
        })
    })?)?;

    // ---- host services ----
    n.set("log", Function::new(ctx.clone(), |msg: String| {
        eprintln!("[js] {msg}");
    })?)?;

    let h = http.clone();
    n.set("httpGet", Function::new(ctx.clone(), move |url: String| -> Option<Vec<String>> {
        guarded(|| {
        h("GET", &url, &[], None)
        })
    })?)?;

    // fetch(): any method, headers as a flat name, value, name, value list, and an optional body.
    let h = http.clone();
    n.set("httpRequest", Function::new(ctx.clone(), move |method: String, url: String, headers: Vec<String>, body: Option<String>| -> Option<Vec<String>> {
        guarded(|| h(&method, &url, &headers, body))
    })?)?;

    ctx.globals().set("__kbn", n)?;
    Ok(())
}
