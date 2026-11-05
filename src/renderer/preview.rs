use crate::renderer::fonts::{FontSystem, LoadedFontId, PathCmd};
use crate::renderer::layout::{LayoutPrimitive, PageLayout};
use egui::ColorImage;
use std::collections::HashMap;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform,
};

pub fn render_page_preview(
    page: &PageLayout,
    fonts: &FontSystem,
    scale: f32,
) -> Option<ColorImage> {
    let px_w = ((page.width_pt * scale).round() as u32).clamp(64, 4096);
    let px_h = ((page.height_pt * scale).round() as u32).clamp(64, 4096);

    let mut pixmap = Pixmap::new(px_w, px_h)?;
    pixmap.fill(Color::from_rgba8(
        page.background[0],
        page.background[1],
        page.background[2],
        255,
    ));

    let mut glyph_cache: HashMap<(LoadedFontId, u16), Vec<PathCmd>> = HashMap::new();

    for prim in &page.primitives {
        match prim {
            LayoutPrimitive::Rect {
                x,
                y,
                w,
                h,
                fill,
                stroke,
            } => {
                let rx = *x * scale;
                let ry = *y * scale;
                let rw = (*w * scale).max(0.5);
                let rh = (*h * scale).max(0.5);
                if let Some(rect) = Rect::from_xywh(rx, ry, rw, rh) {
                    if let Some(c) = fill {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(c[0], c[1], c[2], 255);
                        paint.anti_alias = true;
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                    if let Some((sc, sw)) = stroke {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(sc[0], sc[1], sc[2], 255);
                        paint.anti_alias = true;
                        let mut pb = PathBuilder::new();
                        pb.push_rect(rect);
                        if let Some(path) = pb.finish() {
                            let mut st = Stroke::default();
                            st.width = (*sw * scale).max(0.75);
                            pixmap.stroke_path(&path, &paint, &st, Transform::identity(), None);
                        }
                    }
                }
            }
            LayoutPrimitive::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
            } => {
                let mut pb = PathBuilder::new();
                pb.move_to(*x1 * scale, *y1 * scale);
                pb.line_to(*x2 * scale, *y2 * scale);
                if let Some(path) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(color[0], color[1], color[2], 255);
                    paint.anti_alias = true;
                    let mut st = Stroke::default();
                    st.width = (*width * scale).max(0.75);
                    st.line_cap = LineCap::Round;
                    pixmap.stroke_path(&path, &paint, &st, Transform::identity(), None);
                }
            }
            LayoutPrimitive::Glyph {
                font_id,
                glyph_id,
                x,
                baseline_y,
                size_pt,
                color,
                stroke_boost,
                slant,
            } => {
                let cmds = glyph_cache
                    .entry((*font_id, *glyph_id))
                    .or_insert_with(|| fonts.outline_glyph_em(*font_id, *glyph_id));
                if cmds.is_empty() {
                    continue;
                }
                if let Some(path) =
                    build_skia_glyph_path(cmds, *x, *baseline_y, *size_pt, *slant, scale)
                {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(color[0], color[1], color[2], 255);
                    paint.anti_alias = true;
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );

                    if *stroke_boost > 0.01 {
                        let mut st = Stroke::default();
                        st.width = (*size_pt * 0.048 * *stroke_boost * scale).max(0.35);
                        st.line_join = LineJoin::Round;
                        st.line_cap = LineCap::Round;
                        pixmap.stroke_path(&path, &paint, &st, Transform::identity(), None);
                    }
                }
            }
        }
    }

    let rgba = pixmap.take();
    Some(ColorImage::from_rgba_unmultiplied(
        [px_w as usize, px_h as usize],
        &rgba,
    ))
}

fn build_skia_glyph_path(
    cmds: &[PathCmd],
    origin_x: f32,
    baseline_y: f32,
    size_pt: f32,
    slant: f32,
    scale: f32,
) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let map_pt = |px: f32, py: f32| -> (f32, f32) {
        let tx = px + py * slant;
        let sx = (origin_x + tx * size_pt) * scale;
        let sy = (baseline_y - py * size_pt) * scale;
        (sx, sy)
    };

    for cmd in cmds {
        match *cmd {
            PathCmd::MoveTo(x, y) => {
                let (sx, sy) = map_pt(x, y);
                pb.move_to(sx, sy);
            }
            PathCmd::LineTo(x, y) => {
                let (sx, sy) = map_pt(x, y);
                pb.line_to(sx, sy);
            }
            PathCmd::QuadTo(x1, y1, x, y) => {
                let (sx1, sy1) = map_pt(x1, y1);
                let (sx, sy) = map_pt(x, y);
                pb.quad_to(sx1, sy1, sx, sy);
            }
            PathCmd::CurveTo(x1, y1, x2, y2, x, y) => {
                let (sx1, sy1) = map_pt(x1, y1);
                let (sx2, sy2) = map_pt(x2, y2);
                let (sx, sy) = map_pt(x, y);
                pb.cubic_to(sx1, sy1, sx2, sy2, sx, sy);
            }
            PathCmd::Close => {
                pb.close();
            }
        }
    }

    pb.finish()
}
