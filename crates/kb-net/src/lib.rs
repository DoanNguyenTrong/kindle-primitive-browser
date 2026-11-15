//! Network layer: blocking HTTP(S) fetches with rustls (bundled Mozilla roots),
//! a size cap, and a `NetProvider` that lets blitz-dom load sub-resources
//! (stylesheets, images, fonts) synchronously.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use ureq::ResponseExt;

pub mod store;

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};

const USER_AGENT: &str = "Mozilla/5.0 (Linux; Kindle; U) kb-browser/0.1";

#[derive(Debug, Clone)]
pub struct Fetched {
    /// Final URL after redirects.
    pub url: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(30)))
            .max_redirects(8)
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .build()
            .into();
        if let Ok(text) = std::fs::read_to_string(store::cookies_path()) {
            let mut jar = agent.cookie_jar_lock();
            let _ = jar.load_json(text.as_bytes());
        }
        agent
    })
}

/// Write the cookie jar to flash. Call after navigation and on exit.
pub fn save_cookies() {
    let mut buf = Vec::new();
    if agent().cookie_jar_lock().save_json(&mut buf).is_ok() {
        let path = store::cookies_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, buf);
    }
}

/// How a load may use the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Always go to the network (cache is refreshed with the result).
    Network,
    /// Use a cached copy if it is younger than this; otherwise fetch.
    Cache(Duration),
}

/// Hosts that serve ads and trackers. Sub-resources from these are not fetched.
const BLOCKED_HOSTS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googletagmanager.com",
    "google-analytics.com",
    "googleadservices.com",
    "adservice.google.com",
    "facebook.net",
    "connect.facebook.net",
    "scorecardresearch.com",
    "taboola.com",
    "outbrain.com",
    "criteo.com",
    "amazon-adsystem.com",
    "hotjar.com",
    "newrelic.com",
];

/// True if `url` points at a blocked ad or tracking host (or a subdomain of one).
pub fn is_blocked(url: &str) -> bool {
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    BLOCKED_HOSTS
        .iter()
        .any(|b| host == *b || host.ends_with(&format!(".{b}")))
}

/// Load a document with the cache policy. Network failures fall back to any
/// cached copy, so pages already read stay readable offline.
pub fn load(url: &str, max_bytes: u64, policy: Policy) -> Result<Fetched, String> {
    if let Policy::Cache(max_age) = policy {
        if let Some(hit) = store::cache_get(url, max_age) {
            return Ok(hit);
        }
    }
    match fetch(url, max_bytes) {
        Ok(f) => {
            store::cache_put(url, &f);
            Ok(f)
        }
        Err(e) => store::cache_get(url, Duration::from_secs(60 * 60 * 24 * 365))
            .ok_or(e),
    }
}

/// Fetch an http(s) URL, refusing bodies larger than `max_bytes`.
pub fn fetch(url: &str, max_bytes: u64) -> Result<Fetched, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported scheme: {url}"));
    }
    let resp = agent()
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .header("Accept-Language", "en,vi;q=0.8")
        .call()
        .map_err(|e| format!("request failed: {e}"))?;
    read_response(resp, max_bytes)
}

/// Submit a form with POST (`application/x-www-form-urlencoded`).
pub fn post_form(url: &str, fields: &[(String, String)], max_bytes: u64) -> Result<Fetched, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported scheme: {url}"));
    }
    let resp = agent()
        .post(url)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .send_form(fields.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .map_err(|e| format!("request failed: {e}"))?;
    read_response(resp, max_bytes)
}

fn read_response(mut resp: ureq::http::Response<ureq::Body>, max_bytes: u64) -> Result<Fetched, String> {
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let final_url = resp.get_uri().to_string();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp
        .body_mut()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
        .map_err(|e| format!("read failed: {e}"))?;
    Ok(Fetched { url: final_url, content_type, body })
}

/// Counters for progress display. Shared by the loader threads and the UI.
#[derive(Default)]
pub struct Stats {
    /// Sub-resources requested for the current page (not deferred images).
    pub requested: AtomicUsize,
    pub done: AtomicUsize,
    /// Images held back until the text is on screen.
    pub deferred: AtomicUsize,
    pub deferred_done: AtomicUsize,
}

/// Sub-resource loader for blitz-dom. Requests run on worker threads (at most
/// `MAX_IN_FLIGHT` at once). Each gets exactly one `handler.bytes` call, empty
/// on failure, because blitz-dom waits for that callback before it paints.
///
/// With `defer_images` on, image requests are queued instead of fetched, so the
/// text and CSS can be laid out and painted first. `release_deferred` fetches them.
pub struct SyncNet {
    pub max_bytes: u64,
    stats: Arc<Stats>,
    defer_images: AtomicBool,
    /// Off: image requests are answered empty, with no network and no decoding.
    images_allowed: AtomicBool,
    queue: Mutex<Vec<(String, Box<dyn NetHandler>)>>,
    /// Image URLs found in the page markup (so images without a file extension are deferred too).
    image_urls: Mutex<std::collections::HashSet<String>>,
}

const MAX_IN_FLIGHT: usize = 6;
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

