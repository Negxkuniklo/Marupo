use crate::config::theme::{MetadataDisplayMode, PaperSize, ThemeConfig};

/// Generates a human-friendly CSS representation of the current ThemeConfig
pub fn theme_to_css(theme: &ThemeConfig) -> String {
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let page_size = match theme.paper_size {
        PaperSize::A4 => "A4",
        PaperSize::B5 => "B5",
        PaperSize::Letter => "Letter",
    };
    let orientation = if theme.landscape { " landscape" } else { "" };
    let fm_display = match theme.metadata_mode {
        MetadataDisplayMode::Hidden => "none",
        MetadataDisplayMode::HeaderCard => "block",
        MetadataDisplayMode::PropertyTable => "table",
    };
    let toc_display = if theme.show_toc { "block" } else { "none" };

    format!(
        r#"/* Markdown to PDF - Pro Custom Stylesheet */
@page {{
  size: {page_size}{orientation};
  margin: {mt:.1}mm {mr:.1}mm {mb:.1}mm {ml:.1}mm;
}}

body {{
  font-family: "{body_font}";
  font-size: {base_size:.1}pt;
  line-height: {line_height:.2};
  color: {text_color};
  background-color: {page_bg};
}}

.frontmatter {{
  display: {fm_display}; /* none | block | table */
}}

.toc {{
  display: {toc_display}; /* none | block */
}}

h1, h2, h3, h4 {{
  font-family: "{heading_font}";
}}

h1 {{
  color: {h1_color};
  font-size: {h1_scale:.2}em;
  border-bottom: {h1_border};
}}

h2 {{
  color: {h2_color};
  font-size: {h2_scale:.2}em;
  border-bottom: {h2_border};
  border-left: {h2_left};
  page-break-before: {h2_break};
}}

h3 {{
  color: {h3_color};
  font-size: {h3_scale:.2}em;
}}

strong, b {{
  color: {bold_color};
  font-size: {bold_scale:.2}em;
  background-color: {bold_bg};
  text-decoration: {bold_ul};
}}

em, i {{
  color: {italic_color};
  font-size: {italic_scale:.2}em;
}}

strong em, em strong {{
  color: {bi_color};
  font-size: {bi_scale:.2}em;
}}

mark {{
  color: {mark_color};
  background-color: {mark_bg};
  font-size: {mark_scale:.2}em;
}}

code {{
  font-family: "{code_font}";
  color: {code_color};
  background-color: {code_bg};
  font-size: {code_scale:.2}em;
}}

pre {{
  background-color: {pre_bg};
  color: {pre_text};
  border-color: {pre_border};
}}

blockquote {{
  border-left-color: {quote_bar};
  background-color: {quote_bg};
  color: {quote_text};
}}

a {{
  color: {link_color};
  text-decoration: {link_ul};
}}

th {{
  background-color: {th_bg};
  color: {th_text};
}}
"#,
        page_size = page_size,
        orientation = orientation,
        mt = theme.margin_top_mm,
        mr = theme.margin_right_mm,
        mb = theme.margin_bottom_mm,
        ml = theme.margin_left_mm,
        body_font = theme.body_font,
        base_size = theme.base_font_size,
        line_height = theme.line_height,
        text_color = hex(theme.text_color),
        page_bg = hex(theme.page_background),
        fm_display = fm_display,
        toc_display = toc_display,
        heading_font = theme.heading_font,
        h1_color = hex(theme.h1_style.color),
        h1_scale = theme.h1_style.size_scale,
        h1_border = if theme.h1_style.underline {
            format!("2px solid {}", hex(theme.h1_style.underline_color))
        } else {
            "none".to_string()
        },
        h2_color = hex(theme.h2_style.color),
        h2_scale = theme.h2_style.size_scale,
        h2_border = if theme.h2_style.underline {
            format!("1px solid {}", hex(theme.h2_style.underline_color))
        } else {
            "none".to_string()
        },
        h2_left = if theme.h2_style.left_accent_bar {
            format!("4px solid {}", hex(theme.h2_style.color))
        } else {
            "none".to_string()
        },
        h2_break = if theme.h2_style.page_break_before {
            "always"
        } else {
            "auto"
        },
        h3_color = hex(theme.h3_style.color),
        h3_scale = theme.h3_style.size_scale,
        bold_color = hex(theme.bold_style.color),
        bold_scale = theme.bold_style.size_scale,
        bold_bg = if theme.bold_style.use_background {
            hex(theme.bold_style.background_color)
        } else {
            "transparent".to_string()
        },
        bold_ul = if theme.bold_style.underline {
            "underline"
        } else {
            "none"
        },
        italic_color = hex(theme.italic_style.color),
        italic_scale = theme.italic_style.size_scale,
        bi_color = hex(theme.bold_italic_style.color),
        bi_scale = theme.bold_italic_style.size_scale,
        mark_color = hex(theme.highlight_style.color),
        mark_bg = hex(theme.highlight_style.background_color),
        mark_scale = theme.highlight_style.size_scale,
        code_font = theme.code_font,
        code_color = hex(theme.inline_code_style.color),
        code_bg = hex(theme.inline_code_style.background_color),
        code_scale = theme.inline_code_style.size_scale,
        pre_bg = hex(theme.code_block_bg),
        pre_text = hex(theme.code_block_text),
        pre_border = hex(theme.code_block_border),
        quote_bar = hex(theme.quote_bar_color),
        quote_bg = hex(theme.quote_bg_color),
        quote_text = hex(theme.quote_text_color),
        link_color = hex(theme.link_color),
        link_ul = if theme.link_underline {
            "underline"
        } else {
            "none"
        },
        th_bg = hex(theme.table_header_bg),
        th_text = hex(theme.table_header_text),
    )
}

