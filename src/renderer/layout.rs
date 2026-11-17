use crate::config::{EmphasisStyle, HeadingStyle, MetadataDisplayMode, ThemeConfig};
use crate::markdown::{
    ColumnAlign, DocBlock, FootnoteDef, FrontmatterData, InlineFlags, InlineSpan, MathNode,
    ParsedDocument,
};
use crate::renderer::fonts::{FontSystem, LoadedFontId};

#[derive(Debug, Clone)]
pub enum LayoutPrimitive {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<[u8; 3]>,
        stroke: Option<([u8; 3], f32)>,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [u8; 3],
        width: f32,
    },
    Glyph {
        font_id: LoadedFontId,
        glyph_id: u16,
        x: f32,
        baseline_y: f32,
        size_pt: f32,
        color: [u8; 3],
        stroke_boost: f32,
        slant: f32,
    },
}

#[derive(Debug, Clone)]
pub struct LinkAnnotation {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct HeadingEntry {
    pub level: u8,
    pub title: String,
    pub page_index: usize,
    pub y_pt: f32,
}

#[derive(Debug, Clone)]
pub struct PageLayout {
    pub width_pt: f32,
    pub height_pt: f32,
    pub background: [u8; 3],
    pub primitives: Vec<LayoutPrimitive>,
    pub links: Vec<LinkAnnotation>,
}

#[derive(Debug, Clone)]
pub struct DocumentLayout {
    pub pages: Vec<PageLayout>,
    pub headings: Vec<HeadingEntry>,
}

struct ResolvedFonts {
    body: LoadedFontId,
    heading: LoadedFontId,
    code: LoadedFontId,
}

pub fn layout_document(
    doc: &ParsedDocument,
    theme: &ThemeConfig,
    fonts: &mut FontSystem,
) -> DocumentLayout {
    let resolved_fonts = ResolvedFonts {
        body: fonts.resolve_family_or_default(&theme.body_font),
        heading: fonts.resolve_family_or_default(&theme.heading_font),
        code: fonts.resolve_family_or_default(&theme.code_font),
    };

    let has_toc = theme.show_toc
        || doc
            .blocks
            .iter()
            .any(|b| matches!(b, DocBlock::TocPlaceholder));

    // Pass 1: initial layout
    let first_pass = run_layout_pass(doc, theme, fonts, &resolved_fonts, &[]);
    if !has_toc {
        return first_pass;
    }

    // Pass 2: use accurate heading page numbers discovered in Pass 1
    run_layout_pass(doc, theme, fonts, &resolved_fonts, &first_pass.headings)
}

fn run_layout_pass(
    doc: &ParsedDocument,
    theme: &ThemeConfig,
    fonts: &FontSystem,
    rf: &ResolvedFonts,
    known_headings: &[HeadingEntry],
) -> DocumentLayout {
    let (page_w, page_h) = theme.paper_size.dimensions_pt(theme.landscape);
    let mm_to_pt = 72.0 / 25.4;
    let margin_top = theme.margin_top_mm * mm_to_pt;
    let margin_bottom = theme.margin_bottom_mm * mm_to_pt;
    let margin_left = theme.margin_left_mm * mm_to_pt;
    let margin_right = theme.margin_right_mm * mm_to_pt;

    let content_w = (page_w - margin_left - margin_right).max(100.0);
    let bottom_limit = page_h - margin_bottom - if theme.show_page_numbers { 16.0 } else { 0.0 };

    let mut ctx = LayoutContext {
        theme,
        fonts,
        rf,
        page_w,
        page_h,
        margin_top,
        margin_bottom,
        margin_left,
        content_w,
        bottom_limit,
        pages: vec![PageLayout {
            width_pt: page_w,
            height_pt: page_h,
            background: theme.page_background,
            primitives: Vec::new(),
            links: Vec::new(),
        }],
        cursor_y: margin_top,
        headings: Vec::new(),
        known_headings,
        heading_counter: 0,
        prev_top_heading_level: None,
        has_body_content: false,
    };

    // 1. Render Frontmatter if present & enabled
    if !doc.frontmatter.is_empty() && theme.metadata_mode != MetadataDisplayMode::Hidden {
        ctx.layout_frontmatter(&doc.frontmatter);
    }

    // 2. Render auto TOC at top if theme.show_toc is enabled and no explicit [TOC] block exists
    let has_explicit_toc = doc
        .blocks
        .iter()
        .any(|b| matches!(b, DocBlock::TocPlaceholder));
    if theme.show_toc && !has_explicit_toc {
        ctx.layout_toc(doc);
    }

    // 3. Render document body blocks
    for block in &doc.blocks {
        ctx.layout_block(block, ctx.margin_left, ctx.content_w, 0);
        match block {
            DocBlock::Heading { level, .. } => {
                ctx.prev_top_heading_level = Some(*level);
                ctx.has_body_content = true;
            }
            DocBlock::TocPlaceholder => {}
            _ => {
                ctx.prev_top_heading_level = None;
                ctx.has_body_content = true;
            }
        }
    }

    // 4. Render footnotes at the end if any exist
    if !doc.footnotes.is_empty() {
        ctx.layout_footnotes(&doc.footnotes);
    }

    // 5. Render page numbers if enabled
    if theme.show_page_numbers {
        ctx.render_page_numbers();
    }

    DocumentLayout {
        pages: ctx.pages,
        headings: ctx.headings,
    }
}

struct LayoutContext<'a> {
    theme: &'a ThemeConfig,
    fonts: &'a FontSystem,
    rf: &'a ResolvedFonts,
    page_w: f32,
    page_h: f32,
    margin_top: f32,
    margin_bottom: f32,
    margin_left: f32,
    content_w: f32,
    bottom_limit: f32,
    pages: Vec<PageLayout>,
    cursor_y: f32,
    headings: Vec<HeadingEntry>,
    known_headings: &'a [HeadingEntry],
    heading_counter: usize,
    prev_top_heading_level: Option<u8>,
    has_body_content: bool,
}

#[derive(Clone)]
struct ResolvedCharStyle {
    font_id: LoadedFontId,
    size_pt: f32,
    color: [u8; 3],
    stroke_boost: f32,
    slant: f32,
    bg_color: Option<[u8; 3]>,
    underline: bool,
    strikethrough: bool,
    link_url: Option<String>,
}

#[derive(Clone)]
enum InlineAtom {
    Char {
        ch: char,
        glyph_id: u16,
        width_pt: f32,
        style: ResolvedCharStyle,
    },
    Math {
        mbox: MathBox,
    },
    ForcedBreak,
}

impl InlineAtom {
    fn width(&self) -> f32 {
        match self {
            Self::Char { width_pt, .. } => *width_pt,
            Self::Math { mbox } => mbox.width,
            Self::ForcedBreak => 0.0,
        }
    }

    fn ascent(&self) -> f32 {
        match self {
            Self::Char { style, .. } => style.size_pt * 0.82,
            Self::Math { mbox } => mbox.ascent,
            Self::ForcedBreak => 0.0,
        }
    }

    fn descent(&self) -> f32 {
        match self {
            Self::Char { style, .. } => style.size_pt * 0.22,
            Self::Math { mbox } => mbox.descent,
            Self::ForcedBreak => 0.0,
        }
    }

    fn is_ascii_word_char(&self) -> bool {
        match self {
            Self::Char { ch, .. } => {
                ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':' | '@')
            }
            _ => false,
        }
    }

    fn is_whitespace(&self) -> bool {
        matches!(self, Self::Char { ch: ' ' | '\t', .. })
    }

    fn is_kinsoku_line_start(&self) -> bool {
        match self {
            Self::Char { ch, .. } => matches!(
                ch,
                '、' | '。'
                    | '，'
                    | '．'
                    | '・'
                    | '：'
                    | '；'
                    | '？'
                    | '！'
                    | '）'
                    | '〕'
                    | '］'
                    | '｝'
                    | '〉'
                    | '》'
                    | '」'
                    | '』'
                    | '】'
                    | 'ー'
                    | 'ぁ'
                    | 'ぃ'
                    | 'ぅ'
                    | 'ぇ'
                    | 'ぉ'
                    | 'っ'
                    | 'ゃ'
                    | 'ゅ'
                    | 'ょ'
                    | 'ゎ'
                    | 'ァ'
                    | 'ィ'
                    | 'ゥ'
                    | 'ェ'
                    | 'ォ'
                    | 'ッ'
                    | 'ャ'
                    | 'ュ'
                    | 'ョ'
                    | 'ヮ'
                    | ')'
                    | ']'
                    | '}'
                    | ','
                    | '.'
                    | '!'
                    | '?'
            ),
            _ => false,
        }
    }
}

