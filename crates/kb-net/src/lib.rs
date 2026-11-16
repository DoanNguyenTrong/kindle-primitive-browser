//! Network layer: blocking HTTP(S) fetches with rustls (bundled Mozilla roots),
//! a size cap, and a `NetProvider` that lets blitz-dom load sub-resources
//! (stylesheets, images, fonts) synchronously.

use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use ureq::{
    unversioned::{
        resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver},
        transport::{DefaultConnector, NextTimeout},
    },
    ResponseExt,
};

pub mod store;

use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};

const USER_AGENT: &str = "Mozilla/5.0 (Linux; Kindle; U) PrimitiveBrowser/0.1";

#[derive(Debug, Clone)]
pub struct Fetched {
    /// Final URL after redirects.
    pub url: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

/// Whether this system can open IPv6 sockets. (The Kindle cannot.)
fn ipv6_available() -> bool {
    std::net::UdpSocket::bind("[::]:0").is_ok()
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        // IPv6 only where the system can open IPv6 sockets. The Kindle cannot ("Address family not supported
        // by protocol", os error 97), and the HTTP client does not fall back to IPv4 by itself, so there
        // every connection to a site that publishes IPv6 addresses failed. There the agent is IPv4 only.
        let family = if ipv6_available() {
            ureq::config::IpFamily::Any
        } else {
            ureq::config::IpFamily::Ipv4Only
        };
        let config = ureq::Agent::config_builder()
            .ip_family(family)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(30)))
            .max_redirects(8)
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::with_parts(config, DefaultConnector::default(), CachedResolver);
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

/// The host name and port of an http(s) address.
fn host_and_port(url: &str) -> Option<(String, u16)> {
    let (default_port, rest) = match url.split_once("://") {
        Some(("https", rest)) => (443, rest),
        Some(("http", rest)) => (80, rest),
        _ => return None,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p.parse::<u16>().unwrap_or(default_port)),
        _ => (authority, default_port),
    };
    (!host.is_empty()).then(|| (host.to_string(), port))
}

/// How long a host's addresses are reused. A page and each of its images are fetched from the same hosts,
/// and the Kindle's lookups are slow, so one lookup serves them all. The entry is short-lived so that a
/// change of network or DNS server is picked up within a minute.
const DNS_TTL: Duration = Duration::from_secs(60);

/// Most addresses kept per host (the same limit the HTTP client applies).
const MAX_ADDRS: usize = 16;

type DnsCache = Mutex<HashMap<(String, u16), (Instant, Vec<SocketAddr>)>>;

fn dns_cache() -> &'static DnsCache {
    static CACHE: OnceLock<DnsCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The addresses remembered for `host:port`, if they are still fresh.
fn cached_addrs(host: &str, port: u16) -> Option<Vec<SocketAddr>> {
    let cache = dns_cache().lock().ok()?;
    let (at, addrs) = cache.get(&(host.to_string(), port))?;
    (at.elapsed() < DNS_TTL).then(|| addrs.clone())
}

/// Forget what is remembered for `host:port`, so the next use looks it up again.
fn forget_addrs(host: &str, port: u16) {
    if let Ok(mut cache) = dns_cache().lock() {
        cache.remove(&(host.to_string(), port));
    }
}

/// Forget the addresses of the host in `url`. Called when a request to it fails.
fn forget_host(url: &str) {
    if let Some((host, port)) = host_and_port(url) {
        forget_addrs(&host, port);
    }
}

/// The addresses of `host:port`: the remembered ones while fresh, otherwise a new lookup, which is then
/// remembered.
fn resolve_host(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    if let Some(addrs) = cached_addrs(host, port) {
        return Ok(addrs);
    }
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    if !addrs.is_empty() {
        if let Ok(mut cache) = dns_cache().lock() {
            cache.insert((host.to_string(), port), (Instant::now(), addrs.clone()));
        }
    }
    Ok(addrs)
}

/// The name lookup used by the HTTP client. A host looked up recently (by the loopback check in
/// [`redirected_to_device`]) is answered from the cache, filtered to the address families this device
/// can use. Any other host takes the client's own lookup, with its timeout; that result is not kept here,
/// so the cache holds only the complete list from the check.
#[derive(Debug)]
struct CachedResolver;

