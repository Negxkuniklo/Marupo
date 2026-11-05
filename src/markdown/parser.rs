use crate::markdown::frontmatter::{extract_frontmatter, FrontmatterData};
use crate::markdown::math::{parse_latex_math, MathNode};
use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

impl CalloutKind {
    pub fn default_title(&self) -> &'static str {
        match self {
            Self::Note => "NOTE / 補足",
            Self::Tip => "TIP / ヒント",
            Self::Important => "IMPORTANT / 重要",
            Self::Warning => "WARNING / 注意",
            Self::Caution => "CAUTION / 警告",
        }
    }

    pub fn colors(&self) -> ([u8; 3], [u8; 3], [u8; 3]) {
        // Returns (accent_border_color, background_tint, title_text_color)
        match self {
            Self::Note => ([37, 99, 235], [239, 246, 255], [29, 78, 216]),
            Self::Tip => ([22, 163, 74], [240, 253, 244], [21, 128, 61]),
            Self::Important => ([147, 51, 234], [250, 245, 255], [126, 34, 206]),
            Self::Warning => ([217, 119, 6], [255, 251, 235], [180, 83, 9]),
            Self::Caution => ([220, 38, 38], [254, 242, 242], [185, 28, 28]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InlineFlags {
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub highlight: bool,
    pub link_url: Option<String>,
}

#[derive(Debug, Clone)]
pub enum InlineSpan {
    Text { text: String, flags: InlineFlags },
    InlineCode { code: String, flags: InlineFlags },
    InlineMath(MathNode),
    FootnoteRef { label: String, number: usize },
    SoftBreak,
    HardBreak,
}

impl InlineSpan {
    pub fn plain_text(&self) -> String {
        match self {
            Self::Text { text, .. } => text.clone(),
            Self::InlineCode { code, .. } => code.clone(),
            Self::InlineMath(_) => "[数式]".to_string(),
            Self::FootnoteRef { number, .. } => format!("[{}]", number),
            Self::SoftBreak | Self::HardBreak => " ".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub spans: Vec<InlineSpan>,
    pub sub_blocks: Vec<DocBlock>,
}

#[derive(Debug, Clone)]
pub enum DocBlock {
    Heading {
        level: u8,
        spans: Vec<InlineSpan>,
        plain_text: String,
    },
    Paragraph(Vec<InlineSpan>),
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    BlockQuote(Vec<DocBlock>),
    Callout {
        kind: CalloutKind,
        title: String,
        blocks: Vec<DocBlock>,
    },
    List {
        ordered: Option<u64>,
        items: Vec<ListItem>,
    },
    Table {
        alignments: Vec<ColumnAlign>,
        headers: Vec<Vec<InlineSpan>>,
        rows: Vec<Vec<Vec<InlineSpan>>>,
    },
    DisplayMath(MathNode),
    HorizontalRule,
    TocPlaceholder,
}

#[derive(Debug, Clone)]
pub struct FootnoteDef {
    pub label: String,
    pub number: usize,
    pub spans: Vec<InlineSpan>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedDocument {
    pub frontmatter: FrontmatterData,
    pub blocks: Vec<DocBlock>,
    pub footnotes: Vec<FootnoteDef>,
}

pub fn parse_markdown(input: &str) -> ParsedDocument {
    let (frontmatter, body) = extract_frontmatter(input);

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(body, opts);
    let events: Vec<Event> = parser.collect();

    let mut builder = AstBuilder::new();
    builder.process_events(&events);

    ParsedDocument {
        frontmatter,
        blocks: builder.root_blocks,
        footnotes: builder.footnotes,
    }
}

enum Container {
    Root,
    Heading { level: u8, spans: Vec<InlineSpan> },
    Paragraph { spans: Vec<InlineSpan> },
    CodeBlock { lang: Option<String>, code: String },
    BlockQuote { blocks: Vec<DocBlock> },
    List { ordered: Option<u64>, items: Vec<ListItem> },
    Item {
        checked: Option<bool>,
        spans: Vec<InlineSpan>,
        sub_blocks: Vec<DocBlock>,
    },
    Table {
        alignments: Vec<ColumnAlign>,
        headers: Vec<Vec<InlineSpan>>,
        rows: Vec<Vec<Vec<InlineSpan>>>,
        in_head: bool,
        current_row: Vec<Vec<InlineSpan>>,
    },
    TableCell { spans: Vec<InlineSpan> },
    FootnoteDefinition { label: String, spans: Vec<InlineSpan> },
}

struct AstBuilder {
    root_blocks: Vec<DocBlock>,
    stack: Vec<Container>,
    flags: InlineFlags,
    footnote_numbers: HashMap<String, usize>,
    footnotes: Vec<FootnoteDef>,
}

impl AstBuilder {
    fn new() -> Self {
        Self {
            root_blocks: Vec::new(),
            stack: vec![Container::Root],
            flags: InlineFlags::default(),
            footnote_numbers: HashMap::new(),
            footnotes: Vec::new(),
        }
    }

    fn get_or_assign_footnote(&mut self, label: &str) -> usize {
        if let Some(&num) = self.footnote_numbers.get(label) {
            num
        } else {
            let next = self.footnote_numbers.len() + 1;
            self.footnote_numbers.insert(label.to_string(), next);
            next
        }
    }

    fn process_events(&mut self, events: &[Event]) {
        for ev in events {
            match ev {
                Event::Start(tag) => self.start_tag(tag),
                Event::End(tag_end) => self.end_tag(tag_end),
                Event::Text(text) => self.push_text_with_highlights(text),
                Event::Code(code) => {
                    let span = InlineSpan::InlineCode {
                        code: code.to_string(),
                        flags: self.flags.clone(),
                    };
                    self.push_inline(span);
                }
                Event::InlineMath(math_str) => {
                    let node = parse_latex_math(math_str);
                    self.push_inline(InlineSpan::InlineMath(node));
                }
                Event::DisplayMath(math_str) => {
                    let node = parse_latex_math(math_str);
                    self.push_block(DocBlock::DisplayMath(node));
                }
                Event::SoftBreak => self.push_inline(InlineSpan::SoftBreak),
                Event::HardBreak => self.push_inline(InlineSpan::HardBreak),
                Event::Rule => self.push_block(DocBlock::HorizontalRule),
                Event::FootnoteReference(label) => {
                    let num = self.get_or_assign_footnote(label);
                    self.push_inline(InlineSpan::FootnoteRef {
                        label: label.to_string(),
                        number: num,
                    });
                }
                Event::TaskListMarker(checked) => {
                    for cont in self.stack.iter_mut().rev() {
                        if let Container::Item {
                            checked: item_checked,
                            ..
                        } = cont
                        {
                            *item_checked = Some(*checked);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn start_tag(&mut self, tag: &Tag) {
        match tag {
            Tag::Paragraph => {
                self.stack.push(Container::Paragraph { spans: Vec::new() });
            }
            Tag::Heading { level, .. } => {
                let lvl = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                self.stack.push(Container::Heading {
                    level: lvl,
                    spans: Vec::new(),
                });
            }
            Tag::BlockQuote(_) => {
                self.stack
                    .push(Container::BlockQuote { blocks: Vec::new() });
            }
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => {
                        let t = l.trim();
                        if t.is_empty() {
                            None
                        } else {
                            Some(t.to_string())
                        }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.stack.push(Container::CodeBlock {
                    lang,
                    code: String::new(),
                });
            }
            Tag::List(ordered) => {
                self.stack.push(Container::List {
                    ordered: *ordered,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.stack.push(Container::Item {
                    checked: None,
                    spans: Vec::new(),
                    sub_blocks: Vec::new(),
                });
            }
            Tag::Table(aligns) => {
                let alignments = aligns
                    .iter()
                    .map(|a| match a {
                        Alignment::None | Alignment::Left => ColumnAlign::Left,
                        Alignment::Center => ColumnAlign::Center,
                        Alignment::Right => ColumnAlign::Right,
                    })
                    .collect();
                self.stack.push(Container::Table {
                    alignments,
                    headers: Vec::new(),
                    rows: Vec::new(),
                    in_head: false,
                    current_row: Vec::new(),
                });
            }
            Tag::TableHead => {
                if let Some(Container::Table {
                    in_head,
                    current_row,
                    ..
                }) = self.stack.last_mut()
                {
                    *in_head = true;
                    current_row.clear();
                }
            }
            Tag::TableRow => {
                if let Some(Container::Table { current_row, .. }) = self.stack.last_mut() {
                    current_row.clear();
                }
            }
            Tag::TableCell => {
                self.stack.push(Container::TableCell { spans: Vec::new() });
            }
            Tag::FootnoteDefinition(label) => {
                self.stack.push(Container::FootnoteDefinition {
                    label: label.to_string(),
                    spans: Vec::new(),
                });
            }
            Tag::Strong => self.flags.bold = true,
            Tag::Emphasis => self.flags.italic = true,
            Tag::Strikethrough => self.flags.strikethrough = true,
            Tag::Link { dest_url, .. } => {
                self.flags.link_url = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag_end: &TagEnd) {
        match tag_end {
            TagEnd::Strong => self.flags.bold = false,
            TagEnd::Emphasis => self.flags.italic = false,
            TagEnd::Strikethrough => self.flags.strikethrough = false,
            TagEnd::Link => self.flags.link_url = None,
            TagEnd::Paragraph => {
                if let Some(Container::Paragraph { spans }) = self.stack.pop() {
                    let joined: String = spans.iter().map(|s| s.plain_text()).collect();
                    let trimmed = joined.trim();
                    if trimmed.eq_ignore_ascii_case("[toc]")
                        || trimmed.eq_ignore_ascii_case("[[toc]]")
                    {
                        self.push_block(DocBlock::TocPlaceholder);
                    } else if let Some(math_inner) = trimmed
                        .strip_prefix("$$")
                        .and_then(|s| s.strip_suffix("$$"))
                    {
                        self.push_block(DocBlock::DisplayMath(parse_latex_math(math_inner)));
                    } else {
                        // If inside an Item or FootnoteDefinition and item.spans is empty, merge into item.spans
                        match self.stack.last_mut() {
                            Some(Container::Item {
                                spans: item_spans,
                                sub_blocks,
                                ..
                            }) => {
                                if item_spans.is_empty() && sub_blocks.is_empty() {
                                    *item_spans = spans;
                                } else {
                                    sub_blocks.push(DocBlock::Paragraph(spans));
                                }
                            }
                            Some(Container::FootnoteDefinition {
                                spans: fn_spans, ..
                            }) => {
                                if !fn_spans.is_empty() {
                                    fn_spans.push(InlineSpan::SoftBreak);
                                }
                                fn_spans.extend(spans);
                            }
                            _ => {
                                self.push_block(DocBlock::Paragraph(spans));
                            }
                        }
                    }
                }
            }
            TagEnd::Heading(_) => {
                if let Some(Container::Heading { level, spans }) = self.stack.pop() {
                    let plain_text: String = spans.iter().map(|s| s.plain_text()).collect();
                    self.push_block(DocBlock::Heading {
                        level,
                        spans,
                        plain_text: plain_text.trim().to_string(),
                    });
                }
            }
            TagEnd::CodeBlock => {
                if let Some(Container::CodeBlock { lang, code }) = self.stack.pop() {
                    if lang.as_deref() == Some("math") || lang.as_deref() == Some("latex") {
                        self.push_block(DocBlock::DisplayMath(parse_latex_math(&code)));
                    } else {
                        self.push_block(DocBlock::CodeBlock {
                            lang,
                            code: code.trim_end_matches(&['\r', '\n'][..]).to_string(),
                        });
                    }
                }
            }
            TagEnd::BlockQuote(_) => {
                if let Some(Container::BlockQuote { blocks }) = self.stack.pop() {
                    let block = convert_blockquote_or_callout(blocks);
                    self.push_block(block);
                }
            }
            TagEnd::Item => {
                if let Some(Container::Item {
                    checked,
                    spans,
                    sub_blocks,
                }) = self.stack.pop()
                {
                    if let Some(Container::List { items, .. }) = self.stack.last_mut() {
                        items.push(ListItem {
                            checked,
                            spans,
                            sub_blocks,
                        });
                    }
                }
            }
            TagEnd::List(_) => {
                if let Some(Container::List { ordered, items }) = self.stack.pop() {
                    self.push_block(DocBlock::List { ordered, items });
                }
            }
            TagEnd::TableCell => {
                if let Some(Container::TableCell { spans }) = self.stack.pop() {
                    if let Some(Container::Table { current_row, .. }) = self.stack.last_mut() {
                        current_row.push(spans);
                    }
                }
            }
            TagEnd::TableHead => {
                if let Some(Container::Table {
                    headers,
                    in_head,
                    current_row,
                    ..
                }) = self.stack.last_mut()
                {
                    *headers = std::mem::take(current_row);
                    *in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(Container::Table {
                    rows,
                    in_head,
                    current_row,
                    ..
                }) = self.stack.last_mut()
                {
                    if !*in_head {
                        rows.push(std::mem::take(current_row));
                    }
                }
            }
            TagEnd::Table => {
                if let Some(Container::Table {
                    alignments,
                    headers,
                    rows,
                    ..
                }) = self.stack.pop()
                {
                    self.push_block(DocBlock::Table {
                        alignments,
                        headers,
                        rows,
                    });
                }
            }
            TagEnd::FootnoteDefinition => {
                if let Some(Container::FootnoteDefinition { label, spans }) = self.stack.pop() {
                    let number = self.get_or_assign_footnote(&label);
                    self.footnotes.push(FootnoteDef {
                        label,
                        number,
                        spans,
                    });
                }
            }
            _ => {}
        }
    }

    fn push_text_with_highlights(&mut self, raw_text: &str) {
        if let Some(Container::CodeBlock { code, .. }) = self.stack.last_mut() {
            code.push_str(raw_text);
            return;
        }

        // Parse `==highlight==` inline syntax inside normal text
        let mut rest = raw_text;
        while let Some(start_idx) = rest.find("==") {
            let before = &rest[..start_idx];
            if !before.is_empty() {
                self.push_inline(InlineSpan::Text {
                    text: before.to_string(),
                    flags: self.flags.clone(),
                });
            }
            let after_open = &rest[start_idx + 2..];
            if let Some(end_idx) = after_open.find("==") {
                let highlighted = &after_open[..end_idx];
                if !highlighted.is_empty() {
                    let mut hl_flags = self.flags.clone();
                    hl_flags.highlight = true;
                    self.push_inline(InlineSpan::Text {
                        text: highlighted.to_string(),
                        flags: hl_flags,
                    });
                }
                rest = &after_open[end_idx + 2..];
            } else {
                // Unclosed `==`, treat as normal text
                self.push_inline(InlineSpan::Text {
                    text: rest[start_idx..].to_string(),
                    flags: self.flags.clone(),
                });
                return;
            }
        }

        if !rest.is_empty() {
            self.push_inline(InlineSpan::Text {
                text: rest.to_string(),
                flags: self.flags.clone(),
            });
        }
    }

    fn push_inline(&mut self, span: InlineSpan) {
        match self.stack.last_mut() {
            Some(Container::Paragraph { spans })
            | Some(Container::Heading { spans, .. })
            | Some(Container::TableCell { spans })
            | Some(Container::FootnoteDefinition { spans, .. }) => {
                spans.push(span);
            }
            Some(Container::Item { spans, .. }) => {
                spans.push(span);
            }
            _ => {}
        }
    }

    fn push_block(&mut self, block: DocBlock) {
        match self.stack.last_mut() {
            Some(Container::Root) => self.root_blocks.push(block),
            Some(Container::BlockQuote { blocks }) => blocks.push(block),
            Some(Container::Item { sub_blocks, .. }) => sub_blocks.push(block),
            _ => self.root_blocks.push(block),
        }
    }
}

fn convert_blockquote_or_callout(mut blocks: Vec<DocBlock>) -> DocBlock {
    if let Some(DocBlock::Paragraph(spans)) = blocks.first_mut() {
        if let Some(InlineSpan::Text { text, .. }) = spans.first_mut() {
            let trimmed = text.trim_start();
            let markers = [
                ("[!NOTE]", CalloutKind::Note),
                ("[!TIP]", CalloutKind::Tip),
                ("[!IMPORTANT]", CalloutKind::Important),
                ("[!WARNING]", CalloutKind::Warning),
                ("[!CAUTION]", CalloutKind::Caution),
            ];

            for (marker, kind) in markers {
                if trimmed.len() >= marker.len()
                    && trimmed[..marker.len()].eq_ignore_ascii_case(marker)
                {
                    let after_marker = trimmed[marker.len()..].trim_start_matches(' ');
                    // Extract custom title if on the same line before a SoftBreak/HardBreak
                    let mut title = kind.default_title().to_string();
                    if !after_marker.is_empty() {
                        title = after_marker.to_string();
                        spans.remove(0);
                        // Also remove leading SoftBreak/HardBreak if present
                        if matches!(
                            spans.first(),
                            Some(InlineSpan::SoftBreak | InlineSpan::HardBreak)
                        ) {
                            spans.remove(0);
                        }
                    } else {
                        spans.remove(0);
                        if matches!(
                            spans.first(),
                            Some(InlineSpan::SoftBreak | InlineSpan::HardBreak)
                        ) {
                            spans.remove(0);
                        }
                    }

                    if spans.is_empty() {
                        blocks.remove(0);
                    }
                    return DocBlock::Callout {
                        kind,
                        title,
                        blocks,
                    };
                }
            }
        }
    }

    DocBlock::BlockQuote(blocks)
}