#[derive(Clone)]
struct WrappedLine {
    atoms: Vec<InlineAtom>,
    width: f32,
    ascent: f32,
    descent: f32,
}

#[derive(Clone)]
struct MathBox {
    width: f32,
    ascent: f32,
    descent: f32,
    primitives: Vec<LayoutPrimitive>,
}

impl<'a> LayoutContext<'a> {
    fn current_page(&mut self) -> &mut PageLayout {
        self.pages.last_mut().unwrap()
    }

    fn push_prim(&mut self, prim: LayoutPrimitive) {
        self.pages.last_mut().unwrap().primitives.push(prim);
    }

    fn push_link(&mut self, link: LinkAnnotation) {
        self.pages.last_mut().unwrap().links.push(link);
    }

    fn new_page(&mut self) {
        self.pages.push(PageLayout {
            width_pt: self.page_w,
            height_pt: self.page_h,
            background: self.theme.page_background,
            primitives: Vec::new(),
            links: Vec::new(),
        });
        self.cursor_y = self.margin_top;
    }

    fn ensure_space(&mut self, needed_h: f32) {
        if self.cursor_y + needed_h > self.bottom_limit && self.cursor_y > self.margin_top + 1.0 {
            self.new_page();
        }
    }

    fn layout_frontmatter(&mut self, fm: &FrontmatterData) {
        match self.theme.metadata_mode {
            MetadataDisplayMode::Hidden => {}
            MetadataDisplayMode::HeaderCard => {
                if fm.properties.is_empty() {
                    return;
                }

                let pad = 12.0;
                let col_w = (self.content_w - pad * 2.0) * 0.5;
                let meta_line_h = self.theme.base_font_size * 1.45;
                let base_style = ResolvedCharStyle {
                    font_id: self.rf.body,
                    size_pt: self.theme.base_font_size * 0.92,
                    color: self.theme.quote_text_color,
                    stroke_boost: 0.0,
                    slant: 0.0,
                    bg_color: None,
                    underline: false,
                    strikethrough: false,
                    link_url: None,
                };

                let wrapped_items: Vec<Vec<WrappedLine>> = fm
                    .properties
                    .iter()
                    .map(|(k, v)| {
                        let spans = vec![
                            InlineSpan::Text {
                                text: format!("{}: ", k),
                                flags: InlineFlags {
                                    bold: true,
                                    ..Default::default()
                                },
                            },
                            InlineSpan::Text {
                                text: v.clone(),
                                flags: InlineFlags::default(),
                            },
                        ];
                        self.wrap_spans(&spans, &base_style, col_w - 8.0)
                    })
                    .collect();

                let row_heights: Vec<f32> = wrapped_items
                    .chunks(2)
                    .map(|chunk| {
                        let max_lines = chunk.iter().map(|lines| lines.len().max(1)).max().unwrap_or(1);
                        max_lines as f32 * meta_line_h
                    })
                    .collect();

                let rows_h: f32 = row_heights.iter().sum();
                let card_h = pad * 2.0 + rows_h;

                self.ensure_space(card_h + 12.0);
                let card_y = self.cursor_y;

                // Card background & accent top border
                self.push_prim(LayoutPrimitive::Rect {
                    x: self.margin_left,
                    y: card_y,
                    w: self.content_w,
                    h: card_h,
                    fill: Some(self.theme.code_block_bg),
                    stroke: Some((self.theme.code_block_border, 0.8)),
                });
                self.push_prim(LayoutPrimitive::Rect {
                    x: self.margin_left,
                    y: card_y,
                    w: self.content_w,
                    h: 3.5,
                    fill: Some(self.theme.h1_style.underline_color),
                    stroke: None,
                });

                self.cursor_y += pad;
                for (chunk, &row_h) in wrapped_items.chunks(2).zip(row_heights.iter()) {
                    let row_y = self.cursor_y;
                    for (idx, lines) in chunk.iter().enumerate() {
                        let col_x = self.margin_left + pad + idx as f32 * col_w;
                        self.cursor_y = row_y;
                        for line in lines {
                            self.draw_wrapped_line(line, col_x, ColumnAlign::Left, 0.0);
                            self.cursor_y += meta_line_h;
                        }
                    }
                    self.cursor_y = row_y + row_h;
                }

                self.cursor_y = card_y + card_h + self.theme.paragraph_spacing * 1.5;
            }
            MetadataDisplayMode::PropertyTable => {
                // Render all properties as a clean 2-column property table at the top
                let key_col_w = (self.content_w * 0.28).clamp(80.0, 160.0);
                let val_col_w = self.content_w - key_col_w;
                let cell_pad_x = 8.0;
                let cell_pad_y = 5.0;

                // Small header label "PROPERTIES / メタデータ"
                let label_style = ResolvedCharStyle {
                    font_id: self.rf.heading,
                    size_pt: self.theme.base_font_size * 0.82,
                    color: self.theme.quote_text_color,
                    stroke_boost: 0.5,
                    slant: 0.0,
                    bg_color: None,
                    underline: false,
                    strikethrough: false,
                    link_url: None,
                };
                let label_spans = vec![InlineSpan::Text {
                    text: "DOCUMENT PROPERTIES / プロパティ".to_string(),
                    flags: InlineFlags::default(),
                }];
                let label_lines = self.wrap_spans(&label_spans, &label_style, self.content_w);
                if let Some(l) = label_lines.first() {
                    self.ensure_space(24.0);
                    self.draw_wrapped_line(l, self.margin_left, ColumnAlign::Left, 0.0);
                    self.cursor_y += (l.ascent + l.descent) + 4.0;
                }

                for (idx, (k, v)) in fm.properties.iter().enumerate() {
                    let key_style = ResolvedCharStyle {
                        font_id: self.rf.heading,
                        size_pt: self.theme.base_font_size * 0.92,
                        color: self.theme.table_header_text,
                        stroke_boost: 0.45,
                        slant: 0.0,
                        bg_color: None,
                        underline: false,
                        strikethrough: false,
                        link_url: None,
                    };
                    let val_style = ResolvedCharStyle {
                        font_id: self.rf.body,
                        size_pt: self.theme.base_font_size * 0.92,
                        color: self.theme.text_color,
                        stroke_boost: 0.0,
                        slant: 0.0,
                        bg_color: None,
                        underline: false,
                        strikethrough: false,
                        link_url: None,
                    };
                    let key_lines = self.wrap_spans(
                        &[InlineSpan::Text {
                            text: k.clone(),
                            flags: InlineFlags::default(),
                        }],
                        &key_style,
                        key_col_w - cell_pad_x * 2.0,
                    );
                    let val_lines = self.wrap_spans(
                        &[InlineSpan::Text {
                            text: v.clone(),
                            flags: InlineFlags::default(),
                        }],
                        &val_style,
                        val_col_w - cell_pad_x * 2.0,
                    );

                    let kh: f32 = key_lines
                        .iter()
                        .map(|l| (l.ascent + l.descent) * 1.25)
                        .sum();
                    let vh: f32 = val_lines
                        .iter()
                        .map(|l| (l.ascent + l.descent) * 1.25)
                        .sum();
                    let row_h = kh.max(vh).max(self.theme.base_font_size * 1.2) + cell_pad_y * 2.0;

                    self.ensure_space(row_h);
                    let row_y = self.cursor_y;

                    // Key cell background
                    self.push_prim(LayoutPrimitive::Rect {
                        x: self.margin_left,
                        y: row_y,
                        w: key_col_w,
                        h: row_h,
                        fill: Some(self.theme.table_header_bg),
                        stroke: Some((self.theme.table_border_color, 0.6)),
                    });
                    // Value cell background
                    let val_bg = if idx % 2 == 1 {
                        Some(self.theme.table_stripe_bg)
                    } else {
                        None
                    };
                    self.push_prim(LayoutPrimitive::Rect {
                        x: self.margin_left + key_col_w,
                        y: row_y,
                        w: val_col_w,
                        h: row_h,
                        fill: val_bg,
                        stroke: Some((self.theme.table_border_color, 0.6)),
                    });

                    self.cursor_y = row_y + cell_pad_y;
                    for l in &key_lines {
                        self.draw_wrapped_line(
                            l,
                            self.margin_left + cell_pad_x,
                            ColumnAlign::Left,
                            0.0,
                        );
                        self.cursor_y += (l.ascent + l.descent) * 1.25;
                    }

                    self.cursor_y = row_y + cell_pad_y;
                    for l in &val_lines {
                        self.draw_wrapped_line(
                            l,
                            self.margin_left + key_col_w + cell_pad_x,
                            ColumnAlign::Left,
                            0.0,
                        );
                        self.cursor_y += (l.ascent + l.descent) * 1.25;
                    }

                    self.cursor_y = row_y + row_h;
                }

                self.cursor_y += self.theme.paragraph_spacing * 1.5;
            }
        }
    }