impl Resolver for CachedResolver {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        if let Some((host, port)) = uri_host_and_port(uri) {
            if let Some(addrs) = cached_addrs(&host, port) {
                let mut result = self.empty();
                for addr in config.ip_family().keep_wanted(addrs.into_iter()).take(MAX_ADDRS) {
                    result.push(addr);
                }
                if !result.is_empty() {
                    return Ok(result);
                }
            }
        }
        DefaultResolver::default().resolve(uri, config, timeout)
    }
}

/// The host name and port of a request URI, with the scheme's default port when none is given.
fn uri_host_and_port(uri: &ureq::http::Uri) -> Option<(String, u16)> {
    let default_port = match uri.scheme_str()? {
        "https" => 443,
        "http" => 80,
        _ => return None,
    };
    Some((uri.host()?.to_string(), uri.port_u16().unwrap_or(default_port)))
}

/// A site whose name resolves only to this device's own loopback address (127.0.0.0/8 or ::1) is being
/// sent to the device by a hosts-file or DNS override: nothing can be reached that way. Returns the message
/// to show. Literal addresses and `localhost` are not affected. The lookup it makes is remembered for the
/// request that follows.
fn redirected_to_device(url: &str) -> Option<String> {
    let (host, port) = host_and_port(url)?;
    if host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let addrs = resolve_host(&host, port).ok()?;
    if addrs.is_empty() || addrs.iter().any(|a| !a.ip().is_loopback()) {
        return None;
    }
    Some(format!(
        "{host} resolves to this device itself ({}): a hosts-file or DNS override on the device redirects it",
        addrs[0].ip()
    ))
}

/// When a page cannot be fetched, log where its host resolves and whether each address accepts a
/// connection. The device log then shows a network-side refusal apart from a DNS problem.
fn diagnose_connect(url: &str) {
    use std::net::TcpStream;
    let Some((host, port)) = host_and_port(url) else { return };
    match (host.as_str(), port).to_socket_addrs() {
        Ok(addrs) => {
            let addrs: Vec<std::net::SocketAddr> = addrs.collect();
            eprintln!("[net-diag] {host} resolves to {addrs:?}");
            for addr in addrs.iter().take(4) {
                match TcpStream::connect_timeout(addr, Duration::from_secs(5)) {
                    Ok(_) => eprintln!("[net-diag] connect {addr}: ok"),
                    Err(e) => eprintln!("[net-diag] connect {addr}: {e}"),
                }
            }
        }
        Err(e) => eprintln!("[net-diag] {host} does not resolve: {e}"),
    }
}

/// Fetch an http(s) URL, refusing bodies larger than `max_bytes`.
pub fn fetch(url: &str, max_bytes: u64) -> Result<Fetched, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported scheme: {url}"));
    }
    if let Some(message) = redirected_to_device(url) {
        return Err(message);
    }
    let resp = agent()
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .header("Accept-Language", "en,vi;q=0.8")
        .call()
        .map_err(|e| {
            forget_host(url);
            diagnose_connect(url);
            format!("request failed: {e}")
        })?;
    read_response(resp, max_bytes)
}

/// Submit a form with POST (`application/x-www-form-urlencoded`).
/// The answer to a request made by a page script: status, final address, and body text.
pub struct Reply {
    pub status: u16,
    pub url: String,
    pub content_type: String,
    pub body: String,
}

/// Send a request for a page script (`fetch`): any method, extra headers, an optional text body.
/// Every status is an answer; the page decides what it means.
pub fn request(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&str>,
    max_bytes: u64,
) -> Result<Reply, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported scheme: {url}"));
    }
    if let Some(message) = redirected_to_device(url) {
        return Err(message);
    }
    let agent = agent();
    let text = body.unwrap_or("");
    let sent = match method.to_ascii_uppercase().as_str() {
        "POST" | "PUT" | "PATCH" => {
            let mut req = match method.to_ascii_uppercase().as_str() {
                "POST" => agent.post(url),
                "PUT" => agent.put(url),
                _ => agent.patch(url),
            };
            for (k, v) in headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.send(text)
        }
        other => {
            let mut req = match other {
                "HEAD" => agent.head(url),
                "DELETE" => agent.delete(url),
                _ => agent.get(url),
            };
            for (k, v) in headers {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        }
    };
    let resp = sent.map_err(|e| {
        forget_host(url);
        format!("request failed: {e}")
    })?;
    let status = resp.status().as_u16();
    let url = resp.get_uri().to_string();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp
        .into_body()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
        .map_err(|e| format!("read failed: {e}"))?;
    Ok(Reply { status, url, content_type, body: String::from_utf8_lossy(&bytes).into_owned() })
}

