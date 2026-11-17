//! Offline copies: the page's stylesheets and images are stored inside the saved HTML (as `<style>` elements
//! and `data:` URLs), and its scripts are left out. The copy then opens without a network and looks as it did.
//! Anything that cannot be fetched (or is over the budget) keeps its address, so it still loads when online.

use std::collections::HashMap;

use url::Url;

use crate::brand::base64;

/// One fetched resource: its content type and bytes.
pub(crate) type Fetched = (String, Vec<u8>);

/// Store the page's stylesheets and images in `html`. `fetch` returns the content type and bytes of an
/// address, or `None` when it cannot be fetched. `budget` is the most image and style bytes to store.
pub(crate) fn archive_html(html: &str, base: &str, fetch: &mut dyn FnMut(&str) -> Option<Fetched>, budget: usize) -> String {
    let base_url = Url::parse(base).ok();
    let mut cache: HashMap<String, Option<Fetched>> = HashMap::new();
    let mut used = 0usize;
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let lower = rest.to_ascii_lowercase();
        let next = ["<script", "<img", "<link"]
            .iter()
            .filter_map(|tag| lower.find(tag).map(|at| (at, *tag)))
            .min_by_key(|(at, _)| *at);
        let Some((at, tag)) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..at]);
        let Some(close) = rest[at..].find('>') else {
            out.push_str(&rest[at..]);
            break;
        };
        let element = &rest[at..at + close + 1];
        let after = &rest[at + close + 1..];
        match tag {
            // Scripts are not kept in an offline copy. A script's end tag is removed with it.
            "<script" => {
                rest = match lower[at..].find("</script>") {
                    Some(end) => &rest[at + end + "</script>".len()..],
                    None => "",
                };
            }
            "<img" => {
                let src = attr(element, "src");
                let replaced = src
                    .filter(|s| !s.starts_with("data:"))
                    .and_then(|src| resolve(base_url.as_ref(), &src))
                    .and_then(|abs| fetch_cached(&abs, fetch, &mut cache, &mut used, budget))
                    .map(|(ct, bytes)| {
                        let kind = if ct.is_empty() { "image/png" } else { ct.as_str() };
                        set_attr(element, "src", &format!("data:{kind};base64,{}", base64(&bytes)))
                    });
                out.push_str(&replaced.unwrap_or_else(|| element.to_string()));
                rest = after;
            }
            _ => {
                // <link>: a stylesheet becomes a <style> element; other links are dropped (they are not needed
                // to show the page).
                let is_style = attr(element, "rel").map_or(false, |r| r.to_ascii_lowercase().contains("stylesheet"));
                if is_style {
                    let css = attr(element, "href")
                        .and_then(|href| resolve(base_url.as_ref(), &href))
                        .and_then(|abs| fetch_cached(&abs, fetch, &mut cache, &mut used, budget));
                    if let Some((_, bytes)) = css {
                        let text = String::from_utf8_lossy(&bytes).replace("</style", "<\\/style");
                        out.push_str(&format!("<style>{text}</style>"));
                    } else {
                        out.push_str(element);
                    }
                } else {
                    out.push_str(element);
                }
                rest = after;
            }
        }
    }
    out
}

fn fetch_cached(
    url: &str,
    fetch: &mut dyn FnMut(&str) -> Option<Fetched>,
    cache: &mut HashMap<String, Option<Fetched>>,
    used: &mut usize,
    budget: usize,
) -> Option<Fetched> {
    if let Some(hit) = cache.get(url) {
        return hit.clone();
    }
    let got = fetch(url).filter(|(_, bytes)| *used + bytes.len() <= budget);
    if let Some((_, bytes)) = &got {
        *used += bytes.len();
    }
    cache.insert(url.to_string(), got.clone());
    got
}

fn resolve(base: Option<&Url>, value: &str) -> Option<String> {
    let joined = base?.join(value).ok()?;
    matches!(joined.scheme(), "http" | "https").then(|| joined.to_string())
}

/// The value of attribute `name` in an element's start tag (quoted or bare).
pub(crate) fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        let before_ok = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric() && lower.as_bytes()[at - 1] != b'-';
        let rest = &tag[at + name.len()..];
        if before_ok {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                let value = value.trim_start();
                return Some(match value.chars().next() {
                    Some(q @ ('"' | '\'')) => value[1..].split(q).next().unwrap_or("").to_string(),
                    _ => value.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("").to_string(),
                });
            }
        }
        from = at + name.len();
    }
    None
}

/// The element's start tag with attribute `name` set to `value` (added when missing).
fn set_attr(tag: &str, name: &str, value: &str) -> String {
    let escaped = value.replace('"', "&quot;");
    let lower = tag.to_ascii_lowercase();
    if let Some(at) = lower.find(&format!(" {name}=")) {
        let start = at + name.len() + 2;
        let quote = tag[start..].chars().next().unwrap_or('"');
        let (value_start, value_end) = match quote {
            '"' | '\'' => {
                let inner = start + 1;
                let end = inner + tag[inner..].find(quote).unwrap_or(0);
                (inner, end)
            }
            _ => {
                let end = start + tag[start..].find(|c: char| c.is_whitespace() || c == '>').unwrap_or(tag.len() - start);
                (start, end)
            }
        };
        return format!("{}{escaped}{}", &tag[..value_start], &tag[value_end..]);
    }
    let close = tag.rfind('>').unwrap_or(tag.len());
    format!("{} {name}=\"{escaped}\"{}", &tag[..close], &tag[close..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fetcher(files: &'static [(&'static str, &'static str, &'static [u8])]) -> impl FnMut(&str) -> Option<Fetched> {
        move |url: &str| files.iter().find(|(u, _, _)| *u == url).map(|(_, ct, b)| (ct.to_string(), b.to_vec()))
    }

    #[test]
    fn stylesheets_and_images_are_stored_and_scripts_dropped() {
        static FILES: &[(&str, &str, &[u8])] = &[
            ("https://example.org/css/site.css", "text/css", b"p{color:red}"),
            ("https://example.org/img/a.png", "image/png", b"\x89PNG"),
        ];
        let html = r#"<html><head><link rel="stylesheet" href="css/site.css"><script src="x.js"></script><script>alert(1)</script></head><body><img src="img/a.png" alt="A"><img src="missing.png" alt="M"></body></html>"#;
        let out = archive_html(html, "https://example.org/page", &mut fetcher(FILES), 1 << 20);
        assert!(out.contains("<style>p{color:red}</style>"), "{out}");
        assert!(out.contains("src=\"data:image/png;base64,iVBORw==\""), "{out}");
        assert!(out.contains("src=\"missing.png\""), "unfetched image keeps its address: {out}");
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("<link"), "{out}");
    }

    #[test]
    fn the_budget_limits_what_is_stored() {
        static FILES: &[(&str, &str, &[u8])] = &[("https://example.org/a.png", "image/png", &[0u8; 100])];
        let html = r#"<img src="https://example.org/a.png">"#;
        let out = archive_html(html, "https://example.org/", &mut fetcher(FILES), 50);
        assert!(out.contains("src=\"https://example.org/a.png\""), "{out}");
    }

    #[test]
    fn attributes_are_read_quoted_and_bare() {
        assert_eq!(attr(r#"<img src="a.png" alt='x y'>"#, "alt").as_deref(), Some("x y"));
        assert_eq!(attr("<img src=a.png>", "src").as_deref(), Some("a.png"));
        assert_eq!(attr(r#"<img data-src="no">"#, "src"), None);
    }
}