impl SyncNet {
    pub fn new(max_bytes: u64) -> SyncNet {
        SyncNet {
            max_bytes,
            stats: Arc::new(Stats::default()),
            defer_images: AtomicBool::new(false),
            images_allowed: AtomicBool::new(true),
            queue: Mutex::new(Vec::new()),
            image_urls: Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Remember which URLs the page uses as `<img>` sources.
    pub fn set_image_urls(&self, urls: impl IntoIterator<Item = String>) {
        let mut set = self.image_urls.lock().unwrap_or_else(|e| e.into_inner());
        set.extend(urls);
    }

    pub fn stats(&self) -> Arc<Stats> {
        self.stats.clone()
    }

    /// Turn image loading on or off for the pages built from now on.
    pub fn set_images_allowed(&self, on: bool) {
        self.images_allowed.store(on, Ordering::Release);
    }

    pub fn set_defer_images(&self, on: bool) {
        self.defer_images.store(on, Ordering::Release);
    }

    /// Start fetching every deferred image, in the order given (visible images first).
    /// URLs not in `order` follow in their original order.
    pub fn release_ordered(&self, order: &[String]) {
        let mut items: Vec<_> = std::mem::take(&mut *self.queue.lock().unwrap_or_else(|e| e.into_inner()));
        let rank: std::collections::HashMap<&str, usize> =
            order.iter().enumerate().map(|(i, u)| (u.as_str(), i)).collect();
        items.sort_by_key(|(url, _)| rank.get(url.as_str()).copied().unwrap_or(usize::MAX));
        for (url, handler) in items {
            self.spawn(url, handler, true);
        }
    }

    /// Start fetching every deferred image. Safe to call more than once.
    pub fn release_deferred(&self) {
        self.release_ordered(&[]);
    }

    fn spawn(&self, url: String, handler: Box<dyn NetHandler>, deferred: bool) {
        let max = self.max_bytes;
        let stats = self.stats.clone();
        if deferred {
            stats.deferred.fetch_add(0, Ordering::Relaxed);
        } else {
            stats.requested.fetch_add(1, Ordering::Relaxed);
        }
        thread::spawn(move || {
            while IN_FLIGHT.load(Ordering::Acquire) >= MAX_IN_FLIGHT {
                thread::sleep(Duration::from_millis(5));
            }
            IN_FLIGHT.fetch_add(1, Ordering::AcqRel);
            let result = load(&url, max, Policy::Cache(Duration::from_secs(600)));
            IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            match result {
                Ok(f) => handler.bytes(f.url, Bytes::from(f.body)),
                Err(e) => {
                    eprintln!("[net] {url}: {e}");
                    handler.bytes(url, Bytes::new());
                }
            }
            if deferred {
                stats.deferred_done.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.done.fetch_add(1, Ordering::Relaxed);
            }
        });
    }
}

/// True for URLs that point at a raster or vector image (by extension).
pub fn is_image_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".avif", ".bmp", ".svg"]
        .iter()
        .any(|ext| path.ends_with(ext) || path.contains(&format!("{ext}/")))
}

impl NetProvider for SyncNet {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();
        // Inline-only schemes (data:, blob:) and blocked trackers are answered empty.
        if !(url.starts_with("http://") || url.starts_with("https://")) || is_blocked(&url) {
            handler.bytes(url, Bytes::new());
            return;
        }
        let known_image = self
            .image_urls
            .lock()
            .map(|set| set.contains(&url))
            .unwrap_or(false);
        if !self.images_allowed.load(Ordering::Acquire) && (known_image || is_image_url(&url)) {
            eprintln!("[net] image skipped (images off): {url}");
            handler.bytes(url, Bytes::new());
            return;
        }
        if self.defer_images.load(Ordering::Acquire) && (known_image || is_image_url(&url)) {
            self.stats.deferred.fetch_add(1, Ordering::Relaxed);
            self.queue.lock().unwrap_or_else(|e| e.into_inner()).push((url, handler));
            return;
        }
        self.spawn(url, handler, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_ad_and_tracker_hosts_and_their_subdomains() {
        assert!(is_blocked("https://doubleclick.net/ad.js"));
        assert!(is_blocked("https://stats.g.doubleclick.net/x"));
        assert!(is_blocked("https://www.google-analytics.com:443/collect"));
        assert!(!is_blocked("https://news.ycombinator.com/"));
        assert!(!is_blocked("https://notdoubleclick.net.example.org/"));
    }

    #[test]
    fn recognises_image_urls_by_extension_and_path_segment() {
        assert!(is_image_url("https://example.org/a/cover.JPG?w=200"));
        assert!(is_image_url("https://cdn.example.org/img.webp/200"));
        assert!(!is_image_url("https://example.org/app.js"));
        assert!(!is_image_url("https://example.org/page.html#img.png-section"));
    }

    #[test]
    fn fetch_rejects_non_http_schemes_without_touching_the_network() {
        assert!(fetch("file:///etc/passwd", 1024).is_err());
        assert!(post_form("data:text/html,hi", &[], 1024).is_err());
    }
}
