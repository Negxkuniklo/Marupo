use crate::config::{EmphasisStyle, HeadingStyle, MetadataDisplayMode, PaperSize, ThemeConfig};
use crate::markdown::{
    ColumnAlign, DocBlock, FootnoteDef, FrontmatterData, InlineFlags, InlineSpan, MathNode,
    ParsedDocument,
};
use std::fmt::Write;

/// Exports a standalone, self-contained HTML5 document styled to match the preview & ThemeConfig.
pub fn export_html_string(doc: &ParsedDocument, theme: &ThemeConfig) -> String {
    let mut html = String::with_capacity(16384);
    let doc_title = doc
        .frontmatter
        .get("title")
        .unwrap_or("Markdown Document");

    let (page_w_mm, page_h_mm) = match theme.paper_size {
        PaperSize::A4 => (210.0, 297.0),
        PaperSize::B5 => (182.0, 257.0),
        PaperSize::Letter => (215.9, 279.4),
    };
    let (w_mm, h_mm) = if theme.landscape {
        (page_h_mm, page_w_mm)
    } else {
        (page_w_mm, page_h_mm)
    };
    let css_page_size = match theme.paper_size {
        PaperSize::A4 => "A4",
        PaperSize::B5 => "B5",
        PaperSize::Letter => "Letter",
    };
    let css_orient = if theme.landscape { " landscape" } else { "" };

    let _ = write!(
        html,
        r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <style>
{css}
  </style>
</head>
<body>
  <main class="page-sheet">
"#,
        title = escape_html(doc_title),
        css = generate_preview_matching_css(theme, w_mm, h_mm, css_page_size, css_orient),
    );

    // 1. Frontmatter (Metadata)
    if !doc.frontmatter.is_empty() && theme.metadata_mode != MetadataDisplayMode::Hidden {
        render_frontmatter_html(&mut html, &doc.frontmatter, theme.metadata_mode);
    }

    // 2. Auto TOC if enabled and no explicit [TOC] block exists
    let has_explicit_toc = doc
        .blocks
        .iter()
        .any(|b| matches!(b, DocBlock::TocPlaceholder));
    if theme.show_toc && !has_explicit_toc {
        render_toc_html(&mut html, doc, theme);
    }

    // 3. Document Blocks
    let mut heading_idx = 0usize;
    for block in &doc.blocks {
        render_block_html(&mut html, block, doc, theme, &mut heading_idx);
    }

    // 4. Footnotes
    if !doc.footnotes.is_empty() {
        render_footnotes_html(&mut html, &doc.footnotes, theme.strict_line_breaks);
    }

    let _ = write!(
        html,
        r#"  </main>
</body>
</html>
"#
    );

    html
}

