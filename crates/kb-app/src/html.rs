//! HTML helpers: escaping for markup the browser builds, and the page-level CSS added to every
//! document from the settings.

use crate::settings::Settings;

pub(crate) fn prepare_html(html: &str, settings: &Settings) -> String {
    let mut css = String::from("<style>img{background:#ececec}");
    if settings.text_pct != 100 {
        css.push_str(&format!("html{{zoom:{}%}}", settings.text_pct));
    }
    css.push_str("</style>");
    let lower = html.to_ascii_lowercase();
    if let Some(i) = lower.find("<head") {
        if let Some(j) = html[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], css, &html[at..]);
        }
    }
    // No head element: put the style right after the doctype (keeps standards mode).
    if let Some(i) = lower.find("<!doctype") {
        if let Some(j) = html[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], css, &html[at..]);
        }
    }
    format!("{css}{html}")
}

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_css_goes_into_the_head_so_standards_mode_is_kept() {
        let settings = Settings {
            text_pct: 125,
            ..Settings::default()
        };
        let out = prepare_html(
            "<!doctype html><html><head><title>t</title></head><body></body></html>",
            &settings,
        );
        assert!(out.starts_with("<!doctype html><html><head><style>"));
        assert!(out.contains("zoom:125%"));
        let plain = prepare_html("<!doctype html><p>x</p>", &Settings::default());
        assert!(plain.starts_with("<!doctype html><style>"));
    }
}
