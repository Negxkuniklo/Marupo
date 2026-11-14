use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataDisplayMode {
    Hidden,
    HeaderCard,
    PropertyTable,
}

impl Default for MetadataDisplayMode {
    fn default() -> Self {
        Self::HeaderCard
    }
}

impl MetadataDisplayMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Hidden => "非表示 (PDFに出力しない)",
            Self::HeaderCard => "ドキュメントヘッダー形式 (タイトル＋属性バッジ)",
            Self::PropertyTable => "プロパティテーブル形式 (全項目を表形式で表示)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperSize {
    A4,
    B5,
    Letter,
}

impl Default for PaperSize {
    fn default() -> Self {
        Self::A4
    }
}

impl PaperSize {
    pub fn label(&self) -> &'static str {
        match self {
            Self::A4 => "A4 (210 × 297 mm)",
            Self::B5 => "JIS B5 (182 × 257 mm)",
            Self::Letter => "US Letter (215.9 × 279.4 mm)",
        }
    }

    /// Returns (width_pt, height_pt) in portrait orientation (1 pt = 1/72 inch)
    pub fn dimensions_pt(&self, landscape: bool) -> (f32, f32) {
        let (w_mm, h_mm) = match self {
            Self::A4 => (210.0, 297.0),
            Self::B5 => (182.0, 257.0),
            Self::Letter => (215.9, 279.4),
        };
        let mm_to_pt = 72.0 / 25.4;
        let w = w_mm * mm_to_pt;
        let h = h_mm * mm_to_pt;
        if landscape {
            (h, w)
        } else {
            (w, h)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EmphasisStyle {
    pub color: [u8; 3],
    pub size_scale: f32,
    pub stroke_boost: f32,
    pub slant: f32,
    pub use_background: bool,
    pub background_color: [u8; 3],
    pub underline: bool,
}

impl Default for EmphasisStyle {
    fn default() -> Self {
        Self {
            color: [20, 24, 33],
            size_scale: 1.0,
            stroke_boost: 0.0,
            slant: 0.0,
            use_background: false,
            background_color: [255, 243, 163],
            underline: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadingStyle {
    pub color: [u8; 3],
    pub size_scale: f32,
    pub stroke_boost: f32,
    pub underline: bool,
    pub underline_color: [u8; 3],
    pub left_accent_bar: bool,
}

impl Default for HeadingStyle {
    fn default() -> Self {
        Self {
            color: [17, 24, 39],
            size_scale: 1.5,
            stroke_boost: 0.55,
            underline: false,
            underline_color: [209, 213, 219],
            left_accent_bar: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub theme_name: String,

    // Fonts
    pub body_font: String,
    pub heading_font: String,
    pub code_font: String,
    pub base_font_size: f32,
    pub line_height: f32,
    pub paragraph_spacing: f32,

    // Colors
    pub text_color: [u8; 3],
    pub page_background: [u8; 3],
    pub link_color: [u8; 3],
    pub link_underline: bool,

    // Metadata & TOC & Line Breaks
    pub metadata_mode: MetadataDisplayMode,
    pub show_toc: bool,
    pub toc_title: String,
    pub max_toc_level: u8,
    pub strict_line_breaks: bool,

    // Page Setup
    pub paper_size: PaperSize,
    pub landscape: bool,
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub margin_right_mm: f32,
    pub show_page_numbers: bool,

    // Emphasis Customization
    pub bold_style: EmphasisStyle,
    pub italic_style: EmphasisStyle,
    pub highlight_style: EmphasisStyle,
    pub inline_code_style: EmphasisStyle,

    // Headings
    pub h1_style: HeadingStyle,
    pub h2_style: HeadingStyle,
    pub h3_style: HeadingStyle,
    pub h4_style: HeadingStyle,

    // Blocks
    pub code_block_bg: [u8; 3],
    pub code_block_text: [u8; 3],
    pub code_block_border: [u8; 3],
    pub quote_bar_color: [u8; 3],
    pub quote_bg_color: [u8; 3],
    pub quote_text_color: [u8; 3],
    pub table_header_bg: [u8; 3],
    pub table_header_text: [u8; 3],
    pub table_stripe_bg: [u8; 3],
    pub table_border_color: [u8; 3],
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::preset_standard()
    }
}

impl ThemeConfig {
    pub fn preset_standard() -> Self {
        #[cfg(target_os = "windows")]
        let (body_font, heading_font, code_font) = (
            "Yu Gothic".to_string(),
            "Yu Gothic".to_string(),
            "Consolas".to_string(),
        );
        #[cfg(not(target_os = "windows"))]
        let (body_font, heading_font, code_font) = (
            "Noto Sans CJK JP".to_string(),
            "Noto Sans CJK JP".to_string(),
            "DejaVu Sans Mono".to_string(),
        );

        Self {
            theme_name: "Standard Clean (標準)".to_string(),
            body_font,
            heading_font,
            code_font,
            base_font_size: 10.5,
            line_height: 1.65,
            paragraph_spacing: 8.0,

            text_color: [31, 41, 55],
            page_background: [255, 255, 255],
            link_color: [37, 99, 235],
            link_underline: true,

            metadata_mode: MetadataDisplayMode::HeaderCard,
            show_toc: false,
            toc_title: "目次 / Table of Contents".to_string(),
            max_toc_level: 3,
            strict_line_breaks: false,

            paper_size: PaperSize::A4,
            landscape: false,
            margin_top_mm: 20.0,
            margin_bottom_mm: 20.0,
            margin_left_mm: 20.0,
            margin_right_mm: 20.0,
            show_page_numbers: true,

            bold_style: EmphasisStyle {
                color: [15, 23, 42],
                size_scale: 1.0,
                stroke_boost: 0.65,
                slant: 0.0,
                use_background: false,
                background_color: [254, 249, 195],
                underline: false,
            },
            italic_style: EmphasisStyle {
                color: [51, 65, 85],
                size_scale: 1.0,
                stroke_boost: 0.0,
                slant: 0.20,
                use_background: false,
                background_color: [241, 245, 249],
                underline: false,
            },
            highlight_style: EmphasisStyle {
                color: [17, 24, 39],
                size_scale: 1.0,
                stroke_boost: 0.35,
                slant: 0.0,
                use_background: true,
                background_color: [254, 240, 138],
                underline: false,
            },
            inline_code_style: EmphasisStyle {
                color: [190, 24, 93],
                size_scale: 0.92,
                stroke_boost: 0.1,
                slant: 0.0,
                use_background: true,
                background_color: [243, 244, 246],
                underline: false,
            },

            h1_style: HeadingStyle {
                color: [15, 23, 42],
                size_scale: 2.0,
                stroke_boost: 0.8,
                underline: true,
                underline_color: [59, 130, 246],
                left_accent_bar: false,
            },
            h2_style: HeadingStyle {
                color: [30, 41, 59],
                size_scale: 1.5,
                stroke_boost: 0.7,
                underline: true,
                underline_color: [203, 213, 225],
                left_accent_bar: true,
            },
            h3_style: HeadingStyle {
                color: [51, 65, 85],
                size_scale: 1.25,
                stroke_boost: 0.6,
                underline: false,
                underline_color: [226, 232, 240],
                left_accent_bar: false,
            },
            h4_style: HeadingStyle {
                color: [71, 85, 105],
                size_scale: 1.1,
                stroke_boost: 0.5,
                underline: false,
                underline_color: [226, 232, 240],
                left_accent_bar: false,
            },

            code_block_bg: [248, 250, 252],
            code_block_text: [30, 41, 59],
            code_block_border: [226, 232, 240],
            quote_bar_color: [100, 116, 139],
            quote_bg_color: [248, 250, 252],
            quote_text_color: [71, 85, 105],
            table_header_bg: [241, 245, 249],
            table_header_text: [15, 23, 42],
            table_stripe_bg: [249, 250, 251],
            table_border_color: [203, 213, 225],
        }
    }

    pub fn preset_academic() -> Self {
        let mut t = Self::preset_standard();
        t.theme_name = "Academic / 論文・レポート調 (明朝)".to_string();
        #[cfg(target_os = "windows")]
        {
            t.body_font = "Yu Mincho".to_string();
            t.heading_font = "Yu Gothic".to_string();
        }
        #[cfg(not(target_os = "windows"))]
        {
            t.body_font = "Noto Serif CJK JP".to_string();
            t.heading_font = "Noto Sans CJK JP".to_string();
        }
        t.base_font_size = 10.5;
        t.line_height = 1.75;
        t.metadata_mode = MetadataDisplayMode::HeaderCard;
        t.show_toc = true;
        t.h1_style.color = [0, 0, 0];
        t.h1_style.underline_color = [80, 80, 80];
        t.h2_style.color = [20, 20, 20];
        t.h2_style.left_accent_bar = false;
        t.bold_style.color = [0, 0, 0];
        t
    }

    pub fn preset_vivid_emphasis() -> Self {
        let mut t = Self::preset_standard();
        t.theme_name = "Vivid Tech / 強調カスタム重視".to_string();
        #[cfg(target_os = "windows")]
        {
            t.body_font = "Meiryo".to_string();
            t.heading_font = "Meiryo".to_string();
        }
        #[cfg(not(target_os = "windows"))]
        {
            t.body_font = "Noto Sans CJK JP".to_string();
            t.heading_font = "Noto Sans CJK JP".to_string();
        }
        t.metadata_mode = MetadataDisplayMode::PropertyTable;
        t.bold_style.color = [185, 28, 28]; // Deep crimson bold
        t.bold_style.size_scale = 1.12; // Slightly larger bold!
        t.bold_style.underline = true;
        t.italic_style.color = [12, 74, 110];
        t.highlight_style.background_color = [253, 224, 71];
        t.highlight_style.size_scale = 1.05;
        t.h1_style.color = [30, 58, 138];
        t.h1_style.underline_color = [37, 99, 235];
        t.h2_style.color = [29, 78, 216];
        t.h2_style.left_accent_bar = true;
        t
    }

    pub fn preset_warm_editorial() -> Self {
        let mut t = Self::preset_standard();
        t.theme_name = "Warm Editorial / 書籍・Zine風".to_string();
        t.page_background = [253, 251, 247];
        t.text_color = [45, 38, 34];
        #[cfg(target_os = "windows")]
        {
            t.body_font = "Yu Mincho".to_string();
            t.heading_font = "Yu Mincho".to_string();
        }
        #[cfg(not(target_os = "windows"))]
        {
            t.body_font = "Noto Serif CJK JP".to_string();
            t.heading_font = "Noto Serif CJK JP".to_string();
        }
        t.bold_style.color = [154, 52, 18];
        t.bold_style.size_scale = 1.05;
        t.highlight_style.background_color = [254, 215, 170];
        t.h1_style.color = [124, 45, 18];
        t.h1_style.underline_color = [194, 65, 12];
        t.h2_style.color = [154, 52, 18];
        t.quote_bar_color = [180, 83, 9];
        t.quote_bg_color = [254, 243, 199];
        t
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserDefaultsConfig {
    pub default_theme: ThemeConfig,
    pub ui_dark_mode: bool,
    pub default_preview_zoom: f32,
    pub open_sample_on_startup: bool,
}

impl Default for UserDefaultsConfig {
    fn default() -> Self {
        Self {
            default_theme: ThemeConfig::preset_standard(),
            ui_dark_mode: true,
            default_preview_zoom: 1.20,
            open_sample_on_startup: true,
        }
    }
}

impl UserDefaultsConfig {
    pub fn config_file_path() -> std::path::PathBuf {
        #[cfg(target_os = "windows")]
        {
            std::path::PathBuf::from("user_defaults.json")
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                if !xdg.trim().is_empty() {
                    return std::path::PathBuf::from(xdg)
                        .join("marupo")
                        .join("user_defaults.json");
                }
            }
            if let Ok(home) = std::env::var("HOME") {
                if !home.trim().is_empty() {
                    return std::path::PathBuf::from(home)
                        .join(".config")
                        .join("marupo")
                        .join("user_defaults.json");
                }
            }
            std::path::PathBuf::from("user_defaults.json")
        }
    }

    pub fn load_or_default() -> Self {
        let path = Self::config_file_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<Self>(&content) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}