    fn layout_toc(&mut self, doc: &ParsedDocument) {
        // Collect headings from doc up to theme.max_toc_level
        let mut toc_items: Vec<(u8, String, usize)> = Vec::new();
        let mut idx = 0usize;
        for b in &doc.blocks {
            if let DocBlock::Heading {
                level, plain_text, ..
            } = b
            {
                let page_num = self
                    .known_headings
                    .get(idx)
                    .map(|h| h.page_index + 1)
                    .unwrap_or(1);
                idx += 1;
                if *level <= self.theme.max_toc_level {
                    toc_items.push((*level, plain_text.clone(), page_num));
                }
            }
        }

        if toc_items.is_empty() {
            return;
        }

        let pad = 12.0;
        let line_h = self.theme.base_font_size * 1.55;
        let title_h = self.theme.base_font_size * 1.8;
        let box_h = pad * 2.0 + title_h + 6.0 + toc_items.len() as f32 * line_h;

        self.ensure_space(box_h.min(180.0));
        let start_y = self.cursor_y;

        // TOC Header
        let title_style = ResolvedCharStyle {
            font_id: self.rf.heading,
            size_pt: self.theme.base_font_size * 1.25,
            color: self.theme.h2_style.color,
            stroke_boost: 0.65,
            slant: 0.0,
            bg_color: None,
            underline: false,
            strikethrough: false,
            link_url: None,
        };
        let title_lines = self.wrap_spans(
            &[InlineSpan::Text {
                text: self.theme.toc_title.clone(),
                flags: InlineFlags::default(),
            }],
            &title_style,
            self.content_w,
        );
        if let Some(tl) = title_lines.first() {
            self.draw_wrapped_line(tl, self.margin_left, ColumnAlign::Left, 0.0);
            self.cursor_y += tl.ascent + tl.descent + 4.0;
        }
        self.push_prim(LayoutPrimitive::Line {
            x1: self.margin_left,
            y1: self.cursor_y,
            x2: self.margin_left + self.content_w,
            y2: self.cursor_y,
            color: self.theme.table_border_color,
            width: 1.0,
        });
        self.cursor_y += 8.0;

        for (level, text, page_num) in toc_items {
            self.ensure_space(line_h);
            let indent = (level.saturating_sub(1) as f32) * 14.0;
            let is_h1 = level == 1;
            let entry_style = ResolvedCharStyle {
                font_id: if is_h1 { self.rf.heading } else { self.rf.body },
                size_pt: self.theme.base_font_size * if is_h1 { 1.0 } else { 0.94 },
                color: self.theme.text_color,
                stroke_boost: if is_h1 { 0.45 } else { 0.0 },
                slant: 0.0,
                bg_color: None,
                underline: false,
                strikethrough: false,
                link_url: None,
            };

            let page_str = page_num.to_string();
            let page_lines = self.wrap_spans(
                &[InlineSpan::Text {
                    text: page_str,
                    flags: InlineFlags::default(),
                }],
                &entry_style,
                40.0,
            );
            let page_w = page_lines.first().map(|l| l.width).unwrap_or(12.0);

            let max_title_w = (self.content_w - indent - page_w - 24.0).max(60.0);
            let entry_lines = self.wrap_spans(
                &[InlineSpan::Text {
                    text,
                    flags: InlineFlags::default(),
                }],
                &entry_style,
                max_title_w,
            );

            let row_y = self.cursor_y;
            if let Some(el) = entry_lines.first() {
                self.draw_wrapped_line(el, self.margin_left + indent, ColumnAlign::Left, 0.0);
                // Leader dots between title end and page number
                let dots_start = self.margin_left + indent + el.width + 6.0;
                let dots_end = self.margin_left + self.content_w - page_w - 6.0;
                let dot_y = row_y + el.ascent * 0.85;
                let mut dx = dots_start;
                while dx < dots_end {
                    self.push_prim(LayoutPrimitive::Rect {
                        x: dx,
                        y: dot_y,
                        w: 1.2,
                        h: 1.2,
                        fill: Some(self.theme.table_border_color),
                        stroke: None,
                    });
                    dx += 4.5;
                }
            }
            if let Some(pl) = page_lines.first() {
                self.cursor_y = row_y;
                self.draw_wrapped_line(
                    pl,
                    self.margin_left + self.content_w - page_w,
                    ColumnAlign::Left,
                    0.0,
                );
            }
            self.cursor_y = row_y + line_h;
        }

        let _ = start_y;
        self.cursor_y += self.theme.paragraph_spacing * 1.4;
    }