fn generate_preview_matching_css(
    theme: &ThemeConfig,
    w_mm: f32,
    _h_mm: f32,
    css_page_size: &str,
    css_orient: &str,
) -> String {
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);

    let emp_css = |s: &EmphasisStyle| -> String {
        let mut rules = format!(
            "color: {}; font-size: {:.3}em;",
            hex(s.color),
            s.size_scale
        );
        if s.use_background {
            let _ = write!(
                rules,
                " background-color: {}; padding: 0.08em 0.22em; border-radius: 3px;",
                hex(s.background_color)
            );
        }
        if s.underline {
            rules.push_str(" text-decoration: underline;");
        }
        if s.stroke_boost > 0.25 {
            rules.push_str(" font-weight: 700;");
        }
        if s.slant > 0.05 {
            rules.push_str(" font-style: italic;");
        }
        rules
    };

    let heading_css = |h: &HeadingStyle, is_h1: bool| -> String {
        let mut rules = format!(
            "color: {}; font-size: {:.3}em; font-weight: 700;",
            hex(h.color),
            h.size_scale
        );
        if h.underline {
            let bw = if is_h1 { "2px" } else { "1px" };
            let _ = write!(
                rules,
                " border-bottom: {} solid {}; padding-bottom: 0.2em;",
                bw,
                hex(h.underline_color)
            );
        }
        if h.left_accent_bar {
            let _ = write!(
                rules,
                " border-left: 4px solid {}; padding-left: 0.45em;",
                hex(h.color)
            );
        }
        if h.page_break_before {
            rules.push_str(" page-break-before: always; break-before: page;");
        }
        rules
    };

    format!(
        r#"    @page {{
      size: {css_page_size}{css_orient};
      margin: {mt:.1}mm {mr:.1}mm {mb:.1}mm {ml:.1}mm;
    }}
    * {{
      box-sizing: border-box;
    }}
    body {{
      margin: 0;
      padding: 0;
      background-color: {page_bg};
      font-family: "{body_font}", "Yu Gothic", "Meiryo", "Noto Sans CJK JP", "Noto Sans JP", "Hiragino Sans", sans-serif;
      font-size: {base_size:.2}pt;
      line-height: {line_height:.2};
      color: {text_color};
      -webkit-font-smoothing: antialiased;
    }}
    .page-sheet {{
      max-width: {w_mm:.1}mm;
      margin: 0 auto;
      padding: {mt:.1}mm {mr:.1}mm {mb:.1}mm {ml:.1}mm;
      background-color: {page_bg};
      color: {text_color};
    }}
    @media print {{
      .page-sheet {{
        max-width: none;
        width: 100%;
        margin: 0;
        padding: 0;
      }}
    }}
    h1, h2, h3, h4, h5, h6 {{
      font-family: "{heading_font}", "{body_font}", sans-serif;
      line-height: 1.3;
      margin-top: {h_top:.1}pt;
      margin-bottom: {h_bot:.1}pt;
    }}
    h1 {{ {h1_css} }}
    h2 {{ {h2_css} }}
    h1 + h2, h2 + h3, h3 + h4 {{ page-break-before: avoid; break-before: avoid; }}
    h3 {{ {h3_css} }}
    h4 {{ {h4_css} }}
    p {{
      margin-top: 0;
      margin-bottom: {p_space:.1}pt;
      text-align: justify;
    }}
    strong, b {{ {bold_css} }}
    em, i {{ {italic_css} }}
    mark {{ {highlight_css} }}
    del, s {{ text-decoration: line-through; }}
    code.inline-code {{
      font-family: "{code_font}", Consolas, "Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", monospace;
      {code_css}
    }}
    a {{
      color: {link_color};
      text-decoration: {link_ul};
    }}
    hr {{
      border: none;
      border-top: 1px solid {table_border};
      margin: 14pt 0;
    }}
    /* Frontmatter Header Card */
    .fm-card {{
      background-color: {code_bg};
      border: 1px solid {code_border};
      border-top: 4px solid {h1_ul};
      padding: 12pt;
      margin-bottom: {fm_bot:.1}pt;
    }}
    .fm-card-grid {{
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      grid-auto-flow: row dense;
      gap: 4pt 14pt;
      font-size: 0.92em;
      color: {quote_text};
    }}
    .fm-card-full {{
      grid-column: 1 / -1;
    }}
    /* Frontmatter Property Table */
    .fm-prop-heading {{
      font-family: "{heading_font}", sans-serif;
      font-size: 0.82em;
      font-weight: 700;
      color: {quote_text};
      margin-bottom: 4pt;
      letter-spacing: 0.04em;
    }}
    table.fm-prop-table {{
      width: 100%;
      border-collapse: collapse;
      margin-bottom: {fm_bot:.1}pt;
      font-size: 0.92em;
    }}
    table.fm-prop-table th {{
      width: 28%;
      text-align: left;
      background-color: {th_bg};
      color: {th_text};
      border: 1px solid {table_border};
      padding: 5pt 8pt;
      font-family: "{heading_font}", sans-serif;
    }}
    table.fm-prop-table td {{
      border: 1px solid {table_border};
      padding: 5pt 8pt;
    }}
    table.fm-prop-table tr:nth-child(even) td {{
      background-color: {table_stripe};
    }}
    /* Table of Contents */
    .toc-box {{
      margin-bottom: {fm_bot:.1}pt;
    }}
    .toc-title {{
      font-family: "{heading_font}", sans-serif;
      font-size: 1.25em;
      font-weight: 700;
      color: {h2_color};
      border-bottom: 1px solid {table_border};
      padding-bottom: 4pt;
      margin-bottom: 8pt;
    }}
    .toc-list {{
      list-style: none;
      margin: 0;
      padding: 0;
    }}
    .toc-item {{
      display: flex;
      align-items: baseline;
      margin: 3pt 0;
    }}
    .toc-item a {{
      color: {text_color};
      text-decoration: none;
    }}
    .toc-item a:hover {{
      color: {link_color};
      text-decoration: underline;
    }}
    .toc-dots {{
      flex: 1;
      border-bottom: 1px dotted {table_border};
      margin: 0 8pt;
    }}
    /* Code Blocks */
    .code-block-wrap {{
      background-color: {code_bg};
      border: 1px solid {code_border};
      margin-bottom: {p_space:.1}pt;
      overflow-x: auto;
    }}
    .code-block-lang {{
      background-color: {th_bg};
      border-bottom: 1px solid {code_border};
      padding: 2.5pt 10pt;
      font-family: "{code_font}", Consolas, "Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", monospace;
      font-size: 0.78em;
      font-weight: 600;
      color: {quote_text};
    }}
    pre.code-block {{
      margin: 0;
      padding: 8pt 10pt;
      font-family: "{code_font}", Consolas, "Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", monospace;
      font-size: 0.90em;
      line-height: 1.45;
      color: {code_text};
      white-space: pre-wrap;
      word-break: break-word;
    }}
    /* Blockquote & Callouts */
    blockquote.md-quote {{
      margin: 0 0 {p_space:.1}pt 0;
      padding: 6pt 10pt 6pt 12pt;
      background-color: {quote_bg};
      border-left: 3.5pt solid {quote_bar};
      color: {quote_text};
    }}
    blockquote.md-quote > *:last-child {{
      margin-bottom: 0;
    }}
    .callout {{
      margin: 0 0 {p_space:.1}pt 0;
      padding: 9pt 12pt 9pt 14pt;
      border: 1px solid;
      border-left-width: 4.5pt;
    }}
    .callout > *:last-child {{
      margin-bottom: 0;
    }}
    .callout-header {{
      display: flex;
      align-items: center;
      gap: 6pt;
      font-family: "{heading_font}", sans-serif;
      font-weight: 700;
      font-size: 1.02em;
      margin-bottom: 5pt;
    }}
    .callout-badge {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 14pt;
      height: 14pt;
      color: #ffffff;
      font-size: 0.8em;
      font-weight: 800;
      border-radius: 2px;
    }}
    /* Lists & Tasklists */
    ul, ol {{
      margin: 0 0 {p_space:.1}pt 0;
      padding-left: 20pt;
    }}
    li {{
      margin-bottom: 3pt;
    }}
    li.task-item {{
      list-style: none;
      margin-left: -18pt;
      display: flex;
      align-items: baseline;
      gap: 6pt;
    }}
    .task-checkbox {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 10pt;
      height: 10pt;
      border: 1px solid {quote_bar};
      border-radius: 2px;
      font-size: 8pt;
      line-height: 1;
      flex-shrink: 0;
      transform: translateY(1pt);
    }}
    .task-checkbox.checked {{
      background-color: {link_color};
      border-color: {link_color};
      color: #ffffff;
    }}
    /* Tables */
    table.md-table {{
      width: 100%;
      border-collapse: collapse;
      margin-bottom: {p_space:.1}pt;
      font-size: 0.94em;
    }}
    table.md-table th {{
      background-color: {th_bg};
      color: {th_text};
      font-family: "{heading_font}", sans-serif;
      font-weight: 700;
      border: 1px solid {table_border};
      padding: 5.5pt 7pt;
    }}
    table.md-table td {{
      border: 1px solid {table_border};
      padding: 5.5pt 7pt;
    }}
    table.md-table tbody tr:nth-child(even) td {{
      background-color: {table_stripe};
    }}
    /* 2D Math Layout */
    .math-inline {{
      font-family: "Cambria Math", "Latin Modern Math", "STIX Two Math", "DejaVu Math TeX Gyre", "{body_font}", serif;
      display: inline-flex;
      align-items: center;
      vertical-align: middle;
      margin: 0 1px;
    }}
    .math-display {{
      font-family: "Cambria Math", "Latin Modern Math", "STIX Two Math", "DejaVu Math TeX Gyre", "{body_font}", serif;
      font-size: 1.18em;
      display: flex;
      justify-content: center;
      align-items: center;
      margin: 10pt 0;
    }}
    .math-var {{
      font-style: italic;
    }}
    .math-op {{
      padding: 0 0.16em;
    }}
    .math-frac {{
      display: inline-flex;
      flex-direction: column;
      align-items: center;
      vertical-align: middle;
      margin: 0 0.15em;
      font-size: 0.85em;
    }}
    .math-frac-num {{
      border-bottom: 1px solid currentColor;
      padding: 0 0.25em 0.08em 0.25em;
      line-height: 1.1;
    }}
    .math-frac-den {{
      padding: 0.08em 0.25em 0 0.25em;
      line-height: 1.1;
    }}
    .math-sqrt {{
      display: inline-flex;
      align-items: stretch;
      vertical-align: middle;
    }}
    .math-sqrt-stem {{
      font-size: 1.15em;
      line-height: 1;
      padding-right: 1px;
    }}
    .math-sqrt-body {{
      border-top: 1px solid currentColor;
      padding: 0.05em 0.15em 0 0.1em;
    }}
    .math-supsub {{
      display: inline-flex;
      align-items: baseline;
    }}
    .math-scripts {{
      display: inline-flex;
      flex-direction: column;
      font-size: 0.70em;
      line-height: 1;
      margin-left: 1px;
      vertical-align: middle;
    }}
    /* Footnotes */
    .footnotes-section {{
      margin-top: 18pt;
      padding-top: 8pt;
      border-top: 1px solid {table_border};
      font-size: 0.86em;
      color: {quote_text};
    }}
    .footnote-item {{
      margin-bottom: 4pt;
    }}
    sup.fn-ref a {{
      color: {link_color};
      font-weight: 700;
      text-decoration: none;
    }}"#,
        css_page_size = css_page_size,
        css_orient = css_orient,
        w_mm = w_mm,
        mt = theme.margin_top_mm,
        mr = theme.margin_right_mm,
        mb = theme.margin_bottom_mm,
        ml = theme.margin_left_mm,
        body_font = theme.body_font,
        heading_font = theme.heading_font,
        code_font = theme.code_font,
        base_size = theme.base_font_size,
        line_height = theme.line_height,
        p_space = theme.paragraph_spacing,
        fm_bot = theme.paragraph_spacing * 1.5,
        h_top = theme.paragraph_spacing * 1.3,
        h_bot = theme.paragraph_spacing * 0.65,
        text_color = hex(theme.text_color),
        page_bg = hex(theme.page_background),
        link_color = hex(theme.link_color),
        link_ul = if theme.link_underline {
            "underline"
        } else {
            "none"
        },
        h1_css = heading_css(&theme.h1_style, true),
        h2_css = heading_css(&theme.h2_style, false),
        h3_css = heading_css(&theme.h3_style, false),
        h4_css = heading_css(&theme.h4_style, false),
        h1_ul = hex(theme.h1_style.underline_color),
        h2_color = hex(theme.h2_style.color),
        bold_css = emp_css(&theme.bold_style),
        italic_css = emp_css(&theme.italic_style),
        highlight_css = emp_css(&theme.highlight_style),
        code_css = emp_css(&theme.inline_code_style),
        code_bg = hex(theme.code_block_bg),
        code_text = hex(theme.code_block_text),
        code_border = hex(theme.code_block_border),
        quote_bar = hex(theme.quote_bar_color),
        quote_bg = hex(theme.quote_bg_color),
        quote_text = hex(theme.quote_text_color),
        th_bg = hex(theme.table_header_bg),
        th_text = hex(theme.table_header_text),
        table_stripe = hex(theme.table_stripe_bg),
        table_border = hex(theme.table_border_color),
    )
}

