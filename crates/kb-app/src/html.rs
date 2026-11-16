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

/// The attribute `name` of an HTML tag, unquoted (`content="…"`, `content='…'` or `content=…`).
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let at = lower.find(&format!("{name}="))?;
    let rest = tag[at + name.len() + 1..].trim_start();
    match rest.chars().next()? {
        q @ ('"' | '\'') => {
            let inner = &rest[1..];
            Some(inner[..inner.find(q)?].to_string())
        }
        _ => Some(rest.split_whitespace().next()?.trim_end_matches('>').to_string()),
    }
}

/// The address of a `<meta http-equiv="refresh">` in the start of `html`, when the wait is short enough
/// to count as a redirect (up to three seconds; a longer wait is a waiting page). A relative address
/// resolves against `base`.
pub(crate) fn meta_refresh_target(html: &str, base: &str) -> Option<String> {
    let head: String = html.chars().take(64 * 1024).collect();
    let lower = head.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<meta") {
        let start = from + i;
        let end = start + lower[start..].find('>')?;
        let tag_lower = &lower[start..end];
        if tag_lower.contains("http-equiv") && tag_lower.contains("refresh") {
            let content = attr_value(&head[start..end], "content")?;
            let (secs, rest) = content.split_once([';', ','])?;
            if secs.trim().parse::<f32>().ok()? > 3.0 {
                return None;
            }
            let target = rest.trim();
            let target = if target.to_ascii_lowercase().starts_with("url=") { &target[4..] } else { target };
            let target = target.trim().trim_matches(|c| c == '\'' || c == '"');
            if target.is_empty() {
                return None;
            }
            return url::Url::parse(base).ok()?.join(target).ok().map(|u| u.to_string());
        }
        from = end;
    }
    None
}

#[cfg(test)]
mod meta_refresh_tests {
    use super::meta_refresh_target;

    #[test]
    fn a_short_meta_refresh_is_a_redirect_to_its_address() {
        let html = r#"<html><head><meta http-equiv="refresh" content="0;URL=https://www.cnn.com/"></head>"#;
        assert_eq!(meta_refresh_target(html, "https://duckduckgo.com/l/"), Some("https://www.cnn.com/".into()));
        let relative = "<meta http-equiv='refresh' content=\"0; url='plain.html'\">";
        assert_eq!(
            meta_refresh_target(relative, "http://127.0.0.1:8780/redirect.html"),
            Some("http://127.0.0.1:8780/plain.html".into())
        );
    }

    #[test]
    fn a_long_wait_or_no_refresh_is_not_a_redirect() {
        assert_eq!(meta_refresh_target(r#"<meta http-equiv="refresh" content="30;URL=/next">"#, "https://a.test/"), None);
        assert_eq!(meta_refresh_target("<html><head></head></html>", "https://a.test/"), None);
    }
}

/// Whether a media query (the `media` attribute of a stylesheet link) applies to this device: a light,
/// screen display. Print-only sheets and the dark colour scheme do not apply. Other queries are kept,
/// as before.
fn media_matches(media: &str) -> bool {
    let m = media.trim().to_ascii_lowercase();
    m.is_empty() || m.split(',').any(|q| query_matches(q.trim()))
}

fn query_matches(q: &str) -> bool {
    if let Some(rest) = q.strip_prefix("not ") {
        return !query_matches(rest.trim());
    }
    if q.contains("prefers-color-scheme: dark") {
        return false;
    }
    let kind = q.split(|c: char| c.is_whitespace() || c == '(').next().unwrap_or("");
    !matches!(kind, "print" | "speech")
}

/// Removes the stylesheet links whose `media` query does not apply here, so a print sheet or a dark-scheme
/// sheet is not applied to the screen. (The layout engine does not evaluate the `media` attribute of a link.)
pub(crate) fn drop_unmatched_stylesheet_links(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let (mut last, mut i) = (0, 0);
    while let Some(p) = lower[i..].find("<link") {
        let start = i + p;
        let Some(e) = lower[start..].find('>') else { break };
        let end = start + e + 1;
        if lower[start..end].contains("stylesheet") {
            if let Some(media) = attr_value(&html[start..end], "media") {
                if !media_matches(&media) {
                    out.push_str(&html[last..start]);
                    last = end;
                }
            }
        }
        i = end;
    }
    out.push_str(&html[last..]);
    out
}

#[cfg(test)]
mod stylesheet_media_tests {
    use super::{drop_unmatched_stylesheet_links as drop_links, media_matches};

    #[test]
    fn print_and_dark_sheets_do_not_apply_but_screen_and_plain_sheets_do() {
        assert!(!media_matches("print"));
        assert!(!media_matches("(prefers-color-scheme: dark)"));
        assert!(!media_matches("print, (prefers-color-scheme: dark)"));
        assert!(media_matches("screen"));
        assert!(media_matches("all"));
        assert!(media_matches(""));
        assert!(media_matches("not print"));
        assert!(media_matches("(min-width: 600px)"));
    }

    #[test]
    fn only_the_unmatched_stylesheet_links_are_removed() {
        let html = concat!(
            "<head>",
            "<link rel=\"stylesheet\" media=\"(prefers-color-scheme: dark)\" href=\"dark.css\">",
            "<link rel=\"stylesheet\" href=\"site.css\">",
            "<link rel='stylesheet' media='print' href='print.css'>",
            "<link rel=\"icon\" media=\"print\" href=\"i.png\">",
            "</head>"
        );
        let out = drop_links(html);
        assert!(!out.contains("dark.css") && !out.contains("print.css"));
        assert!(out.contains("site.css") && out.contains("i.png"));
    }
}