    fn layout_block(&mut self, block: &DocBlock, x: f32, w: f32, depth: usize) {
        match block {
            DocBlock::TocPlaceholder => {
                // Handled inline
            }
            DocBlock::Heading {
                level,
                spans,
                plain_text,
            } => {
                let h_style: &HeadingStyle = match level {
                    1 => &self.theme.h1_style,
                    2 => &self.theme.h2_style,
                    3 => &self.theme.h3_style,
                    _ => &self.theme.h4_style,
                };
                let after_parent_heading = self
                    .prev_top_heading_level
                    .is_some_and(|prev_lvl| prev_lvl < *level);
                if depth == 0
                    && h_style.page_break_before
                    && self.has_body_content
                    && !after_parent_heading
                    && self.cursor_y > self.margin_top + 2.0
                {
                    self.new_page();
                }
                let size_pt = self.theme.base_font_size * h_style.size_scale;
                let top_space = match level {
                    1 => self.theme.paragraph_spacing * 1.6,
                    2 => self.theme.paragraph_spacing * 1.3,
                    _ => self.theme.paragraph_spacing * 1.0,
                };

                if self.cursor_y > self.margin_top + 2.0 {
                    self.cursor_y += top_space;
                }

                let text_offset_x = if h_style.left_accent_bar { 10.0 } else { 0.0 };
                let base = ResolvedCharStyle {
                    font_id: self.rf.heading,
                    size_pt,
                    color: h_style.color,
                    stroke_boost: h_style.stroke_boost,
                    slant: 0.0,
                    bg_color: None,
                    underline: false,
                    strikethrough: false,
                    link_url: None,
                };

                let lines = self.wrap_spans(spans, &base, w - text_offset_x);
                let total_h: f32 = lines
                    .iter()
                    .map(|l| (l.ascent + l.descent) * 1.25)
                    .sum::<f32>()
                    + if h_style.underline { 6.0 } else { 2.0 };

                // Keep heading together with at least 36pt of following content
                self.ensure_space(total_h + 36.0);

                // Record heading position for TOC and PDF bookmarks
                if depth == 0 {
                    let page_idx = self.pages.len() - 1;
                    self.headings.push(HeadingEntry {
                        level: *level,
                        title: plain_text.clone(),
                        page_index: page_idx,
                        y_pt: self.cursor_y,
                    });
                    self.heading_counter += 1;
                }

                let heading_start_y = self.cursor_y;
                for line in &lines {
                    self.draw_wrapped_line(line, x + text_offset_x, ColumnAlign::Left, w);
                    self.cursor_y += (line.ascent + line.descent) * 1.25;
                }
                let heading_end_y = self.cursor_y;

                if h_style.left_accent_bar {
                    self.push_prim(LayoutPrimitive::Rect {
                        x,
                        y: heading_start_y + 1.0,
                        w: 4.0,
                        h: (heading_end_y - heading_start_y - 3.0).max(size_pt),
                        fill: Some(h_style.color),
                        stroke: None,
                    });
                }

                if h_style.underline {
                    self.cursor_y += 2.0;
                    self.push_prim(LayoutPrimitive::Line {
                        x1: x,
                        y1: self.cursor_y,
                        x2: x + w,
                        y2: self.cursor_y,
                        color: h_style.underline_color,
                        width: if *level == 1 { 1.5 } else { 0.9 },
                    });
                    self.cursor_y += 4.0;
                }

                self.cursor_y += self.theme.paragraph_spacing * 0.65;
            }
            DocBlock::Paragraph(spans) => {
                let base = self.default_body_style();
                let lines = self.wrap_spans(spans, &base, w);
                for line in &lines {
                    let lh = (line.ascent + line.descent) * (self.theme.line_height / 1.04);
                    self.ensure_space(lh);
                    self.draw_wrapped_line(line, x, ColumnAlign::Left, w);
                    self.cursor_y += lh;
                }
                self.cursor_y += self.theme.paragraph_spacing;
            }
            DocBlock::CodeBlock { lang, code } => {
                let code_size = self.theme.base_font_size * 0.90;
                let base = ResolvedCharStyle {
                    font_id: self.rf.code,
                    size_pt: code_size,
                    color: self.theme.code_block_text,
                    stroke_boost: 0.0,
                    slant: 0.0,
                    bg_color: None,
                    underline: false,
                    strikethrough: false,
                    link_url: None,
                };
                let pad_x = 10.0;
                let pad_y = 8.0;
                let header_h = if lang.is_some() { 16.0 } else { 0.0 };

                let mut wrapped_code_lines: Vec<WrappedLine> = Vec::new();
                for raw_line in code.lines() {
                    let text = if raw_line.is_empty() { " " } else { raw_line };
                    let spans = vec![InlineSpan::Text {
                        text: text.to_string(),
                        flags: InlineFlags::default(),
                    }];
                    let mut sub = self.wrap_spans(&spans, &base, w - pad_x * 2.0);
                    wrapped_code_lines.append(&mut sub);
                }
                if wrapped_code_lines.is_empty() {
                    let spans = vec![InlineSpan::Text {
                        text: " ".to_string(),
                        flags: InlineFlags::default(),
                    }];
                    wrapped_code_lines = self.wrap_spans(&spans, &base, w - pad_x * 2.0);
                }

                let line_step = code_size * 1.42;
                let box_h = header_h + pad_y * 2.0 + wrapped_code_lines.len() as f32 * line_step;

                self.ensure_space(box_h.min(self.bottom_limit - self.margin_top));
                let box_y = self.cursor_y;

                // Code block background & border
                self.push_prim(LayoutPrimitive::Rect {
                    x,
                    y: box_y,
                    w,
                    h: box_h,
                    fill: Some(self.theme.code_block_bg),
                    stroke: Some((self.theme.code_block_border, 0.8)),
                });

                if let Some(l_name) = lang {
                    self.push_prim(LayoutPrimitive::Rect {
                        x,
                        y: box_y,
                        w,
                        h: header_h,
                        fill: Some(self.theme.table_header_bg),
                        stroke: Some((self.theme.code_block_border, 0.6)),
                    });
                    let lang_style = ResolvedCharStyle {
                        font_id: self.rf.code,
                        size_pt: code_size * 0.82,
                        color: self.theme.quote_text_color,
                        stroke_boost: 0.4,
                        slant: 0.0,
                        bg_color: None,
                        underline: false,
                        strikethrough: false,
                        link_url: None,
                    };
                    let l_lines = self.wrap_spans(
                        &[InlineSpan::Text {
                            text: l_name.clone(),
                            flags: InlineFlags::default(),
                        }],
                        &lang_style,
                        w - pad_x * 2.0,
                    );
                    if let Some(ll) = l_lines.first() {
                        self.cursor_y = box_y + 2.5;
                        self.draw_wrapped_line(ll, x + pad_x, ColumnAlign::Left, w);
                    }
                }

                self.cursor_y = box_y + header_h + pad_y;
                for cline in &wrapped_code_lines {
                    self.draw_wrapped_line(cline, x + pad_x, ColumnAlign::Left, w);
                    self.cursor_y += line_step;
                }

                self.cursor_y = box_y + box_h + self.theme.paragraph_spacing;
            }
            DocBlock::BlockQuote(sub_blocks) => {
                let pad_left = 12.0;
                let pad_right = 10.0;
                let pad_y = 6.0;
                let inner_w = (w - pad_left - pad_right).max(60.0);

                // Measure height in a dry-run or render directly with background strip
                let est_h = self.estimate_blocks_height(sub_blocks, inner_w) + pad_y * 2.0;
                self.ensure_space(est_h.min(120.0));

                let start_y = self.cursor_y;
                let start_page = self.pages.len() - 1;
                // Reserve background rect index on current page
                let bg_prim_idx = self.current_page().primitives.len();
                self.push_prim(LayoutPrimitive::Rect {
                    x,
                    y: start_y,
                    w,
                    h: 0.0,
                    fill: Some(self.theme.quote_bg_color),
                    stroke: None,
                });
                let bar_prim_idx = self.current_page().primitives.len();
                self.push_prim(LayoutPrimitive::Rect {
                    x,
                    y: start_y,
                    w: 3.5,
                    h: 0.0,
                    fill: Some(self.theme.quote_bar_color),
                    stroke: None,
                });

                self.cursor_y += pad_y;
                for sb in sub_blocks {
                    self.layout_block(sb, x + pad_left, inner_w, depth + 1);
                }
                self.cursor_y = (self.cursor_y - self.theme.paragraph_spacing * 0.5) + pad_y;

                let end_page = self.pages.len() - 1;
                if end_page == start_page {
                    let actual_h = (self.cursor_y - start_y).max(16.0);
                    if let LayoutPrimitive::Rect { h, .. } =
                        &mut self.pages[start_page].primitives[bg_prim_idx]
                    {
                        *h = actual_h;
                    }
                    if let LayoutPrimitive::Rect { h, .. } =
                        &mut self.pages[start_page].primitives[bar_prim_idx]
                    {
                        *h = actual_h;
                    }
                }

                self.cursor_y += self.theme.paragraph_spacing;
            }
            DocBlock::Callout {
                kind,
                title,
                blocks,
            } => {
                let (accent, bg_tint, title_color) = kind.colors();
                let pad_left = 14.0;
                let pad_right = 12.0;
                let pad_y = 9.0;
                let inner_w = (w - pad_left - pad_right).max(60.0);

                let est_h = self.estimate_blocks_height(blocks, inner_w)
                    + self.theme.base_font_size * 1.8
                    + pad_y * 2.0;
                self.ensure_space(est_h.min(140.0));

                let start_y = self.cursor_y;
                let start_page = self.pages.len() - 1;
                let bg_idx = self.current_page().primitives.len();
                self.push_prim(LayoutPrimitive::Rect {
                    x,
                    y: start_y,
                    w,
                    h: 0.0,
                    fill: Some(bg_tint),
                    stroke: Some((accent, 0.6)),
                });
                let bar_idx = self.current_page().primitives.len();
                self.push_prim(LayoutPrimitive::Rect {
                    x,
                    y: start_y,
                    w: 4.5,
                    h: 0.0,
                    fill: Some(accent),
                    stroke: None,
                });

                self.cursor_y += pad_y;

                // Draw callout icon badge + title
                let badge_size = self.theme.base_font_size * 0.95;
                let badge_y = self.cursor_y + 1.5;
                self.push_prim(LayoutPrimitive::Rect {
                    x: x + pad_left,
                    y: badge_y,
                    w: badge_size,
                    h: badge_size,
                    fill: Some(accent),
                    stroke: None,
                });
                // Small inner mark in the badge
                self.push_prim(LayoutPrimitive::Line {
                    x1: x + pad_left + badge_size * 0.5,
                    y1: badge_y + badge_size * 0.22,
                    x2: x + pad_left + badge_size * 0.5,
                    y2: badge_y + badge_size * 0.58,
                    color: [255, 255, 255],
                    width: 1.6,
                });
                self.push_prim(LayoutPrimitive::Rect {
                    x: x + pad_left + badge_size * 0.5 - 0.8,
                    y: badge_y + badge_size * 0.70,
                    w: 1.6,
                    h: 1.6,
                    fill: Some([255, 255, 255]),
                    stroke: None,
                });

                let title_style = ResolvedCharStyle {
                    font_id: self.rf.heading,
                    size_pt: self.theme.base_font_size * 1.02,
                    color: title_color,
                    stroke_boost: 0.65,
                    slant: 0.0,
                    bg_color: None,
                    underline: false,
                    strikethrough: false,
                    link_url: None,
                };
                let title_lines = self.wrap_spans(
                    &[InlineSpan::Text {
                        text: title.clone(),
                        flags: InlineFlags::default(),
                    }],
                    &title_style,
                    inner_w - badge_size - 8.0,
                );
                if let Some(tl) = title_lines.first() {
                    self.draw_wrapped_line(
                        tl,
                        x + pad_left + badge_size + 6.0,
                        ColumnAlign::Left,
                        inner_w,
                    );
                    self.cursor_y += (tl.ascent + tl.descent) + 5.0;
                }

                for b in blocks {
                    self.layout_block(b, x + pad_left, inner_w, depth + 1);
                }
                self.cursor_y = (self.cursor_y - self.theme.paragraph_spacing * 0.5) + pad_y;

                let end_page = self.pages.len() - 1;
                if end_page == start_page {
                    let actual_h = (self.cursor_y - start_y).max(24.0);
                    if let LayoutPrimitive::Rect { h, .. } =
                        &mut self.pages[start_page].primitives[bg_idx]
                    {
                        *h = actual_h;
                    }
                    if let LayoutPrimitive::Rect { h, .. } =
                        &mut self.pages[start_page].primitives[bar_idx]
                    {
                        *h = actual_h;
                    }
                }

                self.cursor_y += self.theme.paragraph_spacing;
            }
            DocBlock::List { ordered, items } => {
                let indent = 18.0;
                for (i, item) in items.iter().enumerate() {
                    let base = self.default_body_style();
                    let lines = self.wrap_spans(&item.spans, &base, (w - indent).max(40.0));
                    let first_lh = lines
                        .first()
                        .map(|l| (l.ascent + l.descent) * (self.theme.line_height / 1.04))
                        .unwrap_or(self.theme.base_font_size * 1.4);

                    self.ensure_space(first_lh);
                    let marker_y = self.cursor_y;

                    // Draw bullet, number, or task checkbox
                    if let Some(checked) = item.checked {
                        let box_sz = self.theme.base_font_size * 0.88;
                        let bx = x + 2.0;
                        let by = marker_y + self.theme.base_font_size * 0.14;
                        self.push_prim(LayoutPrimitive::Rect {
                            x: bx,
                            y: by,
                            w: box_sz,
                            h: box_sz,
                            fill: if checked {
                                Some(self.theme.link_color)
                            } else {
                                Some([255, 255, 255])
                            },
                            stroke: Some((
                                if checked {
                                    self.theme.link_color
                                } else {
                                    self.theme.quote_bar_color
                                },
                                1.0,
                            )),
                        });
                        if checked {
                            // Draw crisp checkmark
                            self.push_prim(LayoutPrimitive::Line {
                                x1: bx + box_sz * 0.22,
                                y1: by + box_sz * 0.52,
                                x2: bx + box_sz * 0.44,
                                y2: by + box_sz * 0.76,
                                color: [255, 255, 255],
                                width: 1.4,
                            });
                            self.push_prim(LayoutPrimitive::Line {
                                x1: bx + box_sz * 0.44,
                                y1: by + box_sz * 0.76,
                                x2: bx + box_sz * 0.80,
                                y2: by + box_sz * 0.24,
                                color: [255, 255, 255],
                                width: 1.4,
                            });
                        }
                    } else if let Some(start_num) = ordered {
                        let num_str = format!("{}.", start_num + i as u64);
                        let mut num_style = base.clone();
                        num_style.stroke_boost = 0.4;
                        num_style.color = self.theme.h3_style.color;
                        let num_lines = self.wrap_spans(
                            &[InlineSpan::Text {
                                text: num_str,
                                flags: InlineFlags::default(),
                            }],
                            &num_style,
                            indent,
                        );
                        if let Some(nl) = num_lines.first() {
                            self.draw_wrapped_line(nl, x, ColumnAlign::Left, indent);
                        }
                    } else {
                        // Bullet dot
                        let dot_sz = (self.theme.base_font_size * 0.34).clamp(3.0, 5.0);
                        self.push_prim(LayoutPrimitive::Rect {
                            x: x + 5.0,
                            y: marker_y + self.theme.base_font_size * 0.45,
                            w: dot_sz,
                            h: dot_sz,
                            fill: Some(self.theme.h2_style.color),
                            stroke: None,
                        });
                    }

                    for line in &lines {
                        let lh = (line.ascent + line.descent) * (self.theme.line_height / 1.04);
                        self.ensure_space(lh);
                        self.draw_wrapped_line(line, x + indent, ColumnAlign::Left, w - indent);
                        self.cursor_y += lh;
                    }

                    for sb in &item.sub_blocks {
                        self.layout_block(sb, x + indent, w - indent, depth + 1);
                    }

                    self.cursor_y += 2.5;
                }
                self.cursor_y += self.theme.paragraph_spacing * 0.6;
            }
            DocBlock::Table {
                alignments,
                headers,
                rows,
            } => {
                let num_cols = headers
                    .len()
                    .max(rows.iter().map(|r| r.len()).max().unwrap_or(0))
                    .max(1);
                let col_w = w / num_cols as f32;
                let pad_x = 7.0;
                let pad_y = 5.5;

                // Helper to render one table row
                let render_row =
                    |ctx: &mut Self, cells: &[Vec<InlineSpan>], is_header: bool, row_idx: usize| {
                        let mut cell_wrapped: Vec<Vec<WrappedLine>> = Vec::with_capacity(num_cols);
                        let mut max_cell_h = ctx.theme.base_font_size * 1.3;

                        for c_idx in 0..num_cols {
                            let spans = cells.get(c_idx).map(|v| v.as_slice()).unwrap_or(&[]);
                            let style = ResolvedCharStyle {
                                font_id: if is_header {
                                    ctx.rf.heading
                                } else {
                                    ctx.rf.body
                                },
                                size_pt: ctx.theme.base_font_size * 0.94,
                                color: if is_header {
                                    ctx.theme.table_header_text
                                } else {
                                    ctx.theme.text_color
                                },
                                stroke_boost: if is_header { 0.55 } else { 0.0 },
                                slant: 0.0,
                                bg_color: None,
                                underline: false,
                                strikethrough: false,
                                link_url: None,
                            };
                            let lines = ctx.wrap_spans(spans, &style, (col_w - pad_x * 2.0).max(20.0));
                            let h: f32 = lines
                                .iter()
                                .map(|l| (l.ascent + l.descent) * 1.25)
                                .sum();
                            max_cell_h = max_cell_h.max(h);
                            cell_wrapped.push(lines);
                        }

                        let row_h = max_cell_h + pad_y * 2.0;
                        ctx.ensure_space(row_h);
                        let row_y = ctx.cursor_y;

                        for c_idx in 0..num_cols {
                            let cx = x + c_idx as f32 * col_w;
                            let fill = if is_header {
                                Some(ctx.theme.table_header_bg)
                            } else if row_idx % 2 == 1 {
                                Some(ctx.theme.table_stripe_bg)
                            } else {
                                None
                            };
                            ctx.push_prim(LayoutPrimitive::Rect {
                                x: cx,
                                y: row_y,
                                w: col_w,
                                h: row_h,
                                fill,
                                stroke: Some((ctx.theme.table_border_color, 0.65)),
                            });

                            let align = alignments
                                .get(c_idx)
                                .copied()
                                .unwrap_or(ColumnAlign::Left);
                            ctx.cursor_y = row_y + pad_y;
                            for l in &cell_wrapped[c_idx] {
                                ctx.draw_wrapped_line(
                                    l,
                                    cx + pad_x,
                                    align,
                                    (col_w - pad_x * 2.0).max(20.0),
                                );
                                ctx.cursor_y += (l.ascent + l.descent) * 1.25;
                            }
                        }

                        ctx.cursor_y = row_y + row_h;
                    };

                if !headers.is_empty() {
                    render_row(self, headers, true, 0);
                }
                for (r_idx, row) in rows.iter().enumerate() {
                    render_row(self, row, false, r_idx);
                }
                self.cursor_y += self.theme.paragraph_spacing;
            }
            DocBlock::DisplayMath(math_node) => {
                let math_size = self.theme.base_font_size * 1.18;
                let mbox = self.layout_math_node(math_node, math_size, self.theme.text_color);
                let total_h = mbox.ascent + mbox.descent + 12.0;
                self.ensure_space(total_h);

                let baseline_y = self.cursor_y + 6.0 + mbox.ascent;
                let start_x = x + ((w - mbox.width) * 0.5).max(0.0);
                self.emit_math_box(&mbox, start_x, baseline_y);
                self.cursor_y += total_h + self.theme.paragraph_spacing * 0.7;
            }
            DocBlock::HorizontalRule => {
                self.ensure_space(16.0);
                self.cursor_y += 6.0;
                self.push_prim(LayoutPrimitive::Line {
                    x1: x,
                    y1: self.cursor_y,
                    x2: x + w,
                    y2: self.cursor_y,
                    color: self.theme.table_border_color,
                    width: 1.0,
                });
                self.cursor_y += 6.0 + self.theme.paragraph_spacing * 0.6;
            }
        }
    }