/// Applies CSS rules from a stylesheet string onto the given ThemeConfig.
/// Returns the number of CSS property declarations successfully applied.
pub fn apply_css_to_theme(css: &str, theme: &mut ThemeConfig) -> Result<usize, String> {
    let clean = strip_css_comments(css);
    let mut applied_count = 0usize;

    for block in clean.split('}') {
        let Some((selectors_raw, body_raw)) = block.split_once('{') else {
            continue;
        };
        let selectors: Vec<String> = selectors_raw
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        for decl in body_raw.split(';') {
            let Some((prop_raw, val_raw)) = decl.split_once(':') else {
                continue;
            };
            let prop = prop_raw.trim().to_lowercase();
            let val = val_raw.trim();
            if prop.is_empty() || val.is_empty() {
                continue;
            }

            for sel in &selectors {
                if apply_declaration(sel, &prop, val, theme) {
                    applied_count += 1;
                }
            }
        }
    }

    Ok(applied_count)
}

fn strip_css_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(nc) = chars.next() {
                if nc == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn apply_declaration(sel: &str, prop: &str, val: &str, theme: &mut ThemeConfig) -> bool {
    match sel {
        "@page" => match prop {
            "size" => {
                let lower = val.to_lowercase();
                if lower.contains("b5") {
                    theme.paper_size = PaperSize::B5;
                } else if lower.contains("letter") {
                    theme.paper_size = PaperSize::Letter;
                } else if lower.contains("a4") {
                    theme.paper_size = PaperSize::A4;
                }
                theme.landscape = lower.contains("landscape");
                true
            }
            "margin" => {
                let nums: Vec<f32> = val
                    .split_whitespace()
                    .filter_map(parse_length_mm)
                    .collect();
                match nums.as_slice() {
                    [all] => {
                        theme.margin_top_mm = *all;
                        theme.margin_right_mm = *all;
                        theme.margin_bottom_mm = *all;
                        theme.margin_left_mm = *all;
                        true
                    }
                    [v, h] => {
                        theme.margin_top_mm = *v;
                        theme.margin_bottom_mm = *v;
                        theme.margin_left_mm = *h;
                        theme.margin_right_mm = *h;
                        true
                    }
                    [t, r, b, l] => {
                        theme.margin_top_mm = *t;
                        theme.margin_right_mm = *r;
                        theme.margin_bottom_mm = *b;
                        theme.margin_left_mm = *l;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        },
        "body" | "html" | ":root" => match prop {
            "font-family" => {
                if let Some(f) = parse_font_family(val) {
                    theme.body_font = f;
                    return true;
                }
                false
            }
            "font-size" => {
                if let Some(pt) = parse_font_size_pt(val, theme.base_font_size) {
                    theme.base_font_size = pt.clamp(6.0, 36.0);
                    return true;
                }
                false
            }
            "line-height" => {
                if let Ok(v) = val.trim().parse::<f32>() {
                    theme.line_height = v.clamp(1.0, 3.0);
                    return true;
                }
                false
            }
            "color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.text_color = c;
                    return true;
                }
                false
            }
            "background-color" | "background" => {
                if let Some(c) = parse_css_color(val) {
                    theme.page_background = c;
                    return true;
                }
                false
            }
            _ => false,
        },
        ".frontmatter" | ".metadata" | "frontmatter" => {
            if prop == "display" {
                let v = val.to_lowercase();
                if v.contains("none") {
                    theme.metadata_mode = MetadataDisplayMode::Hidden;
                } else if v.contains("table") {
                    theme.metadata_mode = MetadataDisplayMode::PropertyTable;
                } else {
                    theme.metadata_mode = MetadataDisplayMode::HeaderCard;
                }
                return true;
            }
            false
        }
        ".toc" | "#toc" | "toc" => {
            if prop == "display" {
                theme.show_toc = !val.to_lowercase().contains("none");
                return true;
            }
            false
        }
        "h1" | "h2" | "h3" | "h4" => {
            if prop == "font-family" {
                if let Some(f) = parse_font_family(val) {
                    theme.heading_font = f;
                    return true;
                }
                return false;
            }
            let base = theme.base_font_size;
            let style = match sel {
                "h1" => &mut theme.h1_style,
                "h2" => &mut theme.h2_style,
                "h3" => &mut theme.h3_style,
                _ => &mut theme.h4_style,
            };
            match prop {
                "color" => {
                    if let Some(c) = parse_css_color(val) {
                        style.color = c;
                        return true;
                    }
                    false
                }
                "font-size" => {
                    if let Some(scale) = parse_scale(val, base) {
                        style.size_scale = scale.clamp(0.7, 4.0);
                        return true;
                    }
                    false
                }
                "border-bottom" => {
                    if val.trim().eq_ignore_ascii_case("none") {
                        style.underline = false;
                    } else {
                        style.underline = true;
                        if let Some(c) = extract_color_from_border(val) {
                            style.underline_color = c;
                        }
                    }
                    true
                }
                "border-left" => {
                    style.left_accent_bar = !val.trim().eq_ignore_ascii_case("none");
                    true
                }
                "page-break-before" | "break-before" => {
                    let v = val.trim().to_ascii_lowercase();
                    style.page_break_before =
                        matches!(v.as_str(), "always" | "page" | "left" | "right" | "recto" | "verso");
                    true
                }
                _ => false,
            }
        }
        "strong em" | "em strong" | "b i" | "i b" => {
            apply_emphasis_css(prop, val, theme.base_font_size, &mut theme.bold_italic_style)
        }
        "strong" | "b" => apply_emphasis_css(prop, val, theme.base_font_size, &mut theme.bold_style),
        "em" | "i" => apply_emphasis_css(prop, val, theme.base_font_size, &mut theme.italic_style),
        "mark" => apply_emphasis_css(prop, val, theme.base_font_size, &mut theme.highlight_style),
        "code" => {
            if prop == "font-family" {
                if let Some(f) = parse_font_family(val) {
                    theme.code_font = f;
                    return true;
                }
                return false;
            }
            apply_emphasis_css(prop, val, theme.base_font_size, &mut theme.inline_code_style)
        }
        "pre" | "pre code" => match prop {
            "background-color" | "background" => {
                if let Some(c) = parse_css_color(val) {
                    theme.code_block_bg = c;
                    return true;
                }
                false
            }
            "color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.code_block_text = c;
                    return true;
                }
                false
            }
            "border-color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.code_block_border = c;
                    return true;
                }
                false
            }
            _ => false,
        },
        "blockquote" => match prop {
            "border-left-color" | "border-color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.quote_bar_color = c;
                    return true;
                }
                false
            }
            "background-color" | "background" => {
                if let Some(c) = parse_css_color(val) {
                    theme.quote_bg_color = c;
                    return true;
                }
                false
            }
            "color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.quote_text_color = c;
                    return true;
                }
                false
            }
            _ => false,
        },
        "a" => match prop {
            "color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.link_color = c;
                    return true;
                }
                false
            }
            "text-decoration" => {
                theme.link_underline = val.to_lowercase().contains("underline");
                true
            }
            _ => false,
        },
        "th" => match prop {
            "background-color" | "background" => {
                if let Some(c) = parse_css_color(val) {
                    theme.table_header_bg = c;
                    return true;
                }
                false
            }
            "color" => {
                if let Some(c) = parse_css_color(val) {
                    theme.table_header_text = c;
                    return true;
                }
                false
            }
            _ => false,
        },
        _ => false,
    }
}

