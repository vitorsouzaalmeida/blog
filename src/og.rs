use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};

use crate::config;
use crate::content::Post;

const REGULAR: &[u8] = include_bytes!("../assets/og-fonts/Merriweather-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../assets/og-fonts/Merriweather-Bold.ttf");

const W: u32 = 1200;
const H: u32 = 630;
const MARGIN: f32 = 96.0;
const TITLE_PX: f32 = 72.0;
const META_PX: f32 = 28.0;
const TITLE_TOP: f32 = 235.0;
const LINE_HEIGHT: f32 = 88.0;
const MAX_LINES: usize = 4;
const INK: [u8; 3] = [17, 17, 17];
const MUTED: [u8; 3] = [85, 85, 85];

pub struct Fonts {
    regular: FontRef<'static>,
    bold: FontRef<'static>,
}

impl Fonts {
    pub fn embedded() -> Self {
        Fonts {
            regular: FontRef::try_from_slice(REGULAR).expect("embedded regular font"),
            bold: FontRef::try_from_slice(BOLD).expect("embedded bold font"),
        }
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::embedded()
    }
}

/// A `W` by `H` RGBA8 buffer. Every pixel is opaque, so alpha is written once
/// at creation and never touched again.
struct Canvas {
    px: Vec<u8>,
}

impl Canvas {
    fn white() -> Canvas {
        Canvas {
            px: vec![255; (W * H * 4) as usize],
        }
    }

    fn at(x: u32, y: u32) -> usize {
        ((y * W + x) * 4) as usize
    }

    fn get(&self, x: u32, y: u32) -> [u8; 3] {
        let i = Canvas::at(x, y);
        [self.px[i], self.px[i + 1], self.px[i + 2]]
    }

    fn set(&mut self, x: u32, y: u32, rgb: [u8; 3]) {
        let i = Canvas::at(x, y);
        self.px[i..i + 3].copy_from_slice(&rgb);
    }
}

fn measure(font: &FontRef, text: &str, px: f32) -> f32 {
    let sf = font.as_scaled(PxScale::from(px));
    text.chars()
        .map(|ch| font.glyph_id(ch))
        .fold((0.0, None), |(width, prev): (f32, Option<GlyphId>), gid| {
            let kern = prev.map(|p| sf.kern(p, gid)).unwrap_or(0.0);
            (width + kern + sf.h_advance(gid), Some(gid))
        })
        .0
}

fn draw_text(
    img: &mut Canvas,
    font: &FontRef,
    text: &str,
    start_x: f32,
    baseline_y: f32,
    px: f32,
    color: [u8; 3],
) {
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let mut x = start_x;
    let mut prev: Option<GlyphId> = None;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        if let Some(p) = prev {
            x += sf.kern(p, gid);
        }
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let px_x = bounds.min.x as i32 + gx as i32;
                let px_y = bounds.min.y as i32 + gy as i32;
                if px_x >= 0 && px_y >= 0 && (px_x as u32) < W && (px_y as u32) < H {
                    let bg = img.get(px_x as u32, px_y as u32);
                    let a = cov.clamp(0.0, 1.0);
                    let blend = |c: u8, b: u8| ((c as f32) * a + (b as f32) * (1.0 - a)) as u8;
                    img.set(
                        px_x as u32,
                        px_y as u32,
                        [
                            blend(color[0], bg[0]),
                            blend(color[1], bg[1]),
                            blend(color[2], bg[2]),
                        ],
                    );
                }
            });
        }
        x += sf.h_advance(gid);
        prev = Some(gid);
    }
}

fn wrap(font: &FontRef, text: &str, px: f32, max_w: f32) -> Vec<String> {
    text.split_whitespace()
        .fold(Vec::new(), |mut lines: Vec<String>, word| {
            match lines.last() {
                Some(line) if measure(font, &format!("{line} {word}"), px) <= max_w => {
                    let line = lines.last_mut().expect("checked above");
                    line.push(' ');
                    line.push_str(word);
                }
                _ => lines.push(word.to_string()),
            }
            lines
        })
}

pub fn render(fonts: &Fonts, post: &Post) -> Vec<u8> {
    let mut img = Canvas::white();

    draw_text(
        &mut img,
        &fonts.regular,
        config::WEBSITE,
        MARGIN,
        130.0,
        META_PX,
        INK,
    );

    let max_w = W as f32 - MARGIN * 2.0;
    for (i, line) in wrap(&fonts.bold, &post.title, TITLE_PX, max_w)
        .iter()
        .take(MAX_LINES)
        .enumerate()
    {
        let y = TITLE_TOP + LINE_HEIGHT * i as f32;
        draw_text(&mut img, &fonts.bold, line, MARGIN, y, TITLE_PX, INK);
    }

    let tags = post
        .tags
        .iter()
        .map(|t| format!("#{t}"))
        .collect::<Vec<_>>()
        .join("  ");
    let bottom = H as f32 - 70.0;
    draw_text(
        &mut img,
        &fonts.regular,
        &tags,
        MARGIN,
        bottom,
        META_PX,
        MUTED,
    );

    let date = post.pub_date.to_string();
    let dw = measure(&fonts.regular, &date, META_PX);
    draw_text(
        &mut img,
        &fonts.regular,
        &date,
        W as f32 - MARGIN - dw,
        bottom,
        META_PX,
        MUTED,
    );

    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, W, H);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("png header")
        .write_image_data(&img.px)
        .expect("encode png");
    out
}