fn render_frontmatter_html(
    out: &mut String,
    fm: &FrontmatterData,
    mode: MetadataDisplayMode,
) {
    match mode {
        MetadataDisplayMode::Hidden => {}
        MetadataDisplayMode::HeaderCard => {
            if fm.properties.is_empty() {
                return;
            }
            let _ = writeln!(out, r#"    <header class="fm-card">"#);
            let _ = writeln!(out, r#"      <div class="fm-card-grid">"#);
            for (k, v) in &fm.properties {
                if FrontmatterData::is_full_width_property(k, v) {
                    let _ = writeln!(
                        out,
                        r#"        <div class="fm-card-full"><strong>{}:</strong> {}</div>"#,
                        escape_html(k),
                        escape_html(v)
                    );
                } else {
                    let _ = writeln!(
                        out,
                        r#"        <div><strong>{}:</strong> {}</div>"#,
                        escape_html(k),
                        escape_html(v)
                    );
                }
            }
            let _ = writeln!(out, r#"      </div>"#);
            let _ = writeln!(out, r#"    </header>"#);
        }
        MetadataDisplayMode::PropertyTable => {
            let _ = writeln!(
                out,
                r#"    <div class="fm-prop-heading">DOCUMENT PROPERTIES / プロパティ</div>"#
            );
            let _ = writeln!(out, r#"    <table class="fm-prop-table"><tbody>"#);
            for (k, v) in &fm.properties {
                let _ = writeln!(
                    out,
                    r#"      <tr><th>{}</th><td>{}</td></tr>"#,
                    escape_html(k),
                    escape_html(v)
                );
            }
            let _ = writeln!(out, r#"    </tbody></table>"#);
        }
    }
}

fn render_toc_html(out: &mut String, doc: &ParsedDocument, theme: &ThemeConfig) {
    let mut entries: Vec<(usize, u8, String)> = Vec::new();
    let mut idx = 0usize;
    for b in &doc.blocks {
        if let DocBlock::Heading {
            level, plain_text, ..
        } = b
        {
            idx += 1;
            if *level <= theme.max_toc_level {
                entries.push((idx, *level, plain_text.clone()));
            }
        }
    }
    if entries.is_empty() {
        return;
    }

    let _ = writeln!(out, r#"    <nav class="toc-box">"#);
    let _ = writeln!(
        out,
        r#"      <div class="toc-title">{}</div>"#,
        escape_html(&theme.toc_title)
    );
    let _ = writeln!(out, r#"      <ul class="toc-list">"#);
    for (h_id, level, title) in entries {
        let indent_pt = (level.saturating_sub(1) as usize) * 14;
        let weight = if level == 1 { "font-weight: 700;" } else { "" };
        let _ = writeln!(
            out,
            r##"        <li class="toc-item" style="padding-left: {}pt; {}"><a href="#heading-{}">{}</a><span class="toc-dots"></span></li>"##,
            indent_pt,
            weight,
            h_id,
            escape_html(&title)
        );
    }
    let _ = writeln!(out, r#"      </ul>"#);
    let _ = writeln!(out, r#"    </nav>"#);
}

fn render_block_html(
    out: &mut String,
    block: &DocBlock,
    doc: &ParsedDocument,
    theme: &ThemeConfig,
    heading_idx: &mut usize,
) {
    match block {
        DocBlock::TocPlaceholder => {
            render_toc_html(out, doc, theme);
        }
        DocBlock::Heading { level, spans, .. } => {
            *heading_idx += 1;
            let tag = match level {
                1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            let _ = write!(out, r#"    <{} id="heading-{}">"#, tag, *heading_idx);
            render_spans_html(out, spans, theme.strict_line_breaks);
            let _ = writeln!(out, "</{}>", tag);
        }
        DocBlock::Paragraph(spans) => {
            let _ = write!(out, "    <p>");
            render_spans_html(out, spans, theme.strict_line_breaks);
            let _ = writeln!(out, "</p>");
        }
        DocBlock::CodeBlock { lang, code } => {
            let _ = writeln!(out, r#"    <div class="code-block-wrap">"#);
            if let Some(l) = lang {
                let _ = writeln!(
                    out,
                    r#"      <div class="code-block-lang">{}</div>"#,
                    escape_html(l)
                );
            }
            let _ = writeln!(
                out,
                r#"      <pre class="code-block"><code>{}</code></pre>"#,
                escape_html(code)
            );
            let _ = writeln!(out, r#"    </div>"#);
        }
        DocBlock::BlockQuote(sub_blocks) => {
            let _ = writeln!(out, r#"    <blockquote class="md-quote">"#);
            for sb in sub_blocks {
                render_block_html(out, sb, doc, theme, heading_idx);
            }
            let _ = writeln!(out, r#"    </blockquote>"#);
        }
        DocBlock::Callout {
            kind,
            title,
            blocks,
        } => {
            let (accent, bg, title_col) = kind.colors();
            let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
            let _ = writeln!(
                out,
                r#"    <div class="callout" style="background-color: {}; border-color: {};">"#,
                hex(bg),
                hex(accent)
            );
            let _ = writeln!(
                out,
                r#"      <div class="callout-header" style="color: {};"><span class="callout-badge" style="background-color: {};">!</span><span>{}</span></div>"#,
                hex(title_col),
                hex(accent),
                escape_html(title)
            );
            for b in blocks {
                render_block_html(out, b, doc, theme, heading_idx);
            }
            let _ = writeln!(out, r#"    </div>"#);
        }
        DocBlock::List { ordered, items } => {
            if let Some(start) = ordered {
                let _ = writeln!(out, r#"    <ol start="{}">"#, start);
            } else {
                let _ = writeln!(out, r#"    <ul>"#);
            }
            for item in items {
                if let Some(checked) = item.checked {
                    let cls = if checked {
                        "task-checkbox checked"
                    } else {
                        "task-checkbox"
                    };
                    let mark = if checked { "✓" } else { "" };
                    let _ = write!(
                        out,
                        r#"      <li class="task-item"><span class="{}">{}</span><div>"#,
                        cls, mark
                    );
                    render_spans_html(out, &item.spans, theme.strict_line_breaks);
                    for sb in &item.sub_blocks {
                        render_block_html(out, sb, doc, theme, heading_idx);
                    }
                    let _ = writeln!(out, "</div></li>");
                } else {
                    let _ = write!(out, "      <li>");
                    render_spans_html(out, &item.spans, theme.strict_line_breaks);
                    for sb in &item.sub_blocks {
                        render_block_html(out, sb, doc, theme, heading_idx);
                    }
                    let _ = writeln!(out, "</li>");
                }
            }
            if ordered.is_some() {
                let _ = writeln!(out, "    </ol>");
            } else {
                let _ = writeln!(out, "    </ul>");
            }
        }
        DocBlock::Table {
            alignments,
            headers,
            rows,
        } => {
            let align_str = |col: usize| -> &'static str {
                match alignments.get(col).copied().unwrap_or(ColumnAlign::Left) {
                    ColumnAlign::Left => "left",
                    ColumnAlign::Center => "center",
                    ColumnAlign::Right => "right",
                }
            };
            let _ = writeln!(out, r#"    <table class="md-table">"#);
            if !headers.is_empty() {
                let _ = writeln!(out, "      <thead><tr>");
                for (c_idx, cell) in headers.iter().enumerate() {
                    let _ = write!(out, r#"        <th style="text-align: {};">"#, align_str(c_idx));
                    render_spans_html(out, cell, theme.strict_line_breaks);
                    let _ = writeln!(out, "</th>");
                }
                let _ = writeln!(out, "      </tr></thead>");
            }
            let _ = writeln!(out, "      <tbody>");
            for row in rows {
                let _ = writeln!(out, "        <tr>");
                for (c_idx, cell) in row.iter().enumerate() {
                    let _ = write!(out, r#"          <td style="text-align: {};">"#, align_str(c_idx));
                    render_spans_html(out, cell, theme.strict_line_breaks);
                    let _ = writeln!(out, "</td>");
                }
                let _ = writeln!(out, "        </tr>");
            }
            let _ = writeln!(out, "      </tbody></table>");
        }
        DocBlock::DisplayMath(math_node) => {
            let _ = write!(out, r#"    <div class="math-display">"#);
            render_math_html(out, math_node);
            let _ = writeln!(out, "</div>");
        }
        DocBlock::HorizontalRule => {
            let _ = writeln!(out, "    <hr>");
        }
    }
}

fn render_spans_html(out: &mut String, spans: &[InlineSpan], strict_line_breaks: bool) {
    for span in spans {
        match span {
            InlineSpan::Text { text, flags } => {
                write_flagged_html(out, &escape_html(text), flags, false);
            }
            InlineSpan::InlineCode { code, flags } => {
                write_flagged_html(out, &escape_html(code), flags, true);
            }
            InlineSpan::InlineMath(math_node) => {
                let _ = write!(out, r#"<span class="math-inline">"#);
                render_math_html(out, math_node);
                let _ = write!(out, "</span>");
            }
            InlineSpan::FootnoteRef { number, .. } => {
                let _ = write!(
                    out,
                    r##"<sup class="fn-ref"><a href="#fn-{}">[{}]</a></sup>"##,
                    number, number
                );
            }
            InlineSpan::SoftBreak => {
                if strict_line_breaks {
                    out.push(' ');
                } else {
                    out.push_str("<br>");
                }
            }
            InlineSpan::HardBreak => {
                out.push_str("<br>");
            }
        }
    }
}

fn write_flagged_html(out: &mut String, escaped_content: &str, flags: &InlineFlags, is_code: bool) {
    if let Some(url) = &flags.link_url {
        let _ = write!(
            out,
            r#"<a href="{}" target="_blank" rel="noopener">"#,
            escape_html(url)
        );
    }
    if flags.highlight {
        out.push_str("<mark>");
    }
    if flags.bold {
        out.push_str("<strong>");
    }
    if flags.italic {
        out.push_str("<em>");
    }
    if flags.strikethrough {
        out.push_str("<del>");
    }
    if is_code {
        out.push_str(r#"<code class="inline-code">"#);
    }

    out.push_str(escaped_content);

    if is_code {
        out.push_str("</code>");
    }
    if flags.strikethrough {
        out.push_str("</del>");
    }
    if flags.italic {
        out.push_str("</em>");
    }
    if flags.bold {
        out.push_str("</strong>");
    }
    if flags.highlight {
        out.push_str("</mark>");
    }
    if flags.link_url.is_some() {
        out.push_str("</a>");
    }
}

fn render_math_html(out: &mut String, node: &MathNode) {
    match node {
        MathNode::Row(children) => {
            for ch in children {
                render_math_html(out, ch);
            }
        }
        MathNode::Text(s) => {
            for c in s.chars() {
                if c.is_ascii_alphabetic() {
                    let _ = write!(out, r#"<span class="math-var">{}</span>"#, c);
                } else {
                    let _ = write!(out, "<span>{}</span>", escape_html(&c.to_string()));
                }
            }
        }
        MathNode::Operator(op) => {
            let _ = write!(out, r#"<span class="math-op">{}</span>"#, escape_html(op.trim()));
        }
        MathNode::Frac(num, den) => {
            out.push_str(r#"<span class="math-frac"><span class="math-frac-num">"#);
            render_math_html(out, num);
            out.push_str(r#"</span><span class="math-frac-den">"#);
            render_math_html(out, den);
            out.push_str("</span></span>");
        }
        MathNode::Sqrt(inner) => {
            out.push_str(r#"<span class="math-sqrt"><span class="math-sqrt-stem">√</span><span class="math-sqrt-body">"#);
            render_math_html(out, inner);
            out.push_str("</span></span>");
        }
        MathNode::SupSub { base, sup, sub } => {
            out.push_str(r#"<span class="math-supsub">"#);
            render_math_html(out, base);
            match (sup, sub) {
                (Some(sup_n), Some(sub_n)) => {
                    out.push_str(r#"<span class="math-scripts"><span>"#);
                    render_math_html(out, sup_n);
                    out.push_str("</span><span>");
                    render_math_html(out, sub_n);
                    out.push_str("</span></span>");
                }
                (Some(sup_n), None) => {
                    out.push_str("<sup>");
                    render_math_html(out, sup_n);
                    out.push_str("</sup>");
                }
                (None, Some(sub_n)) => {
                    out.push_str("<sub>");
                    render_math_html(out, sub_n);
                    out.push_str("</sub>");
                }
                (None, None) => {}
            }
            out.push_str("</span>");
        }
    }
}

fn render_footnotes_html(out: &mut String, footnotes: &[FootnoteDef], strict_line_breaks: bool) {
    let _ = writeln!(out, r#"    <section class="footnotes-section">"#);
    for fdef in footnotes {
        let _ = write!(
            out,
            r#"      <div class="footnote-item" id="fn-{}"><strong>[{}]</strong> "#,
            fdef.number, fdef.number
        );
        render_spans_html(out, &fdef.spans, strict_line_breaks);
        let _ = writeln!(out, "</div>");
    }
    let _ = writeln!(out, "    </section>");
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}