    fn layout_footnotes(&mut self, footnotes: &[FootnoteDef]) {
        self.ensure_space(36.0);
        self.cursor_y += 8.0;
        self.push_prim(LayoutPrimitive::Line {
            x1: self.margin_left,
            y1: self.cursor_y,
            x2: self.margin_left + self.content_w * 0.38,
            y2: self.cursor_y,
            color: self.theme.table_border_color,
            width: 0.8,
        });
        self.cursor_y += 8.0;

        let fn_size = self.theme.base_font_size * 0.86;
        for fdef in footnotes {
            let mut spans = vec![InlineSpan::Text {
                text: format!("[{}] ", fdef.number),
                flags: InlineFlags {
                    bold: true,
                    ..Default::default()
                },
            }];
            spans.extend(fdef.spans.clone());

            let base = ResolvedCharStyle {
                font_id: self.rf.body,
                size_pt: fn_size,
                color: self.theme.quote_text_color,
                stroke_boost: 0.0,
                slant: 0.0,
                bg_color: None,
                underline: false,
                strikethrough: false,
                link_url: None,
            };
            let lines = self.wrap_spans(&spans, &base, self.content_w);
            for line in &lines {
                let lh = (line.ascent + line.descent) * 1.3;
                self.ensure_space(lh);
                self.draw_wrapped_line(line, self.margin_left, ColumnAlign::Left, self.content_w);
                self.cursor_y += lh;
            }
            self.cursor_y += 3.0;
        }
    }

