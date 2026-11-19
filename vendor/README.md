# Vendored crates

`rquickjs-sys-0.14.0` is a copy of the crates.io release with bindings for static musl builds.
The ARMv7 musl targets use the complete QuickJS declarations from `armv7-unknown-linux-gnueabihf`.
Their C data layouts match, and `extern "C"` uses each target's calling convention. The bundled
`armv7-unknown-linux-gnueabi` file has older declarations, including a different `JS_NewArrayBuffer`
signature. Remove this patch when the upstream crate ships current musl bindings.

## blitz-dom 0.3.0-beta.4

A copy of the crates.io release with three guards, marked `KB patch` in the source. Page scripts
remove nodes while their resources are still loading; upstream then indexes a missing node and
panics, and a panic inside `handle_messages` leaves its message receiver unset, so every later
call panics too.

- `document.rs`, `upsert_stylesheet_for_node` and `add_stylesheet_for_node`: a stylesheet that
  arrives for a removed node is dropped.
- `document.rs`, `handle_messages`: returns when the receiver is missing instead of unwrapping.
- `query_selector.rs`, `query_selector_in_raw` and `query_selector_all_in_raw`: a removed scope
  node has no matches.

Remove the patch (and the `[patch.crates-io]` line) when upstream handles removed nodes.
