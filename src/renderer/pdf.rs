use crate::renderer::fonts::{FontSystem, LoadedFontId, PathCmd};
use crate::renderer::layout::{DocumentLayout, LayoutPrimitive, PageLayout};
use miniz_oxide::deflate::compress_to_vec_zlib;
use pdf_writer::types::{
    ActionType, AnnotationType, BorderType, LineCapStyle, LineJoinStyle, ProcSet,
};
use pdf_writer::{Content, Filter, Pdf, Rect as PdfRect, Ref, Str, TextStr};
use std::collections::HashMap;

pub fn export_pdf_bytes(layout: &DocumentLayout, fonts: &FontSystem) -> Vec<u8> {
    let mut pdf = Pdf::new();

    let catalog_id = Ref::new(1);
    let page_tree_id = Ref::new(2);
    let mut next_id = 3i32;
    let mut alloc_ref = || {
        let r = Ref::new(next_id);
        next_id += 1;
        r
    };

    let mut page_ids: Vec<Ref> = Vec::with_capacity(layout.pages.len());
    let mut content_ids: Vec<Ref> = Vec::with_capacity(layout.pages.len());
    let mut page_annot_ids: Vec<Vec<Ref>> = Vec::with_capacity(layout.pages.len());

    for page in &layout.pages {
        page_ids.push(alloc_ref());
        content_ids.push(alloc_ref());
        let mut a_ids = Vec::with_capacity(page.links.len());
        for _ in &page.links {
            a_ids.push(alloc_ref());
        }
        page_annot_ids.push(a_ids);
    }

    // Optional PDF Outline (Bookmarks) if headings exist
    let outline_root_id = if !layout.headings.is_empty() {
        Some(alloc_ref())
    } else {
        None
    };
    let outline_item_ids: Vec<Ref> = layout.headings.iter().map(|_| alloc_ref()).collect();

    // 1. Catalog
    {
        let mut cat = pdf.catalog(catalog_id);
        cat.pages(page_tree_id);
        if let Some(out_id) = outline_root_id {
            cat.outlines(out_id);
        }
    }

    // 2. Page Tree
    {
        let mut pages = pdf.pages(page_tree_id);
        pages.kids(page_ids.iter().copied());
        pages.count(page_ids.len() as i32);
    }

    // 3. Outlines (Bookmarks)
    if let Some(out_root) = outline_root_id {
        let count = outline_item_ids.len();
        {
            let mut outlines = pdf.outline(out_root);
            outlines.first(outline_item_ids[0]);
            outlines.last(outline_item_ids[count - 1]);
            outlines.count(count as i32);
        }
        for (i, h) in layout.headings.iter().enumerate() {
            let item_id = outline_item_ids[i];
            let page_id = page_ids
                .get(h.page_index)
                .copied()
                .unwrap_or(page_ids[0]);
            let page_h = layout
                .pages
                .get(h.page_index)
                .map(|p| p.height_pt)
                .unwrap_or(841.89);

            let mut item = pdf.outline_item(item_id);
            item.title(TextStr(&h.title));
            item.parent(out_root);
            if i > 0 {
                item.prev(outline_item_ids[i - 1]);
            }
            if i + 1 < count {
                item.next(outline_item_ids[i + 1]);
            }
            item.dest()
                .page(page_id)
                .xyz(0.0, (page_h - h.y_pt).max(0.0), None);
        }
    }

    let mut glyph_cache: HashMap<(LoadedFontId, u16), Vec<PathCmd>> = HashMap::new();

    // 4. Pages, Content Streams, and Link Annotations
    for (idx, page) in layout.pages.iter().enumerate() {
        let page_id = page_ids[idx];
        let content_id = content_ids[idx];
        let annot_ids = &page_annot_ids[idx];

        let content_bytes = build_page_content_stream(page, fonts, &mut glyph_cache);
        let compressed = compress_to_vec_zlib(&content_bytes, 6);

        {
            let mut stream = pdf.stream(content_id, &compressed);
            stream.filter(Filter::FlateDecode);
        }

        // Link annotations
        for (l_idx, link) in page.links.iter().enumerate() {
            let aid = annot_ids[l_idx];
            let y1 = page.height_pt - (link.y + link.h);
            let y2 = page.height_pt - link.y;
            let mut annot = pdf.annotation(aid);
            annot.subtype(AnnotationType::Link);
            annot.rect(PdfRect::new(link.x, y1, link.x + link.w, y2));
            annot.border(0.0, 0.0, 0.0, None);
            annot. border_style().width(0.0).style(BorderType::Solid);
            annot
                .action()
                .action_type(ActionType::Uri)
                .uri(Str(link.url.as_bytes()));
        }

        {
            let mut pdf_page = pdf.page(page_id);
            pdf_page.parent(page_tree_id);
            pdf_page.media_box(PdfRect::new(0.0, 0.0, page.width_pt, page.height_pt));
            pdf_page.contents(content_id);
            if !annot_ids.is_empty() {
                pdf_page.annotations(annot_ids.iter().copied());
            }
            pdf_page.resources().proc_sets([ProcSet::Pdf]);
        }
    }

    pdf.finish()
}