    fn render_page_numbers(&mut self) {
        let total = self.pages.len();
        let footer_size = (self.theme.base_font_size * 0.84).clamp(7.5, 10.5);
        let footer_style = ResolvedCharStyle {
            font_id: self.rf.body,
            size_pt: footer_size,
            color: self.theme.quote_text_color,
            stroke_boost: 0.0,
            slant: 0.0,
            bg_color: None,
            underline: false,
            strikethrough: false,
            link_url: None,
        };

        for p_idx in 0..total {
            let text = format!("— {} / {} —", p_idx + 1, total);
            let lines = self.wrap_spans(
                &[InlineSpan::Text {
                    text,
                    flags: InlineFlags::default(),
                }],
                &footer_style,
                self.content_w,
            );
            if let Some(line) = lines.first() {
                let baseline_y = self.page_h - self.margin_bottom * 0.55;
                let start_x = (self.page_w - line.width) * 0.5;
                let mut cx = start_x;
                for atom in &line.atoms {
                    if let InlineAtom::Char {
                        glyph_id,
                        width_pt,
                        style,
                        ..
                    } = atom
                    {
                        if *glyph_id != 0 {
                            self.pages[p_idx].primitives.push(LayoutPrimitive::Glyph {
                                font_id: style.font_id,
                                glyph_id: *glyph_id,
                                x: cx,
                                baseline_y,
                                size_pt: style.size_pt,
                                color: style.color,
                                stroke_boost: style.stroke_boost,
                                slant: style.slant,
                            });
                        }
                        cx += *width_pt;
                    }
                }
            }
        }
    }

    fn default_body_style(&self) -> ResolvedCharStyle {
        ResolvedCharStyle {
            font_id: self.rf.body,
            size_pt: self.theme.base_font_size,
            color: self.theme.text_color,
            stroke_boost: 0.0,
            slant: 0.0,
            bg_color: None,
            underline: false,
            strikethrough: false,
            link_url: None,
        }
    }

    fn resolve_span_style(&self, base: &ResolvedCharStyle, flags: &InlineFlags) -> ResolvedCharStyle {
        let mut s = base.clone();
        s.strikethrough = flags.strikethrough;
        s.link_url = flags.link_url.clone();

        if flags.italic {
            apply_emp(&mut s, &self.theme.italic_style);
        }
        if flags.bold {
            apply_emp(&mut s, &self.theme.bold_style);
        }
        if flags.highlight {
            apply_emp(&mut s, &self.theme.highlight_style);
        }
        if flags.link_url.is_some() {
            s.color = self.theme.link_color;
            if self.theme.link_underline {
                s.underline = true;
            }
        }
        s
    }

