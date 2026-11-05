use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    SpreadMode, Stroke, Transform,
};

/// Renders the custom "MRP" (Markdown + Rust + PDF) fusion icon at arbitrary resolution `size x size`.
pub fn render_mrp_icon_rgba(size: u32) -> Vec<u8> {
    let mut pixmap = Pixmap::new(size, size).expect("Failed to allocate icon pixmap");
    let s = size as f32 / 256.0;
    let ts = Transform::from_scale(s, s);

    // 1. Rounded Squircle App Badge Background (Dark Slate -> Deep Navy gradient)
    if let Some(bg_path) = rounded_rect_path(10.0, 10.0, 236.0, 236.0, 52.0) {
        let mut bg_paint = Paint::default();
        bg_paint.anti_alias = true;
        bg_paint.shader = LinearGradient::new(
            Point::from_xy(10.0, 10.0),
            Point::from_xy(246.0, 246.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(17, 24, 39, 255)),
                GradientStop::new(0.55, Color::from_rgba8(15, 23, 42, 255)),
                GradientStop::new(1.0, Color::from_rgba8(30, 58, 138, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        pixmap.fill_path(&bg_path, &bg_paint, FillRule::Winding, ts, None);

        // Outer metallic border (Blue -> Rust Orange gradient)
        let mut border_paint = Paint::default();
        border_paint.anti_alias = true;
        border_paint.shader = LinearGradient::new(
            Point::from_xy(10.0, 10.0),
            Point::from_xy(246.0, 246.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(56, 189, 248, 255)),
                GradientStop::new(0.5, Color::from_rgba8(59, 130, 246, 255)),
                GradientStop::new(1.0, Color::from_rgba8(249, 115, 22, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        let stroke = Stroke {
            width: 6.5,
            ..Default::default()
        };
        pixmap.stroke_path(&bg_path, &border_paint, &stroke, ts, None);
    }

    // 2. Glowing "Maru" (Circle Ring) + Rust Gear Silhouette in Center-Back
    draw_rust_gear(&mut pixmap, 128.0, 128.0, 96.0, 82.0, 16, ts);
    if let Some(circle_path) = PathBuilder::from_circle(128.0, 128.0, 88.0) {
        let mut ring_paint = Paint::default();
        ring_paint.anti_alias = true;
        ring_paint.shader = LinearGradient::new(
            Point::from_xy(40.0, 40.0),
            Point::from_xy(216.0, 216.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(56, 189, 248, 175)),
                GradientStop::new(0.5, Color::from_rgba8(249, 115, 22, 175)),
                GradientStop::new(1.0, Color::from_rgba8(244, 63, 94, 175)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        let ring_stroke = Stroke {
            width: 5.0,
            ..Default::default()
        };
        pixmap.stroke_path(&circle_path, &ring_paint, &ring_stroke, ts, None);
    }

    // 3. Crisp Folded PDF Document Sheet (representing "P" = PDF)
    draw_folded_document(&mut pixmap, ts);

    // 4. Bold Interlocking "M R P" Monogram Emblem + Markdown Down-Arrow
    draw_mrp_monogram(&mut pixmap, ts);

    pixmap.data().to_vec()
}

fn draw_rust_gear(
    pixmap: &mut Pixmap,
    cx: f32,
    cy: f32,
    outer_r: f32,
    inner_r: f32,
    teeth: usize,
    ts: Transform,
) {
    let mut pb = PathBuilder::new();
    let total_steps = teeth * 4;
    for i in 0..total_steps {
        let angle = (i as f32 / total_steps as f32) * std::f32::consts::TAU;
        let r = if i % 4 == 0 || i % 4 == 1 {
            outer_r
        } else {
            inner_r
        };
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();

    if let Some(gear_path) = pb.finish() {
        let mut paint = Paint::default();
        paint.anti_alias = true;
        paint.set_color_rgba8(249, 115, 22, 42);
        pixmap.fill_path(&gear_path, &paint, FillRule::Winding, ts, None);

        let mut stroke_paint = Paint::default();
        stroke_paint.anti_alias = true;
        stroke_paint.set_color_rgba8(251, 146, 60, 85);
        let stroke = Stroke {
            width: 3.0,
            ..Default::default()
        };
        pixmap.stroke_path(&gear_path, &stroke_paint, &stroke, ts, None);
    }
}

fn draw_folded_document(pixmap: &mut Pixmap, ts: Transform) {
    // Document body with top-right folded corner
    let x0 = 44.0;
    let y0 = 32.0;
    let x1 = 212.0;
    let y1 = 224.0;
    let fold = 36.0;

    let mut pb = PathBuilder::new();
    pb.move_to(x0, y0);
    pb.line_to(x1 - fold, y0);
    pb.line_to(x1, y0 + fold);
    pb.line_to(x1, y1);
    pb.line_to(x0, y1);
    pb.close();

    if let Some(doc_path) = pb.finish() {
        let mut fill_paint = Paint::default();
        fill_paint.anti_alias = true;
        fill_paint.shader = LinearGradient::new(
            Point::from_xy(x0, y0),
            Point::from_xy(x1, y1),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(30, 41, 59, 235)),
                GradientStop::new(1.0, Color::from_rgba8(15, 23, 42, 245)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        pixmap.fill_path(&doc_path, &fill_paint, FillRule::Winding, ts, None);

        let mut edge_paint = Paint::default();
        edge_paint.anti_alias = true;
        edge_paint.set_color_rgba8(148, 163, 184, 140);
        let stroke = Stroke {
            width: 3.5,
            ..Default::default()
        };
        pixmap.stroke_path(&doc_path, &edge_paint, &stroke, ts, None);
    }

    // Folded corner flap
    let mut flap_pb = PathBuilder::new();
    flap_pb.move_to(x1 - fold, y0);
    flap_pb.line_to(x1 - fold, y0 + fold);
    flap_pb.line_to(x1, y0 + fold);
    flap_pb.close();
    if let Some(flap_path) = flap_pb.finish() {
        let mut flap_paint = Paint::default();
        flap_paint.anti_alias = true;
        flap_paint.set_color_rgba8(56, 189, 248, 190);
        pixmap.fill_path(&flap_path, &flap_paint, FillRule::Winding, ts, None);
    }
}

fn draw_mrp_monogram(pixmap: &mut Pixmap, ts: Transform) {
    // Top Markdown Ribbon Badge ("M ↓")
    if let Some(md_badge) = rounded_rect_path(58.0, 48.0, 96.0, 34.0, 8.0) {
        let mut p = Paint::default();
        p.anti_alias = true;
        p.set_color_rgba8(14, 165, 233, 45);
        pixmap.fill_path(&md_badge, &p, FillRule::Winding, ts, None);
    }

    // Markdown down-arrow on top-right of the header area
    let mut arrow_pb = PathBuilder::new();
    arrow_pb.move_to(132.0, 55.0);
    arrow_pb.line_to(132.0, 73.0);
    arrow_pb.move_to(124.0, 66.0);
    arrow_pb.line_to(132.0, 75.0);
    arrow_pb.line_to(140.0, 66.0);
    if let Some(arrow_path) = arrow_pb.finish() {
        let mut p = Paint::default();
        p.anti_alias = true;
        p.set_color_rgba8(56, 189, 248, 255);
        let stroke = Stroke {
            width: 5.5,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        pixmap.stroke_path(&arrow_path, &p, &stroke, ts, None);
    }

    // Small '#' hash symbol on top-left
    let mut hash_pb = PathBuilder::new();
    hash_pb.move_to(74.0, 54.0);
    hash_pb.line_to(71.0, 76.0);
    hash_pb.move_to(86.0, 54.0);
    hash_pb.line_to(83.0, 76.0);
    hash_pb.move_to(67.0, 61.0);
    hash_pb.line_to(91.0, 61.0);
    hash_pb.move_to(65.0, 69.0);
    hash_pb.line_to(89.0, 69.0);
    if let Some(hash_path) = hash_pb.finish() {
        let mut p = Paint::default();
        p.anti_alias = true;
        p.set_color_rgba8(56, 189, 248, 255);
        let stroke = Stroke {
            width: 4.2,
            line_cap: tiny_skia::LineCap::Round,
            ..Default::default()
        };
        pixmap.stroke_path(&hash_path, &p, &stroke, ts, None);
    }

    // =========================================================================
    // Main Geometric "M" - "R" - "P" Bold Vector Letters (y = 96 .. 168)
    // =========================================================================
    let y_top = 98.0;
    let y_mid = 132.0;
    let y_bot = 168.0;
    let stroke_m = Stroke {
        width: 14.5,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    // 1) Letter "M" (Electric Sky Blue #38BDF8)
    let mut m_pb = PathBuilder::new();
    m_pb.move_to(56.0, y_bot);
    m_pb.line_to(56.0, y_top);
    m_pb.line_to(77.0, y_mid + 6.0);
    m_pb.line_to(98.0, y_top);
    m_pb.line_to(98.0, y_bot);
    if let Some(m_path) = m_pb.finish() {
        let mut m_paint = Paint::default();
        m_paint.anti_alias = true;
        m_paint.set_color_rgba8(56, 189, 248, 255);
        pixmap.stroke_path(&m_path, &m_paint, &stroke_m, ts, None);
    }

    // 2) Letter "R" (Vibrant Rust Orange #FB923C)
    let mut r_pb = PathBuilder::new();
    r_pb.move_to(117.0, y_bot);
    r_pb.line_to(117.0, y_top);
    r_pb.line_to(138.0, y_top);
    r_pb.quad_to(151.0, y_top, 151.0, (y_top + y_mid) * 0.5);
    r_pb.quad_to(151.0, y_mid, 138.0, y_mid);
    r_pb.line_to(117.0, y_mid);
    r_pb.move_to(133.0, y_mid);
    r_pb.line_to(151.0, y_bot);
    if let Some(r_path) = r_pb.finish() {
        let mut r_paint = Paint::default();
        r_paint.anti_alias = true;
        r_paint.set_color_rgba8(251, 146, 60, 255);
        pixmap.stroke_path(&r_path, &r_paint, &stroke_m, ts, None);
    }

    // 3) Letter "P" (Crisp White #F8FAFC)
    let mut p_pb = PathBuilder::new();
    p_pb.move_to(168.0, y_bot);
    p_pb.line_to(168.0, y_top);
    p_pb.line_to(187.0, y_top);
    p_pb.quad_to(200.0, y_top, 200.0, (y_top + y_mid) * 0.5);
    p_pb.quad_to(200.0, y_mid, 187.0, y_mid);
    p_pb.line_to(168.0, y_mid);
    if let Some(p_path) = p_pb.finish() {
        let mut p_paint = Paint::default();
        p_paint.anti_alias = true;
        p_paint.set_color_rgba8(248, 250, 252, 255);
        pixmap.stroke_path(&p_path, &p_paint, &stroke_m, ts, None);
    }

    // Bottom Accent Bar (Crimson-to-Orange-to-Blue gradient bar under MRP)
    if let Some(bar_path) = rounded_rect_path(54.0, 188.0, 148.0, 16.0, 8.0) {
        let mut bar_paint = Paint::default();
        bar_paint.anti_alias = true;
        bar_paint.shader = LinearGradient::new(
            Point::from_xy(54.0, 188.0),
            Point::from_xy(202.0, 188.0),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(56, 189, 248, 255)),
                GradientStop::new(0.5, Color::from_rgba8(249, 115, 22, 255)),
                GradientStop::new(1.0, Color::from_rgba8(244, 63, 94, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap();
        pixmap.fill_path(&bar_path, &bar_paint, FillRule::Winding, ts, None);
    }
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w * 0.5).min(h * 0.5);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Encodes multi-size RGBA images (e.g. 256, 64, 48, 32, 16) into a standard Windows `.ico` binary.
pub fn encode_ico_bytes() -> Vec<u8> {
    let sizes = [256u32, 64, 48, 32, 16];
    let mut png_blobs: Vec<(u32, Vec<u8>)> = Vec::with_capacity(sizes.len());

    for &sz in &sizes {
        let mut pixmap = Pixmap::new(sz, sz).unwrap();
        let rgba = render_mrp_icon_rgba(sz);
        pixmap.data_mut().copy_from_slice(&rgba);
        let png_bytes = pixmap.encode_png().expect("Failed to encode icon PNG");
        png_blobs.push((sz, png_bytes));
    }

    let count = png_blobs.len() as u16;
    let mut ico = Vec::new();
    // ICONDIR header (6 bytes)
    ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // Type = 1 (Icon)
    ico.extend_from_slice(&count.to_le_bytes()); // Count

    let mut data_offset = 6u32 + (count as u32) * 16;
    for (sz, blob) in &png_blobs {
        let dim_byte = if *sz >= 256 { 0u8 } else { *sz as u8 };
        ico.push(dim_byte); // Width
        ico.push(dim_byte); // Height
        ico.push(0); // ColorCount
        ico.push(0); // Reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // Planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // BitCount
        let bytes_in_res = blob.len() as u32;
        ico.extend_from_slice(&bytes_in_res.to_le_bytes());
        ico.extend_from_slice(&data_offset.to_le_bytes());
        data_offset += bytes_in_res;
    }

    for (_, blob) in &png_blobs {
        ico.extend_from_slice(blob);
    }

    ico
}