fn build_page_content_stream(
    page: &PageLayout,
    fonts: &FontSystem,
    glyph_cache: &mut HashMap<(LoadedFontId, u16), Vec<PathCmd>>,
) -> Vec<u8> {
    let mut content = Content::new();
    let page_h = page.height_pt;

    content.set_line_join(LineJoinStyle::RoundJoin);
    content.set_line_cap(LineCapStyle::RoundCap);

    // Fill page background if non-white or always for consistency
    let bg = to_rgb_f32(page.background);
    content.set_fill_rgb(bg.0, bg.1, bg.2);
    content.rect(0.0, 0.0, page.width_pt, page.height_pt);
    content.fill_nonzero();

    let mut cur_fill: Option<[u8; 3]> = Some(page.background);
    let mut cur_stroke: Option<[u8; 3]> = None;
    let mut cur_line_w: f32 = -1.0;

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
                let pdf_y = page_h - (*y + *h);
                match (fill, stroke) {
                    (Some(fc), Some((sc, sw))) => {
                        if cur_fill != Some(*fc) {
                            let (r, g, b) = to_rgb_f32(*fc);
                            content.set_fill_rgb(r, g, b);
                            cur_fill = Some(*fc);
                        }
                        if cur_stroke != Some(*sc) {
                            let (r, g, b) = to_rgb_f32(*sc);
                            content.set_stroke_rgb(r, g, b);
                            cur_stroke = Some(*sc);
                        }
                        if (cur_line_w - *sw).abs() > 0.005 {
                            content.set_line_width(*sw);
                            cur_line_w = *sw;
                        }
                        content.rect(*x, pdf_y, *w, *h);
                        content.fill_nonzero_and_stroke();
                    }
                    (Some(fc), None) => {
                        if cur_fill != Some(*fc) {
                            let (r, g, b) = to_rgb_f32(*fc);
                            content.set_fill_rgb(r, g, b);
                            cur_fill = Some(*fc);
                        }
                        content.rect(*x, pdf_y, *w, *h);
                        content.fill_nonzero();
                    }
                    (None, Some((sc, sw))) => {
                        if cur_stroke != Some(*sc) {
                            let (r, g, b) = to_rgb_f32(*sc);
                            content.set_stroke_rgb(r, g, b);
                            cur_stroke = Some(*sc);
                        }
                        if (cur_line_w - *sw).abs() > 0.005 {
                            content.set_line_width(*sw);
                            cur_line_w = *sw;
                        }
                        content.rect(*x, pdf_y, *w, *h);
                        content.stroke();
                    }
                    (None, None) => {}
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
                if cur_stroke != Some(*color) {
                    let (r, g, b) = to_rgb_f32(*color);
                    content.set_stroke_rgb(r, g, b);
                    cur_stroke = Some(*color);
                }
                if (cur_line_w - *width).abs() > 0.005 {
                    content.set_line_width(*width);
                    cur_line_w = *width;
                }
                content.move_to(*x1, page_h - *y1);
                content.line_to(*x2, page_h - *y2);
                content.stroke();
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

                if cur_fill != Some(*color) {
                    let (r, g, b) = to_rgb_f32(*color);
                    content.set_fill_rgb(r, g, b);
                    cur_fill = Some(*color);
                }

                let use_stroke = *stroke_boost > 0.01;
                if use_stroke {
                    if cur_stroke != Some(*color) {
                        let (r, g, b) = to_rgb_f32(*color);
                        content.set_stroke_rgb(r, g, b);
                        cur_stroke = Some(*color);
                    }
                    let sw = (*size_pt * 0.048 * *stroke_boost).max(0.15);
                    if (cur_line_w - sw).abs() > 0.005 {
                        content.set_line_width(sw);
                        cur_line_w = sw;
                    }
                }

                let pdf_base_y = page_h - *baseline_y;
                emit_pdf_glyph_path(&mut content, cmds, *x, pdf_base_y, *size_pt, *slant);
                if use_stroke {
                    content.fill_nonzero_and_stroke();
                } else {
                    content.fill_nonzero();
                }
            }
        }
    }

    content.finish()
}

fn emit_pdf_glyph_path(
    content: &mut Content,
    cmds: &[PathCmd],
    origin_x: f32,
    pdf_base_y: f32,
    size_pt: f32,
    slant: f32,
) {
    let map_pt = |px: f32, py: f32| -> (f32, f32) {
        let tx = px + py * slant;
        (origin_x + tx * size_pt, pdf_base_y + py * size_pt)
    };

    let mut last_x = origin_x;
    let mut last_y = pdf_base_y;

    for cmd in cmds {
        match *cmd {
            PathCmd::MoveTo(x, y) => {
                let (px, py) = map_pt(x, y);
                content.move_to(px, py);
                last_x = px;
                last_y = py;
            }
            PathCmd::LineTo(x, y) => {
                let (px, py) = map_pt(x, y);
                content.line_to(px, py);
                last_x = px;
                last_y = py;
            }
            PathCmd::QuadTo(x1, y1, x, y) => {
                let (qx1, qy1) = map_pt(x1, y1);
                let (qx, qy) = map_pt(x, y);
                // Exact degree elevation from quadratic Bezier to cubic Bezier
                let c1x = last_x + (2.0 / 3.0) * (qx1 - last_x);
                let c1y = last_y + (2.0 / 3.0) * (qy1 - last_y);
                let c2x = qx + (2.0 / 3.0) * (qx1 - qx);
                let c2y = qy + (2.0 / 3.0) * (qy1 - qy);
                content.cubic_to(c1x, c1y, c2x, c2y, qx, qy);
                last_x = qx;
                last_y = qy;
            }
            PathCmd::CurveTo(x1, y1, x2, y2, x, y) => {
                let (c1x, c1y) = map_pt(x1, y1);
                let (c2x, c2y) = map_pt(x2, y2);
                let (px, py) = map_pt(x, y);
                content.cubic_to(c1x, c1y, c2x, c2y, px, py);
                last_x = px;
                last_y = py;
            }
            PathCmd::Close => {
                content.close_path();
            }
        }
    }
}

fn to_rgb_f32(c: [u8; 3]) -> (f32, f32, f32) {
    (
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
    )
}