    fn wrap_spans(
        &self,
        spans: &[InlineSpan],
        base: &ResolvedCharStyle,
        max_w: f32,
    ) -> Vec<WrappedLine> {
        let mut atoms: Vec<InlineAtom> = Vec::new();

        for span in spans {
            match span {
                InlineSpan::Text { text, flags } => {
                    let style = self.resolve_span_style(base, flags);
                    for ch in text.chars() {
                        if ch == '\n' {
                            atoms.push(InlineAtom::ForcedBreak);
                            continue;
                        }
                        if ch == '\r' {
                            continue;
                        }
                        let g = self.fonts.resolve_char(style.font_id, ch);
                        let mut char_style = style.clone();
                        char_style.font_id = g.font_id;
                        atoms.push(InlineAtom::Char {
                            ch,
                            glyph_id: g.glyph_id,
                            width_pt: g.advance_em * char_style.size_pt,
                            style: char_style,
                        });
                    }
                }
                InlineSpan::InlineCode { code, flags } => {
                    let mut style = self.resolve_span_style(base, flags);
                    style.font_id = self.rf.code;
                    apply_emp(&mut style, &self.theme.inline_code_style);
                    for ch in code.chars() {
                        let g = self.fonts.resolve_char(style.font_id, ch);
                        let mut char_style = style.clone();
                        char_style.font_id = g.font_id;
                        atoms.push(InlineAtom::Char {
                            ch,
                            glyph_id: g.glyph_id,
                            width_pt: g.advance_em * char_style.size_pt,
                            style: char_style,
                        });
                    }
                }
                InlineSpan::InlineMath(mnode) => {
                    let mbox = self.layout_math_node(mnode, base.size_pt * 1.04, base.color);
                    atoms.push(InlineAtom::Math { mbox });
                }
                InlineSpan::FootnoteRef { number, .. } => {
                    let mut sup_style = base.clone();
                    sup_style.size_pt = base.size_pt * 0.72;
                    sup_style.color = self.theme.link_color;
                    sup_style.stroke_boost = 0.35;
                    let text = format!("[{}]", number);
                    for ch in text.chars() {
                        let g = self.fonts.resolve_char(sup_style.font_id, ch);
                        let mut cs = sup_style.clone();
                        cs.font_id = g.font_id;
                        atoms.push(InlineAtom::Char {
                            ch,
                            glyph_id: g.glyph_id,
                            width_pt: g.advance_em * cs.size_pt,
                            style: cs,
                        });
                    }
                }
                InlineSpan::SoftBreak => {
                    if self.theme.strict_line_breaks {
                        let g = self.fonts.resolve_char(base.font_id, ' ');
                        atoms.push(InlineAtom::Char {
                            ch: ' ',
                            glyph_id: g.glyph_id,
                            width_pt: g.advance_em * base.size_pt,
                            style: base.clone(),
                        });
                    } else {
                        atoms.push(InlineAtom::ForcedBreak);
                    }
                }
                InlineSpan::HardBreak => {
                    atoms.push(InlineAtom::ForcedBreak);
                }
            }
        }

        // Group atoms into breakable units (ASCII words vs individual CJK / symbols / Math)
        let mut lines: Vec<WrappedLine> = Vec::new();
        let mut cur_atoms: Vec<InlineAtom> = Vec::new();
        let mut cur_w = 0.0f32;

        let mut i = 0;
        while i < atoms.len() {
            if matches!(atoms[i], InlineAtom::ForcedBreak) {
                lines.push(finalize_line(std::mem::take(&mut cur_atoms), base.size_pt));
                cur_w = 0.0;
                i += 1;
                continue;
            }

            // Collect an atomic chunk (either an ASCII word + any trailing kinsoku punctuation, or a single CJK char + trailing kinsoku punctuation)
            let start = i;
            if atoms[i].is_ascii_word_char() {
                while i < atoms.len() && atoms[i].is_ascii_word_char() {
                    i += 1;
                }
            } else {
                i += 1;
            }
            // Attach immediately following kinsoku line-start punctuation so it never starts a new line alone
            while i < atoms.len() && atoms[i].is_kinsoku_line_start() {
                i += 1;
            }

            let chunk = &atoms[start..i];
            let chunk_w: f32 = chunk.iter().map(|a| a.width()).sum();

            if cur_atoms.is_empty() && chunk.len() == 1 && chunk[0].is_whitespace() {
                // Skip leading space on wrapped line
                continue;
            }

            if !cur_atoms.is_empty() && cur_w + chunk_w > max_w {
                lines.push(finalize_line(std::mem::take(&mut cur_atoms), base.size_pt));
                cur_w = 0.0;
                if chunk.len() == 1 && chunk[0].is_whitespace() {
                    continue;
                }
            }

            // If a single long chunk itself exceeds max_w, break it character by character
            if chunk_w > max_w {
                for atom in chunk {
                    let aw = atom.width();
                    if !cur_atoms.is_empty() && cur_w + aw > max_w {
                        lines.push(finalize_line(std::mem::take(&mut cur_atoms), base.size_pt));
                        cur_w = 0.0;
                    }
                    cur_atoms.push(atom.clone());
                    cur_w += aw;
                }
            } else {
                cur_atoms.extend_from_slice(chunk);
                cur_w += chunk_w;
            }
        }

        if !cur_atoms.is_empty() || lines.is_empty() {
            lines.push(finalize_line(cur_atoms, base.size_pt));
        }

        lines
    }

    fn draw_wrapped_line(
        &mut self,
        line: &WrappedLine,
        x: f32,
        align: ColumnAlign,
        container_w: f32,
    ) {
        let start_x = match align {
            ColumnAlign::Left => x,
            ColumnAlign::Center => x + ((container_w - line.width) * 0.5).max(0.0),
            ColumnAlign::Right => x + (container_w - line.width).max(0.0),
        };

        let baseline_y = self.cursor_y + line.ascent;

        // Pass 1: Draw background highlights (so they sit cleanly behind glyphs)
        let mut cx = start_x;
        for atom in &line.atoms {
            let aw = atom.width();
            if let InlineAtom::Char { style, .. } = atom {
                if let Some(bg) = style.bg_color {
                    let bg_y = baseline_y - style.size_pt * 0.82;
                    let bg_h = style.size_pt * 1.06;
                    self.push_prim(LayoutPrimitive::Rect {
                        x: cx - 0.5,
                        y: bg_y,
                        w: aw + 1.0,
                        h: bg_h,
                        fill: Some(bg),
                        stroke: None,
                    });
                }
            }
            cx += aw;
        }

        // Pass 2: Draw glyphs, underlines, strikethroughs, math boxes, and link annotations
        cx = start_x;
        for atom in &line.atoms {
            match atom {
                InlineAtom::Char {
                    glyph_id,
                    width_pt,
                    style,
                    ..
                } => {
                    if *glyph_id != 0 {
                        self.push_prim(LayoutPrimitive::Glyph {
                            font_id: style.font_id,
                            glyph_id: *glyph_id,
                            x: cx,
                            baseline_y,
                            size_pt: style.size_pt,
                            color: style.color,
                            stroke_boost: style.stroke_boost,
                            slant: style.slant,
                        });
                    }
                    if style.underline {
                        let uy = baseline_y + style.size_pt * 0.12;
                        self.push_prim(LayoutPrimitive::Line {
                            x1: cx,
                            y1: uy,
                            x2: cx + *width_pt,
                            y2: uy,
                            color: style.color,
                            width: (style.size_pt * 0.075).max(0.7),
                        });
                    }
                    if style.strikethrough {
                        let sy = baseline_y - style.size_pt * 0.30;
                        self.push_prim(LayoutPrimitive::Line {
                            x1: cx,
                            y1: sy,
                            x2: cx + *width_pt,
                            y2: sy,
                            color: style.color,
                            width: (style.size_pt * 0.075).max(0.7),
                        });
                    }
                    if let Some(url) = &style.link_url {
                        self.push_link(LinkAnnotation {
                            x: cx,
                            y: baseline_y - style.size_pt * 0.85,
                            w: *width_pt,
                            h: style.size_pt * 1.1,
                            url: url.clone(),
                        });
                    }
                    cx += *width_pt;
                }
                InlineAtom::Math { mbox } => {
                    self.emit_math_box(mbox, cx, baseline_y);
                    cx += mbox.width;
                }
                InlineAtom::ForcedBreak => {}
            }
        }
    }

