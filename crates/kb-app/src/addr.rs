//! Address helpers: typed input becomes a URL, URLs are split for the address bar, and two URLs
//! are compared as documents (fragments ignored).

pub fn normalize(input: &str) -> String {
    let s = input.trim();
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("kbp://") {
        return s.to_string();
    }
    if s.contains(' ') || !s.contains('.') {
        return format!("https://lite.duckduckgo.com/lite/?q={}", percent(s));
    }
    format!("https://{s}")
}

pub(crate) fn same_document(a: &str, b: &str) -> bool {
    let (Ok(mut a), Ok(mut b)) = (url::Url::parse(a), url::Url::parse(b)) else {
        return false;
    };
    if !matches!(a.scheme(), "http" | "https") {
        return false;
    }
    a.set_fragment(None);
    b.set_fragment(None);
    a == b
}

pub(crate) fn split_url(url: &str) -> (String, String) {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), String::new()),
    }
}

pub(crate) fn percent(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_words_become_searches_and_dotted_names_get_https() {
        assert_eq!(normalize("https://a.org/x"), "https://a.org/x");
        assert_eq!(normalize("kbp://home"), "kbp://home");
        assert_eq!(normalize("example.com"), "https://example.com");
        assert!(normalize("kindle browser")
            .starts_with("https://lite.duckduckgo.com/lite/?q=kindle%20browser"));
        assert!(normalize("kindle").starts_with("https://lite.duckduckgo.com/lite/?q=kindle"));
    }

    #[test]
    fn same_document_compares_origin_path_and_query_but_ignores_fragment() {
        assert!(same_document(
            "https://example.org/a?q=1#one",
            "https://example.org/a?q=1#two"
        ));
        for other in [
            "http://example.org/a?q=1",
            "https://example.org/b?q=1",
            "https://example.org/a?q=2",
            "https://other.org/a?q=1",
            "https://example.org:8443/a?q=1",
        ] {
            assert!(!same_document("https://example.org/a?q=1", other));
        }
        assert!(!same_document("kbp://home#one", "kbp://home#two"));
        assert!(!same_document("invalid", "invalid"));
    }

    #[test]
    fn url_splits_into_host_and_path_for_the_address_bar() {
        let (host, path) = split_url("https://example.com/doc/x?y=1");
        assert_eq!(host, "example.com");
        assert_eq!(path, "/doc/x?y=1");
        assert_eq!(
            split_url("https://example.org"),
            ("example.org".to_string(), String::new())
        );
    }

    #[test]
    fn percent_encoding_leaves_unreserved_characters_alone() {
        assert_eq!(percent("a b/c-d_e.f~g"), "a%20b%2Fc-d_e.f~g");
    }
}
