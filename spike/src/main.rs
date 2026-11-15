//! M0 spike: render an HTML file with Blitz (Stylo + Taffy + vello_cpu) to an
//! 8-bit grayscale PNG, the format an e-ink panel wants.
//!
//! usage: kb-spike <input.html> <out.png> [width] [height] [scale]
use std::{env, fs, io::BufWriter, time::Instant};

use anyrender_vello_cpu::VelloCpuScenePainter;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};

fn fetch(url: &str, out: &str) {
    let t0 = Instant::now();
    let mut resp = ureq::get(url)
        .header("User-Agent", "Mozilla/5.0 (Linux; Kindle) kb-spike/0.0")
        .call()
        .expect("request failed");
    let status = resp.status();
    let body = resp.body_mut().read_to_vec().expect("read body");
    fs::write(out, &body).expect("write body");
    eprintln!("fetch status={status} bytes={} time={:?}", body.len(), t0.elapsed());
}

/// Fonts come from explicit files (no system font discovery: the Kindle has
/// none, and fontconfig can't be cross-compiled). Set KB_FONTS to a comma list.
fn load_font_context() -> blitz_dom::FontContext {
    let list = env::var("KB_FONTS").unwrap_or_else(|_| {
        [
            "NotoSans-Regular.ttf",
            "NotoSans-Bold.ttf",
            "NotoSans-Italic.ttf",
            "NotoSerif-Regular.ttf",
            "NotoSerif-Bold.ttf",
        ]
        .iter()
        .map(|f| format!("/Volumes/Kindle/koreader/fonts/noto/{f}"))
        .collect::<Vec<_>>()
        .join(",")
    });
    let paths: Vec<String> = list.split(',').map(str::to_owned).collect();
    let mut font_ctx = blitz_dom::FontContext::default();
    font_ctx.collection.load_fonts_from_paths(&paths);
    // Map generic families onto the loaded families.
    let sans = font_ctx.collection.family_id("Noto Sans");
    let serif = font_ctx.collection.family_id("Noto Serif");
    if let Some(id) = sans {
        font_ctx
            .collection
            .set_generic_families(parley::GenericFamily::SansSerif, std::iter::once(id));
        font_ctx.collection.set_generic_families(parley::GenericFamily::SystemUi, std::iter::once(id));
    }
    if let Some(id) = serif {
        font_ctx
            .collection
            .set_generic_families(parley::GenericFamily::Serif, std::iter::once(id));
    }
    font_ctx
}

fn main() {
    let args: Vec<String> = env::args().collect();
    // `kb-spike fetch <url> <out.html>`: TLS smoke test (rustls, bundled roots).
    if args.get(1).map(String::as_str) == Some("fetch") {
        fetch(&args[2], &args[3]);
        return;
    }
    if args.len() < 3 {
        eprintln!("usage: kb-spike <input.html> <out.png> [width] [height] [scale]");
        std::process::exit(2);
    }
    let width: u32 = args.get(3).map_or(1264, |s| s.parse().unwrap());
    let height: u32 = args.get(4).map_or(1680, |s| s.parse().unwrap());
    let scale: f32 = args.get(5).map_or(1.0, |s| s.parse().unwrap());

    let html = fs::read_to_string(&args[1]).expect("read html");
    let t0 = Instant::now();

    let viewport = Viewport::new(width, height, scale, ColorScheme::Light);
    let font_ctx = load_font_context();
    // A real base URL is required: blitz-dom panics resolving relative
    // stylesheet links against the default data: base. No network yet, so
    // those links simply stay unloaded.
    let config = DocumentConfig {
        viewport: Some(viewport),
        font_ctx: Some(font_ctx),
        base_url: Some("https://kb.invalid/".to_string()),
        ..Default::default()
    };
    let mut doc = HtmlDocument::from_html(&html, config);
    let t_parse = t0.elapsed();

    // Stylo style resolution + Taffy layout.
    doc.resolve(0.0);
    let t_layout = t0.elapsed();

    let mut painter = VelloCpuScenePainter::new(width as u16, height as u16);
    blitz_paint::paint_scene(&mut painter, &mut doc, scale as f64, width, height, 0, 0);
    let pixmap = painter.finish();
    let t_paint = t0.elapsed();

    // Composite premultiplied RGBA over white, then convert to luma.
    let px = pixmap.data_as_u8_slice();
    let mut gray = Vec::with_capacity((width * height) as usize);
    for p in px.chunks_exact(4) {
        let a = p[3] as u16;
        let inv = 255 - a;
        let r = (p[0] as u16 + inv).min(255) as f32;
        let g = (p[1] as u16 + inv).min(255) as f32;
        let b = (p[2] as u16 + inv).min(255) as f32;
        gray.push((0.299 * r + 0.587 * g + 0.114 * b).round() as u8);
    }

    let file = fs::File::create(&args[2]).expect("create png");
    let mut enc = png::Encoder::new(BufWriter::new(file), width, height);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(&gray).expect("png data");
    writer.finish().expect("png finish");

    // Peak resident memory (Linux only; the Kindle exposes /proc).
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmHWM") || line.starts_with("VmRSS") {
                eprintln!("{line}");
            }
        }
    }
    eprintln!(
        "parse {:?} | layout {:?} | paint {:?} | total {:?}",
        t_parse,
        t_layout - t_parse,
        t_paint - t_layout,
        t0.elapsed()
    );
}