    fn emit_math_box(&mut self, mbox: &MathBox, offset_x: f32, baseline_y: f32) {
        let page = self.current_page();
        for p in &mbox.primitives {
            match p {
                LayoutPrimitive::Rect {
                    x,
                    y,
                    w,
                    h,
                    fill,
                    stroke,
                } => {
                    page.primitives.push(LayoutPrimitive::Rect {
                        x: offset_x + *x,
                        y: baseline_y + *y,
                        w: *w,
                        h: *h,
                        fill: *fill,
                        stroke: *stroke,
                    });
                }
                LayoutPrimitive::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width,
                } => {
                    page.primitives.push(LayoutPrimitive::Line {
                        x1: offset_x + *x1,
                        y1: baseline_y + *y1,
                        x2: offset_x + *x2,
                        y2: baseline_y + *y2,
                        color: *color,
                        width: *width,
                    });
                }
                LayoutPrimitive::Glyph {
                    font_id,
                    glyph_id,
                    x,
                    baseline_y: gy,
                    size_pt,
                    color,
                    stroke_boost,
                    slant,
                } => {
                    page.primitives.push(LayoutPrimitive::Glyph {
                        font_id: *font_id,
                        glyph_id: *glyph_id,
                        x: offset_x + *x,
                        baseline_y: baseline_y + *gy,
                        size_pt: *size_pt,
                        color: *color,
                        stroke_boost: *stroke_boost,
                        slant: *slant,
                    });
                }
            }
        }
    }

    /// Recursively lays out a MathNode in local coordinates where (0, 0) is the left baseline point.
    fn layout_math_node(&self, node: &MathNode, size_pt: f32, color: [u8; 3]) -> MathBox {
        match node {
            MathNode::Row(children) => {
                let mut primitives = Vec::new();
                let mut cur_x = 0.0f32;
                let mut max_ascent = size_pt * 0.78;
                let mut max_descent = size_pt * 0.22;

                for child in children {
                    let cbox = self.layout_math_node(child, size_pt, color);
                    max_ascent = max_ascent.max(cbox.ascent);
                    max_descent = max_descent.max(cbox.descent);
                    for p in cbox.primitives {
                        primitives.push(shift_primitive(p, cur_x, 0.0));
                    }
                    cur_x += cbox.width;
                }

                MathBox {
                    width: cur_x,
                    ascent: max_ascent,
                    descent: max_descent,
                    primitives,
                }
            }
            MathNode::Text(s) | MathNode::Operator(s) => {
                let is_op = matches!(node, MathNode::Operator(_));
                let mut cur_x = 0.0f32;
                let mut primitives = Vec::new();

                for ch in s.chars() {
                    let g = self.fonts.resolve_char(self.rf.body, ch);
                    let w = g.advance_em * size_pt;
                    let slant = if !is_op && ch.is_ascii_alphabetic() {
                        0.18
                    } else {
                        0.0
                    };
                    if g.glyph_id != 0 {
                        primitives.push(LayoutPrimitive::Glyph {
                            font_id: g.font_id,
                            glyph_id: g.glyph_id,
                            x: cur_x,
                            baseline_y: 0.0,
                            size_pt,
                            color,
                            stroke_boost: 0.05,
                            slant,
                        });
                    }
                    cur_x += w;
                }

                MathBox {
                    width: cur_x,
                    ascent: size_pt * 0.80,
                    descent: size_pt * 0.22,
                    primitives,
                }
            }
            MathNode::Frac(num, den) => {
                let child_size = (size_pt * 0.78).max(6.0);
                let nbox = self.layout_math_node(num, child_size, color);
                let dbox = self.layout_math_node(den, child_size, color);
                let pad_x = 3.0;
                let bar_w = nbox.width.max(dbox.width) + pad_x * 2.0;
                let axis_y = -size_pt * 0.28; // Y is negative above baseline

                let num_baseline_y = axis_y - 2.5 - nbox.descent;
                let den_baseline_y = axis_y + 2.5 + dbox.ascent;

                let mut primitives = Vec::new();
                let num_x = (bar_w - nbox.width) * 0.5;
                for p in nbox.primitives {
                    primitives.push(shift_primitive(p, num_x, num_baseline_y));
                }
                let den_x = (bar_w - dbox.width) * 0.5;
                for p in dbox.primitives {
                    primitives.push(shift_primitive(p, den_x, den_baseline_y));
                }
                primitives.push(LayoutPrimitive::Line {
                    x1: 1.0,
                    y1: axis_y,
                    x2: bar_w - 1.0,
                    y2: axis_y,
                    color,
                    width: (size_pt * 0.065).max(0.65),
                });

                let ascent = (-num_baseline_y + nbox.ascent).max(size_pt * 0.85);
                let descent = (den_baseline_y + dbox.descent).max(size_pt * 0.30);

                MathBox {
                    width: bar_w + 2.0,
                    ascent,
                    descent,
                    primitives,
                }
            }
            MathNode::Sqrt(inner) => {
                let ibox = self.layout_math_node(inner, size_pt, color);
                let hook_w = size_pt * 0.55;
                let top_pad = 2.5;
                let top_y = -(ibox.ascent + top_pad);
                let bottom_y = ibox.descent * 0.65;
                let stroke_w = (size_pt * 0.068).max(0.7);

                let mut primitives = Vec::new();
                // Radical symbol lines
                primitives.push(LayoutPrimitive::Line {
                    x1: 0.5,
                    y1: -ibox.ascent * 0.35,
                    x2: hook_w * 0.42,
                    y2: bottom_y,
                    color,
                    width: stroke_w * 1.15,
                });
                primitives.push(LayoutPrimitive::Line {
                    x1: hook_w * 0.42,
                    y1: bottom_y,
                    x2: hook_w,
                    y2: top_y,
                    color,
                    width: stroke_w,
                });
                // Vinculum top bar
                primitives.push(LayoutPrimitive::Line {
                    x1: hook_w,
                    y1: top_y,
                    x2: hook_w + ibox.width + 2.5,
                    y2: top_y,
                    color,
                    width: stroke_w,
                });

                for p in ibox.primitives {
                    primitives.push(shift_primitive(p, hook_w + 1.5, 0.0));
                }

                MathBox {
                    width: hook_w + ibox.width + 4.0,
                    ascent: ibox.ascent + top_pad + 1.5,
                    descent: ibox.descent.max(bottom_y),
                    primitives,
                }
            }
            MathNode::SupSub { base, sup, sub } => {
                let bbox = self.layout_math_node(base, size_pt, color);
                let script_size = (size_pt * 0.68).max(5.5);
                let mut primitives = bbox.primitives;
                let mut script_w = 0.0f32;
                let mut ascent = bbox.ascent;
                let mut descent = bbox.descent;

                if let Some(sup_node) = sup {
                    let sup_box = self.layout_math_node(sup_node, script_size, color);
                    let sup_y = -(bbox.ascent * 0.52);
                    script_w = script_w.max(sup_box.width);
                    ascent = ascent.max(-sup_y + sup_box.ascent);
                    for p in sup_box.primitives {
                        primitives.push(shift_primitive(p, bbox.width + 0.8, sup_y));
                    }
                }
                if let Some(sub_node) = sub {
                    let sub_box = self.layout_math_node(sub_node, script_size, color);
                    let sub_y = bbox.descent * 0.65 + script_size * 0.25;
                    script_w = script_w.max(sub_box.width);
                    descent = descent.max(sub_y + sub_box.descent);
                    for p in sub_box.primitives {
                        primitives.push(shift_primitive(p, bbox.width + 0.8, sub_y));
                    }
                }

                MathBox {
                    width: bbox.width + script_w + 1.5,
                    ascent,
                    descent,
                    primitives,
                }
            }
        }
    }

    fn estimate_blocks_height(&self, blocks: &[DocBlock], w: f32) -> f32 {
        let mut h = 0.0f32;
        let base = self.default_body_style();
        for b in blocks {
            match b {
                DocBlock::Paragraph(spans) => {
                    let lines = self.wrap_spans(spans, &base, w);
                    let lh: f32 = lines
                        .iter()
                        .map(|l| (l.ascent + l.descent) * (self.theme.line_height / 1.04))
                        .sum();
                    h += lh + self.theme.paragraph_spacing * 0.5;
                }
                _ => {
                    h += self.theme.base_font_size * 2.2;
                }
            }
        }
        h
    }
}

fn apply_emp(target: &mut ResolvedCharStyle, emp: &EmphasisStyle) {
    target.color = emp.color;
    target.size_pt *= emp.size_scale;
    target.stroke_boost = target.stroke_boost.max(emp.stroke_boost);
    target.slant = target.slant.max(emp.slant);
    if emp.use_background {
        target.bg_color = Some(emp.background_color);
    }
    if emp.underline {
        target.underline = true;
    }
}

fn finalize_line(mut atoms: Vec<InlineAtom>, default_size: f32) -> WrappedLine {
    while atoms.last().map(|a| a.is_whitespace()).unwrap_or(false) {
        atoms.pop();
    }
    let width: f32 = atoms.iter().map(|a| a.width()).sum();
    let ascent = atoms
        .iter()
        .map(|a| a.ascent())
        .fold(default_size * 0.82, f32::max);
    let descent = atoms
        .iter()
        .map(|a| a.descent())
        .fold(default_size * 0.22, f32::max);
    WrappedLine {
        atoms,
        width,
        ascent,
        descent,
    }
}

fn shift_primitive(p: LayoutPrimitive, dx: f32, dy: f32) -> LayoutPrimitive {
    match p {
        LayoutPrimitive::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
        } => LayoutPrimitive::Rect {
            x: x + dx,
            y: y + dy,
            w,
            h,
            fill,
            stroke,
        },
        LayoutPrimitive::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => LayoutPrimitive::Line {
            x1: x1 + dx,
            y1: y1 + dy,
            x2: x2 + dx,
            y2: y2 + dy,
            color,
            width,
        },
        LayoutPrimitive::Glyph {
            font_id,
            glyph_id,
            x,
            baseline_y,
            size_pt,
            color,
            stroke_boost,
            slant,
        } => LayoutPrimitive::Glyph {
            font_id,
            glyph_id,
            x: x + dx,
            baseline_y: baseline_y + dy,
            size_pt,
            color,
            stroke_boost,
            slant,
        },
    }
}
