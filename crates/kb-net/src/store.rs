//! Persistent state on the device's flash: cookies and a size-capped HTTP cache.
//! Everything lives under `$KB_DATA` (default `/mnt/us/browser/data`).

use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::Fetched;

/// Cache budget on flash. The Oasis has a few GB free; keep the browser small.
const CACHE_BUDGET: u64 = 200 * 1024 * 1024;

pub fn data_dir() -> PathBuf {
    std::env::var("KB_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/mnt/us/browser/data"))
}

pub fn cookies_path() -> PathBuf {
    data_dir().join("cookies.json")
}

fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// FNV-1a 64-bit, for cache file names (not security sensitive).
fn key(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A cached response, if one exists and is younger than `max_age`.
pub fn cache_get(url: &str, max_age: Duration) -> Option<Fetched> {
    let raw = fs::read(cache_dir().join(key(url))).ok()?;
    let newline = raw.iter().position(|&b| b == b'\n')?;
    let header = std::str::from_utf8(&raw[..newline]).ok()?;
    let mut parts = header.splitn(3, '\t');
    let stored: u64 = parts.next()?.parse().ok()?;
    let content_type = parts.next()?.to_string();
    let final_url = parts.next()?.to_string();
    if now_secs().saturating_sub(stored) > max_age.as_secs() {
        return None;
    }
    Some(Fetched { url: final_url, content_type, body: raw[newline + 1..].to_vec() })
}

/// Store a response, then evict oldest entries if the cache is over budget.
pub fn cache_put(url: &str, f: &Fetched) {
    let dir = cache_dir();
    let _ = fs::create_dir_all(&dir);
    let mut out = format!("{}\t{}\t{}\n", now_secs(), f.content_type, f.url).into_bytes();
    out.extend_from_slice(&f.body);
    let _ = fs::write(dir.join(key(url)), out);
    evict(&dir);
}

fn evict(dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= CACHE_BUDGET {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files {
        if total <= CACHE_BUDGET {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// Remove every cached response (user action).
pub fn cache_clear() {
    let _ = fs::remove_dir_all(cache_dir());
}

/// Bytes currently used by the cache.
pub fn cache_size() -> u64 {
    fs::read_dir(cache_dir())
        .map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}
