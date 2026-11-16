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
    let _ = write_atomic(&marker, b"1\n");
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

/// Write `bytes` to `path` without ever leaving a half-written file: the data goes to a temporary file
/// beside it, is flushed to flash, and is then renamed over the old file. An interrupted write keeps the old
/// contents. Failures are logged (the device log shows them) and returned.
pub(crate) fn write_atomic(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(match path.extension() {
        Some(ext) => format!("{}.tmp", ext.to_string_lossy()),
        None => "tmp".to_string(),
    });
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if let Err(err) = &result {
        eprintln!("[storage] could not write {}: {err}", path.display());
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub(crate) fn write_pairs(path: &PathBuf, pairs: &[(String, String)]) {
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
    let _ = write_atomic(path, text.as_bytes());
}

pub(crate) fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The sites whose scripts are off (one host per line in `scripts_off.txt`).
fn scripts_off_list() -> Vec<String> {
    fs::read_to_string(data_file("scripts_off.txt"))
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whether scripts are off for `host` (turned off for that site from the Display sheet).
pub(crate) fn scripts_off(host: &str) -> bool {
    scripts_off_list().iter().any(|h| h == host)
}

/// Turn scripts on or off for `host`.
pub(crate) fn set_scripts_off(host: &str, off: bool) {
    let mut hosts = scripts_off_list();
    hosts.retain(|h| h != host);
    if off {
        hosts.push(host.to_string());
    }
    let path = data_file("scripts_off.txt");
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let text: String = hosts.iter().map(|h| format!("{h}\n")).collect();
    let _ = write_atomic(&path, text.as_bytes());
}

#[cfg(test)]
mod atomic_tests {
    use super::*;

    #[test]
    fn writes_replace_the_file_and_leave_no_temporary_file() {
        let dir = std::env::temp_dir().join(format!("kb-atomic-{}", std::process::id()));
        let path = dir.join("session.tsv");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert!(!dir.join("session.tsv.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_that_cannot_happen_reports_an_error_and_keeps_nothing_half_written() {
        let dir = std::env::temp_dir().join(format!("kb-atomic-fail-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // The parent of the target is a file, so no file can be created there.
        let blocker = dir.join("blocker");
        fs::write(&blocker, "x").unwrap();
        let path = blocker.join("data.tsv");
        assert!(write_atomic(&path, b"data").is_err());
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