fn apply_emphasis_css(
    prop: &str,
    val: &str,
    base_pt: f32,
    style: &mut crate::config::theme::EmphasisStyle,
) -> bool {
    match prop {
        "color" => {
            if let Some(c) = parse_css_color(val) {
                style.color = c;
                return true;
            }
            false
        }
        "font-size" => {
            if let Some(scale) = parse_scale(val, base_pt) {
                style.size_scale = scale.clamp(0.6, 3.0);
                return true;
            }
            false
        }
        "background-color" | "background" => {
            if val.trim().eq_ignore_ascii_case("transparent")
                || val.trim().eq_ignore_ascii_case("none")
            {
                style.use_background = false;
                return true;
            }
            if let Some(c) = parse_css_color(val) {
                style.use_background = true;
                style.background_color = c;
                return true;
            }
            false
        }
        "text-decoration" => {
            style.underline = val.to_lowercase().contains("underline");
            true
        }
        _ => false,
    }
}

fn parse_font_family(val: &str) -> Option<String> {
    let first = val.split(',').next()?.trim();
    let trimmed = first.trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_length_mm(val: &str) -> Option<f32> {
    let v = val.trim().to_lowercase();
    if let Some(n) = v.strip_suffix("mm") {
        n.trim().parse::<f32>().ok()
    } else if let Some(n) = v.strip_suffix("cm") {
        n.trim().parse::<f32>().ok().map(|c| c * 10.0)
    } else if let Some(n) = v.strip_suffix("in") {
        n.trim().parse::<f32>().ok().map(|i| i * 25.4)
    } else if let Some(n) = v.strip_suffix("pt") {
        n.trim().parse::<f32>().ok().map(|p| p * 25.4 / 72.0)
    } else {
        v.parse::<f32>().ok()
    }
}

fn parse_font_size_pt(val: &str, base_pt: f32) -> Option<f32> {
    let v = val.trim().to_lowercase();
    if let Some(n) = v.strip_suffix("pt") {
        n.trim().parse::<f32>().ok()
    } else if let Some(n) = v.strip_suffix("px") {
        n.trim().parse::<f32>().ok().map(|px| px * 0.75)
    } else if let Some(n) = v.strip_suffix("rem") {
        n.trim().parse::<f32>().ok().map(|em| em * base_pt)
    } else if let Some(n) = v.strip_suffix("em") {
        n.trim().parse::<f32>().ok().map(|em| em * base_pt)
    } else if let Some(n) = v.strip_suffix('%') {
        n.trim().parse::<f32>().ok().map(|pct| base_pt * (pct / 100.0))
    } else {
        v.parse::<f32>().ok()
    }
}

fn parse_scale(val: &str, base_pt: f32) -> Option<f32> {
    let v = val.trim().to_lowercase();
    if let Some(n) = v.strip_suffix("rem") {
        n.trim().parse::<f32>().ok()
    } else if let Some(n) = v.strip_suffix("em") {
        n.trim().parse::<f32>().ok()
    } else if let Some(n) = v.strip_suffix('%') {
        n.trim().parse::<f32>().ok().map(|p| p / 100.0)
    } else if let Some(pt) = parse_font_size_pt(&v, base_pt) {
        Some(pt / base_pt.max(1.0))
    } else {
        None
    }
}

fn extract_color_from_border(val: &str) -> Option<[u8; 3]> {
    for token in val.split_whitespace() {
        if let Some(c) = parse_css_color(token) {
            return Some(c);
        }
    }
    None
}

pub fn parse_css_color(val: &str) -> Option<[u8; 3]> {
    let s = val.trim().to_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                return Some([r, g, b]);
            }
            6 | 8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some([r, g, b]);
            }
            _ => return None,
        }
    }
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
        let parts: Vec<u8> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<u8>().ok())
            .collect();
        if parts.len() == 3 {
            return Some([parts[0], parts[1], parts[2]]);
        }
    }
    match s.as_str() {
        "black" => Some([0, 0, 0]),
        "white" => Some([255, 255, 255]),
        "red" => Some([220, 38, 38]),
        "crimson" => Some([185, 28, 28]),
        "blue" => Some([37, 99, 235]),
        "navy" => Some([30, 58, 138]),
        "green" => Some([22, 163, 74]),
        "yellow" => Some([254, 240, 138]),
        "orange" => Some([234, 88, 12]),
        "purple" => Some([147, 51, 234]),
        "gray" | "grey" => Some([107, 114, 128]),
        _ => None,
    }
}