pub fn post_form(url: &str, fields: &[(String, String)], max_bytes: u64) -> Result<Fetched, String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!("unsupported scheme: {url}"));
    }
    let resp = agent()
        .post(url)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .send_form(fields.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .map_err(|e| {
            forget_host(url);
            format!("request failed: {e}")
        })?;
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

/// The content of a `data:` URL (base64 or percent-encoded), or None when it is malformed.
pub fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
    if meta.ends_with(";base64") {
        decode_base64(data)
    } else {
        Some(percent_decode(data))
    }
}

/// Standard or URL-safe base64; whitespace is skipped and padding ends the data.
fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// `%XX` escapes to bytes; everything else as is.
fn percent_decode(s: &str) -> Vec<u8> {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(hi), Some(lo)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
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
        // A data: URL carries its content inline. Images still follow the images setting.
        if url.starts_with("data:") {
            if url.starts_with("data:image/") && !self.images_allowed.load(Ordering::Acquire) {
                handler.bytes(url, Bytes::new());
                return;
            }
            let bytes = decode_data_url(&url).unwrap_or_default();
            handler.bytes(url, Bytes::from(bytes));
            return;
        }
        // Other inline-only schemes (blob:) and blocked trackers are answered empty.
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
    fn decodes_base64_and_percent_encoded_data_urls() {
        let png = decode_data_url("data:image/png;base64,iVBORw0KGgo=").unwrap();
        assert_eq!(&png[..4], b"\x89PNG");
        assert_eq!(decode_data_url("data:text/plain,hi%20there").unwrap(), b"hi there");
        assert_eq!(decode_data_url("data:text/plain;base64,aGk=").unwrap(), b"hi");
        assert!(decode_data_url("data:text/plain;base64,@@").is_none());
        assert!(decode_data_url("not-a-data-url").is_none());
    }

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
    fn remembered_addresses_answer_the_client_filtered_to_its_families() {
        let host = "dns-cache-test.example";
        let v4 = SocketAddr::from(([127, 0, 0, 1], 8080));
        let v6 = SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, 8080));
        assert!(cached_addrs(host, 8080).is_none());
        dns_cache().lock().unwrap().insert((host.to_string(), 8080), (Instant::now(), vec![v4, v6]));

        let uri: ureq::http::Uri = "http://dns-cache-test.example:8080/x".parse().unwrap();
        let v4_only = ureq::config::Config::builder()
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            .build();
        let answer = CachedResolver.resolve(&uri, &v4_only, NextTimeout::default()).unwrap();
        assert_eq!(&answer[..], &[v4][..]);

        let any = ureq::config::Config::builder().ip_family(ureq::config::IpFamily::Any).build();
        let answer = CachedResolver.resolve(&uri, &any, NextTimeout::default()).unwrap();
        assert_eq!(&answer[..], &[v4, v6][..]);

        forget_host("http://dns-cache-test.example:8080/x");
        assert!(cached_addrs(host, 8080).is_none());
    }

    #[test]
    fn agent_connects_to_remembered_addresses_without_a_lookup() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            }
        });
        // `.invalid` never resolves, so the request succeeds only if the remembered address is used.
        let host = "cache-agent-test.invalid";
        dns_cache().lock().unwrap().insert(
            (host.to_string(), port),
            (Instant::now(), vec![SocketAddr::from(([127, 0, 0, 1], port))]),
        );
        let result = agent().get(&format!("http://{host}:{port}/")).call();
        forget_addrs(host, port);
        assert_eq!(result.expect("the remembered address should be used").status().as_u16(), 200);
    }

    #[test]
    fn expired_addresses_are_not_reused() {
        let host = "dns-expiry-test.example";
        let stale = Instant::now() - DNS_TTL - Duration::from_secs(1);
        dns_cache().lock().unwrap().insert(
            (host.to_string(), 80),
            (stale, vec![SocketAddr::from(([127, 0, 0, 1], 80))]),
        );
        assert!(cached_addrs(host, 80).is_none());
        forget_addrs(host, 80);
    }

    #[test]
    fn fetch_rejects_non_http_schemes_without_touching_the_network() {
        assert!(fetch("file:///etc/passwd", 1024).is_err());
        assert!(post_form("data:text/html,hi", &[], 1024).is_err());
    }
}
