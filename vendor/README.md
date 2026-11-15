# Vendored crates

`rquickjs-sys-0.14.0` is a copy of the crates.io release with one change: the
pre-generated bindings for `armv7-unknown-linux-musleabihf` are a copy of the
`armv7-unknown-linux-gnueabihf` file. The Kindle needs a static musl build, and
the C struct layouts are the same for both targets. Remove this patch if the
upstream crate ships musl bindings.
