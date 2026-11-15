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
pub type HttpGet = Rc<dyn Fn(&str) -> Option<Vec<String>>>;

impl Js {
    /// Create a script engine for `doc`, with `url` as the page location.
    pub fn new(doc: Doc, url: &str, http: HttpGet) -> Result<Js, String> {
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
            ctx.globals().set("__kbCssWidth", 412)?;
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
            let first = stack.lines().next().unwrap_or("").to_string();
            format!("{message} {first}")
        }
        None => exc.as_string().and_then(|s| s.to_string().ok()).unwrap_or_default(),
    };
    eprintln!("[js] {name}: {msg}: {}", detail.trim());
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
        let doc = d.borrow();
        let found = match node_of(scope) {
            Some(s) => doc.query_selector_in(s, &sel).ok().flatten(),
            None => doc.query_selector(&sel).ok().flatten(),
        };
        found.map(|id| id.as_u64() as f64)
    })?)?;

    let d = doc.clone();
    n.set("qsa", Function::new(ctx.clone(), move |sel: String, scope: f64| -> Vec<f64> {
        let doc = d.borrow();
        let found = match node_of(scope) {
            Some(s) => doc.query_selector_all_in(s, &sel).ok(),
            None => doc.query_selector_all(&sel).ok(),
        };
        found.map(|v| v.iter().map(|id| id.as_u64() as f64).collect()).unwrap_or_default()
    })?)?;

    let d = doc.clone();
    n.set("byId", Function::new(ctx.clone(), move |id: String| -> Option<f64> {
        d.borrow().get_element_by_id(&id).map(|n| n.as_u64() as f64)
    })?)?;

    // ---- reads ----
    let d = doc.clone();
    n.set("tag", Function::new(ctx.clone(), move |id: f64| -> Option<String> {
        let doc = d.borrow();
        let node = doc.get_node(node_of(id)?)?;
        Some(match node.element_data() {
            Some(el) => el.name.local.to_string().to_ascii_uppercase(),
            None => "#TEXT".to_string(),
        })
    })?)?;

    let d = doc.clone();
    n.set("getText", Function::new(ctx.clone(), move |id: f64| -> Option<String> {
        let doc = d.borrow();
        Some(doc.get_node(node_of(id)?)?.text_content())
    })?)?;

    let d = doc.clone();
    n.set("getAttr", Function::new(ctx.clone(), move |id: f64, name: String| -> Option<String> {
        let doc = d.borrow();
        let node = doc.get_node(node_of(id)?)?;
        node.attr(LocalName::from(name.as_str())).map(str::to_string)
    })?)?;

    let d = doc.clone();
    n.set("parent", Function::new(ctx.clone(), move |id: f64| -> Option<f64> {
        let doc = d.borrow();
        doc.get_node(node_of(id)?)?.parent.map(|p| p.as_u64() as f64)
    })?)?;

    let d = doc.clone();
    n.set("children", Function::new(ctx.clone(), move |id: f64| -> Vec<f64> {
        let doc = d.borrow();
        let Some(node) = node_of(id).and_then(|i| doc.get_node(i)) else { return Vec::new() };
        node.children
            .iter()
            .filter(|c| doc.get_node(**c).map_or(false, |n| n.is_element()))
            .map(|c| c.as_u64() as f64)
            .collect()
    })?)?;

    let d = doc.clone();
    n.set("matches", Function::new(ctx.clone(), move |id: f64, sel: String| -> bool {
        let doc = d.borrow();
        match node_of(id) {
            Some(node) => doc.matches_selector(node, &sel).unwrap_or(false),
            None => false,
        }
    })?)?;

    // Next (dir = 1) or previous (dir = -1) element sibling.
    let d = doc.clone();
    n.set("sibling", Function::new(ctx.clone(), move |id: f64, dir: f64| -> Option<f64> {
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
    })?)?;

    // ---- writes ----
    let d = doc.clone();
    n.set("create", Function::new(ctx.clone(), move |tag: String| -> f64 {
        let mut doc = d.borrow_mut();
        let id = doc.mutate().create_element(html_name(&tag.to_ascii_lowercase()), Vec::new());
        id.as_u64() as f64
    })?)?;

    let d = doc.clone();
    n.set("text", Function::new(ctx.clone(), move |text: String| -> f64 {
        let mut doc = d.borrow_mut();
        let id = doc.mutate().create_text_node(&text);
        id.as_u64() as f64
    })?)?;

    let d = doc.clone();
    n.set("setText", Function::new(ctx.clone(), move |id: f64, text: String| {
        let Some(id) = node_of(id) else { return };
        let mut doc = d.borrow_mut();
        let mut m = doc.mutate();
        m.remove_and_drop_all_children(id);
        let t = m.create_text_node(&text);
        m.append_children(id, &[t]);
    })?)?;

    let d = doc.clone();
    n.set("setHtml", Function::new(ctx.clone(), move |id: f64, html: String| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_inner_html(id, &html);
    })?)?;

    let d = doc.clone();
    n.set("setAttr", Function::new(ctx.clone(), move |id: f64, name: String, value: String| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_attribute(id, attr_name(&name), &value);
    })?)?;

    let d = doc.clone();
    n.set("delAttr", Function::new(ctx.clone(), move |id: f64, name: String| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().clear_attribute(id, attr_name(&name));
    })?)?;

    let d = doc.clone();
    n.set("setStyle", Function::new(ctx.clone(), move |id: f64, prop: String, value: String| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().set_style_property(id, &prop, &value);
    })?)?;

    let d = doc.clone();
    n.set("append", Function::new(ctx.clone(), move |parent: f64, child: f64| {
        let (Some(p), Some(c)) = (node_of(parent), node_of(child)) else { return };
        d.borrow_mut().mutate().append_children(p, &[c]);
    })?)?;

    let d = doc.clone();
    n.set("insertBefore", Function::new(ctx.clone(), move |parent: f64, child: f64, before: f64| {
        let (Some(p), Some(c)) = (node_of(parent), node_of(child)) else { return };
        let mut doc = d.borrow_mut();
        let mut m = doc.mutate();
        match node_of(before) {
            Some(b) => m.insert_nodes_before(b, &[c]),
            None => m.append_children(p, &[c]),
        }
    })?)?;

    let d = doc.clone();
    n.set("remove", Function::new(ctx.clone(), move |id: f64| {
        let Some(id) = node_of(id) else { return };
        d.borrow_mut().mutate().remove_node(id);
    })?)?;

    // ---- host services ----
    n.set("log", Function::new(ctx.clone(), |msg: String| {
        eprintln!("[js] {msg}");
    })?)?;

    n.set("httpGet", Function::new(ctx.clone(), move |url: String| -> Option<Vec<String>> {
        http(&url)
    })?)?;

    ctx.globals().set("__kbn", n)?;
    Ok(())
}
