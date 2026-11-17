//! `localStorage` on the device: one JSON file per site, in the data folder, so the values survive a restart.

use std::{fs, path::PathBuf};

/// Most data one site may store. Flash is small and shared, so a page cannot fill it.
const MAX_BYTES: usize = 1024 * 1024;

fn file_for(origin: &str) -> PathBuf {
    let safe: String = origin.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    kb_net::store::data_dir().join("localstorage").join(format!("{safe}.json"))
}

/// The stored values of `origin` as a JSON object. An empty object when there are none or the file is unreadable.
pub fn load(origin: &str) -> String {
    fs::read_to_string(file_for(origin)).unwrap_or_else(|_| "{}".to_string())
}

/// Store `json` as the values of `origin`. Returns false when it is too large or cannot be written.
pub fn save(origin: &str, json: &str) -> bool {
    if json.len() > MAX_BYTES {
        return false;
    }
    let path = file_for(origin);
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    // Write beside the file and rename it into place, so an interrupted write keeps the old values.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).is_ok() && fs::rename(&tmp, &path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_survive_being_read_back_from_the_folder() {
        let dir = std::env::temp_dir().join(format!("kb-ls-test-{}", std::process::id()));
        // SAFETY: tests in this module run one at a time with their own folder name.
        unsafe { std::env::set_var("KB_DATA", &dir) };
        assert_eq!(load("https://example.org"), "{}");
        assert!(save("https://example.org", r#"{"theme":"night"}"#));
        assert_eq!(load("https://example.org"), r#"{"theme":"night"}"#);
        assert!(!save("https://example.org", &"x".repeat(MAX_BYTES + 1)));
        assert_eq!(load("https://example.org"), r#"{"theme":"night"}"#);
        let _ = fs::remove_dir_all(&dir);
    }
}
