//! Local data: bookmarks, history, settings and saved pages, kept as tab-separated files in the
//! data folder (`KB_DATA`, or `/mnt/us/browser/data` on the Kindle).

use std::{fs, path::PathBuf};

use kb_net::store;

/// Bookmarks a new install starts with: none. The user adds their own.
pub(crate) const DEFAULT_BOOKMARKS: &[(&str, &str)] = &[];

pub(crate) const HISTORY_CAP: usize = 300;

pub(crate) fn data_file(name: &str) -> PathBuf {
    store::data_dir().join(name)
}

pub(crate) fn ensure_defaults() {
    // One-time migration: add the default bookmarks once, so devices that already
    // have a bookmark list get them too, and removing one later doesn't bring it back.
    const MARKER: &str = "defaults_v2";
    let marker = data_file(MARKER);
    if marker.exists() {
        return;
    }
    let path = data_file("bookmarks.tsv");
    let mut list = read_pairs(&path);
    for (u, t) in DEFAULT_BOOKMARKS {
        if !list.iter().any(|(existing, _)| existing == u) {
            list.push((u.to_string(), t.to_string()));
        }
    }
    write_pairs(&path, &list);
    if let Some(dir) = marker.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&marker, "1\n");
}

/// Saved-page metadata is `url title`. `write_pairs` turns tabs into spaces, so the separator is a
/// space: a URL never contains one. Returns (url, title); the title may be empty.
pub(crate) fn split_saved_meta(meta: &str) -> (&str, &str) {
    meta.split_once(' ').unwrap_or((meta, ""))
}

pub(crate) fn read_pairs(path: &PathBuf) -> Vec<(String, String)> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            l.split_once('\t')
                .map(|(a, b)| (a.to_string(), b.to_string()))
        })
        .collect()
}

pub(crate) fn write_pairs(path: &PathBuf, pairs: &[(String, String)]) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = pairs
        .iter()
        .map(|(a, b)| {
            format!(
                "{}\t{}\n",
                a.replace(['\t', '\n'], " "),
                b.replace(['\t', '\n'], " ")
            )
        })
        .collect();
    let _ = fs::write(path, text);
}

pub(crate) fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}
