use crate::config::{
    apply_css_to_theme, theme_to_css, EmphasisStyle, HeadingStyle, MetadataDisplayMode, PaperSize,
    ThemeConfig, UserDefaultsConfig,
};
use crate::markdown::{parse_markdown, ParsedDocument};
use crate::renderer::{
    export_html_string, export_pdf_bytes, layout_document, render_page_preview, DocumentLayout,
    FontSystem,
};
use egui::{
    Align, Color32, Frame, Layout, Margin, RichText, Rounding, ScrollArea, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeftTab {
    StyleConfig,
    MarkdownEditor,
    ProCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProSubTab {
    Json,
    Css,
}

pub struct DocumentTab {
    pub id: u64,
    pub title: String,
    pub markdown_text: String,
    pub path: Option<PathBuf>,
    pub parsed_doc: ParsedDocument,
    pub current_page: usize,
    pub dirty: bool,
}

impl DocumentTab {
    pub fn new_with_content(
        id: u64,
        title: String,
        markdown_text: String,
        path: Option<PathBuf>,
    ) -> Self {
        let parsed_doc = parse_markdown(&markdown_text);
        Self {
            id,
            title,
            markdown_text,
            path,
            parsed_doc,
            current_page: 0,
            dirty: false,
        }
    }
}

pub struct MarkdownToPdfApp {
    tabs: Vec<DocumentTab>,
    active_tab_idx: usize,
    next_tab_id: u64,

    user_defaults: UserDefaultsConfig,
    ui_dark_mode: bool,

    theme: ThemeConfig,
    fonts: FontSystem,
    font_search: String,

    left_tab: LeftTab,
    pro_sub_tab: ProSubTab,
    json_buffer: String,
    css_buffer: String,
    pro_status_msg: Option<(String, bool)>,

    // Accordion open states (all false by default on startup!)
    sec_open_meta: bool,
    sec_open_font: bool,
    sec_open_emp: bool,
    sec_open_heading: bool,
    sec_open_page: bool,
    sec_open_defaults: bool,

    doc_layout: DocumentLayout,

    preview_zoom: f32,
    preview_textures: Vec<TextureHandle>,
    needs_relayout: bool,
    needs_rerender_preview: bool,

    status_message: String,
}

impl MarkdownToPdfApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_files(cc, Vec::new())
    }

    pub fn new_with_files(
        cc: &eframe::CreationContext<'_>,
        initial_files: Vec<PathBuf>,
    ) -> Self {
        let mut fonts = FontSystem::new();

        if let Some((font_bytes, face_index)) = fonts.find_egui_japanese_font() {
            let mut egui_fonts = egui::FontDefinitions::default();
            let mut font_data = egui::FontData::from_owned(font_bytes);
            font_data.index = face_index;
            egui_fonts
                .font_data
                .insert("jp_sys_font".to_owned(), font_data);
            egui_fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "jp_sys_font".to_owned());
            egui_fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("jp_sys_font".to_owned());
            cc.egui_ctx.set_fonts(egui_fonts);
        }

        let user_defaults = UserDefaultsConfig::load_or_default();
        let ui_dark_mode = user_defaults.ui_dark_mode;
        apply_modern_ui_style(&cc.egui_ctx, ui_dark_mode);

        let initial_md = if user_defaults.open_sample_on_startup {
            DEFAULT_SAMPLE_MD.to_string()
        } else {
            "# 新規ドキュメント\n\nここにMarkdownを入力するか、ファイルをドロップしてください。\n"
                .to_string()
        };
        let initial_title = if user_defaults.open_sample_on_startup {
            "sample.md".to_string()
        } else {
            "Untitled-1.md".to_string()
        };

        let first_tab = DocumentTab::new_with_content(1, initial_title, initial_md, None);
        let theme = user_defaults.default_theme.clone();
        let json_buffer = serde_json::to_string_pretty(&theme).unwrap_or_default();
        let css_buffer = theme_to_css(&theme);

        let doc_layout = layout_document(&first_tab.parsed_doc, &theme, &mut fonts);
        let preview_zoom = user_defaults.default_preview_zoom.clamp(0.4, 2.5);

        let mut app = Self {
            tabs: vec![first_tab],
            active_tab_idx: 0,
            next_tab_id: 2,
            user_defaults,
            ui_dark_mode,
            theme,
            fonts,
            font_search: String::new(),
            left_tab: LeftTab::StyleConfig,
            pro_sub_tab: ProSubTab::Json,
            json_buffer,
            css_buffer,
            pro_status_msg: None,
            sec_open_meta: false,
            sec_open_font: false,
            sec_open_emp: false,
            sec_open_heading: false,
            sec_open_page: false,
            sec_open_defaults: false,
            doc_layout,
            preview_zoom,
            preview_textures: Vec::new(),
            needs_relayout: false,
            needs_rerender_preview: true,
            status_message: "準備完了 — ファイルをドロップして複数タブで開けます".to_string(),
        };

        if !initial_files.is_empty() {
            let mut opened_md = false;
            let mut first_arg_md_replaced = false;
            for path in initial_files {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "json" || ext == "css" {
                    app.load_theme_from_path(&path);
                } else if let Ok(content) = std::fs::read_to_string(&path) {
                    if !first_arg_md_replaced {
                        app.tabs.clear();
                        app.next_tab_id = 1;
                        first_arg_md_replaced = true;
                    }
                    app.open_markdown_path(path, content);
                    opened_md = true;
                }
            }
            if opened_md {
                app.active_tab_idx = 0;
                app.needs_relayout = true;
            }
        }

        app
    }

    fn active_tab(&self) -> &DocumentTab {
        &self.tabs[self.active_tab_idx]
    }

    fn active_tab_mut(&mut self) -> &mut DocumentTab {
        &mut self.tabs[self.active_tab_idx]
    }

    fn sync_pro_buffers(&mut self) {
        self.json_buffer = serde_json::to_string_pretty(&self.theme).unwrap_or_default();
        self.css_buffer = theme_to_css(&self.theme);
    }

    fn set_all_sections_open(&mut self, open: bool) {
        self.sec_open_meta = open;
        self.sec_open_font = open;
        self.sec_open_emp = open;
        self.sec_open_heading = open;
        self.sec_open_page = open;
        self.sec_open_defaults = open;
    }

    fn reset_to_user_defaults(&mut self) {
        self.theme = self.user_defaults.default_theme.clone();
        self.preview_zoom = self.user_defaults.default_preview_zoom.clamp(0.4, 2.5);
        self.sync_pro_buffers();
        self.needs_relayout = true;
        self.status_message = "↺ 初期設定にリセットしました".to_string();
    }

    fn reset_to_factory_defaults(&mut self) {
        self.theme = ThemeConfig::preset_standard();
        self.sync_pro_buffers();
        self.needs_relayout = true;
        self.status_message = "出荷時の標準設定に戻しました".to_string();
    }

    fn save_current_as_user_defaults(&mut self) {
        self.user_defaults.default_theme = self.theme.clone();
        self.user_defaults.ui_dark_mode = self.ui_dark_mode;
        self.user_defaults.default_preview_zoom = self.preview_zoom;
        match self.user_defaults.save() {
            Ok(_) => {
                self.status_message = "★ 現在の設定を「初期設定」として登録しました".to_string();
            }
            Err(e) => {
                self.status_message = format!("保存エラー: {}", e);
            }
        }
    }

    fn refresh_if_needed(&mut self, ctx: &egui::Context) {
        if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len().saturating_sub(1);
        }

        if self.needs_relayout {
            let parsed = &self.tabs[self.active_tab_idx].parsed_doc;
            self.doc_layout = layout_document(parsed, &self.theme, &mut self.fonts);
            let page_count = self.doc_layout.pages.len();
            let cur_page = &mut self.tabs[self.active_tab_idx].current_page;
            if *cur_page >= page_count {
                *cur_page = page_count.saturating_sub(1);
            }
            self.needs_relayout = false;
            self.needs_rerender_preview = true;
        }

        if self.needs_rerender_preview {
            let render_scale = (self.preview_zoom * 1.4).clamp(0.5, 3.0);
            let mut textures = Vec::with_capacity(self.doc_layout.pages.len());
            for (idx, page) in self.doc_layout.pages.iter().enumerate() {
                if let Some(img) = render_page_preview(page, &self.fonts, render_scale) {
                    let tex = ctx.load_texture(
                        format!("pdf_page_preview_{}", idx),
                        img,
                        TextureOptions::LINEAR,
                    );
                    textures.push(tex);
                }
            }
            self.preview_textures = textures;
            self.needs_rerender_preview = false;
        }
    }

    fn add_new_empty_tab(&mut self) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let title = format!("Untitled-{}.md", id);
        let content = format!(
            "---\ntitle: 新規ドキュメント #{}\ndate: 2026-10-30\n---\n\n# 見出しタイトル\n\nここに本文を入力してください。\n",
            id
        );
        let tab = DocumentTab::new_with_content(id, title.clone(), content, None);
        self.tabs.push(tab);
        self.active_tab_idx = self.tabs.len() - 1;
        self.needs_relayout = true;
        self.status_message = format!("新規タブ「{}」を作成しました", title);
    }

    fn open_markdown_path(&mut self, path: PathBuf, content: String) {
        if let Some(existing_idx) = self
            .tabs
            .iter()
            .position(|t| t.path.as_ref() == Some(&path))
        {
            self.tabs[existing_idx].markdown_text = content;
            self.tabs[existing_idx].parsed_doc =
                parse_markdown(&self.tabs[existing_idx].markdown_text);
            self.tabs[existing_idx].dirty = false;
            self.active_tab_idx = existing_idx;
            self.needs_relayout = true;
            self.status_message = format!("タブを更新: {}", path.display());
            return;
        }

        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.md")
            .to_string();

        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let new_tab = DocumentTab::new_with_content(id, title, content, Some(path.clone()));
        self.tabs.push(new_tab);
        self.active_tab_idx = self.tabs.len() - 1;
        self.needs_relayout = true;
        self.status_message = format!("開きました: {}", path.display());
    }

    fn open_markdown_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "markdown", "txt"])
            .pick_files()
        {
            for path in paths {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    self.open_markdown_path(path, content);
                }
            }
        }
    }

    fn save_markdown_dialog(&mut self) {
        let default_name = self
            .active_tab()
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(&self.active_tab().title)
            .to_string();

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("Markdown", &["md"])
            .save_file()
        {
            let content = self.active_tab().markdown_text.clone();
            if std::fs::write(&path, &content).is_ok() {
                let file_title = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("document.md")
                    .to_string();
                let tab = self.active_tab_mut();
                tab.path = Some(path.clone());
                tab.title = file_title;
                tab.dirty = false;
                self.status_message = format!("保存完了: {}", path.display());
            }
        }
    }

    fn export_pdf_dialog(&mut self) {
        let default_pdf_name = self
            .active_tab()
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map(|s| format!("{}.pdf", s))
            .unwrap_or_else(|| {
                let stem = self
                    .active_tab()
                    .title
                    .strip_suffix(".md")
                    .unwrap_or(&self.active_tab().title);
                format!("{}.pdf", stem)
            });

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_pdf_name)
            .add_filter("PDF Document", &["pdf"])
            .save_file()
        {
            let bytes = export_pdf_bytes(&self.doc_layout, &self.fonts);
            if std::fs::write(&path, &bytes).is_ok() {
                self.status_message = format!(
                    "PDF出力完了 ({}p, {:.1} KB): {}",
                    self.doc_layout.pages.len(),
                    bytes.len() as f32 / 1024.0,
                    path.display()
                );
            }
        }
    }

    fn export_html_dialog(&mut self) {
        let default_html_name = self
            .active_tab()
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map(|s| format!("{}.html", s))
            .unwrap_or_else(|| {
                let stem = self
                    .active_tab()
                    .title
                    .strip_suffix(".md")
                    .unwrap_or(&self.active_tab().title);
                format!("{}.html", stem)
            });

        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_html_name)
            .add_filter("HTML Document", &["html", "htm"])
            .save_file()
        {
            let html_str = export_html_string(&self.active_tab().parsed_doc, &self.theme);
            if std::fs::write(&path, html_str.as_bytes()).is_ok() {
                self.status_message = format!(
                    "HTML出力完了 ({:.1} KB): {}",
                    html_str.len() as f32 / 1024.0,
                    path.display()
                );
            }
        }
    }

    fn import_theme_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Theme (JSON / CSS)", &["json", "css"])
            .pick_file()
        {
            self.load_theme_from_path(&path);
        }
    }

    fn load_theme_from_path(&mut self, path: &PathBuf) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "css" {
            if let Ok(n) = apply_css_to_theme(&content, &mut self.theme) {
                self.css_buffer = content;
                self.json_buffer = serde_json::to_string_pretty(&self.theme).unwrap_or_default();
                self.needs_relayout = true;
                self.status_message =
                    format!("CSS適用 ({}項目): {}", n, path.display());
            }
        } else if let Ok(new_theme) = serde_json::from_str::<ThemeConfig>(&content) {
            self.theme = new_theme;
            self.sync_pro_buffers();
            self.needs_relayout = true;
            self.status_message = format!("JSONテーマ読込: {}", path.display());
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }

        let mut opened_md_count = 0usize;
        for file in dropped {
            if let Some(path) = file.path {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "json" || ext == "css" {
                    self.load_theme_from_path(&path);
                } else if let Ok(content) = std::fs::read_to_string(&path) {
                    self.open_markdown_path(path, content);
                    opened_md_count += 1;
                }
            } else if let Some(bytes) = file.bytes {
                if let Ok(content) = std::str::from_utf8(&bytes) {
                    let id = self.next_tab_id;
                    self.next_tab_id += 1;
                    let name = if file.name.is_empty() {
                        format!("Dropped-{}.md", id)
                    } else {
                        file.name.clone()
                    };
                    let tab = DocumentTab::new_with_content(id, name, content.to_string(), None);
                    self.tabs.push(tab);
                    self.active_tab_idx = self.tabs.len() - 1;
                    self.needs_relayout = true;
                    opened_md_count += 1;
                }
            }
        }

        if opened_md_count > 1 {
            self.status_message = format!("{} 個のファイルをタブで開きました", opened_md_count);
        }
    }

    fn render_drag_drop_overlay(&self, ctx: &egui::Context) {
        let hovered = ctx.input(|i| i.raw.hovered_files.clone());
        if hovered.is_empty() {
            return;
        }

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("dd_overlay_layer"),
        ));
        let screen_rect = ctx.screen_rect();
        painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(185));
        painter.rect_stroke(
            screen_rect.shrink(24.0),
            14.0,
            Stroke::new(2.5_f32, Color32::from_rgb(59, 130, 246)),
        );

        let mut text =
            "📥 ドロップして開く\n\nMarkdown (.md) ➔ 新規タブで開く\nJSON / CSS ➔ テーマを適用"
                .to_string();
        for h in &hovered {
            if let Some(p) = &h.path {
                if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                    text.push_str(&format!("\n\n📄 {}", fname));
                }
            }
        }

        painter.text(
            screen_rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(19.0),
            Color32::WHITE,
        );
    }
}

fn apply_modern_ui_style(ctx: &egui::Context, dark_mode: bool) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.5);
    style.spacing.interact_size = Vec2::new(40.0, 26.0);

    let mut visuals = if dark_mode {
        let mut v = egui::Visuals::dark();
        v.panel_fill = Color32::from_rgb(20, 23, 30);
        v.window_fill = Color32::from_rgb(26, 30, 39);
        v.extreme_bg_color = Color32::from_rgb(13, 15, 20);
        v.faint_bg_color = Color32::from_rgb(29, 34, 44);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(28, 33, 43);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(45, 53, 68));
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(215, 222, 234));
        v.widgets.inactive.bg_fill = Color32::from_rgb(35, 41, 54);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(32, 37, 49);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(52, 61, 78));
        v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(226, 232, 240));
        v.widgets.hovered.bg_fill = Color32::from_rgb(47, 56, 74);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(47, 56, 74);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(96, 165, 250));
        v.widgets.active.bg_fill = Color32::from_rgb(37, 99, 235);
        v.selection.bg_fill = Color32::from_rgb(37, 99, 235);
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v
    } else {
        let mut v = egui::Visuals::light();
        v.panel_fill = Color32::from_rgb(248, 250, 252);
        v.window_fill = Color32::WHITE;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(241, 245, 249);
        v.widgets.inactive.bg_fill = Color32::from_rgb(226, 232, 240);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(236, 240, 246);
        v.selection.bg_fill = Color32::from_rgb(37, 99, 235);
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v
    };

    let r6 = Rounding::same(6.0);
    visuals.widgets.noninteractive.rounding = r6;
    visuals.widgets.inactive.rounding = r6;
    visuals.widgets.hovered.rounding = r6;
    visuals.widgets.active.rounding = r6;
    visuals.widgets.open.rounding = r6;

    style.visuals = visuals;
    ctx.set_style(style);
}

impl eframe::App for MarkdownToPdfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);
        self.refresh_if_needed(ctx);

        // 1. Sleek Top Header Bar
        egui::TopBottomPanel::top("top_toolbar")
            .exact_height(48.0)
            .frame(
                Frame::side_top_panel(&ctx.style())
                    .inner_margin(Margin::symmetric(14.0, 7.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Brand title
                    let brand_col = if self.ui_dark_mode {
                        Color32::from_rgb(96, 165, 250)
                    } else {
                        Color32::from_rgb(37, 99, 235)
                    };
                    ui.label(
                        RichText::new("◆ Marupo")
                            .size(16.5)
                            .strong()
                            .color(brand_col),
                    );
                    ui.add_space(6.0);

                    if ui
                        .button("📂 開く")
                        .on_hover_text("Markdownファイルを開きます（複数選択・D&D対応）")
                        .clicked()
                    {
                        self.open_markdown_dialog();
                    }
                    if ui
                        .button("💾 保存")
                        .on_hover_text("現在のMarkdownを保存します")
                        .clicked()
                    {
                        self.save_markdown_dialog();
                    }

                    ui.separator();

                    // Theme Preset Pull-down Menu
                    ui.label(
                        RichText::new("テーマ:")
                            .size(12.0)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                    let presets = [
                        (
                            "標準 (Standard)",
                            "Standard Clean (標準)",
                            ThemeConfig::preset_standard(),
                        ),
                        (
                            "論文・明朝 (Academic)",
                            "Academic / 論文・レポート調 (明朝)",
                            ThemeConfig::preset_academic(),
                        ),
                        (
                            "強調重視 (Vivid Tech)",
                            "Vivid Tech / 強調カスタム重視",
                            ThemeConfig::preset_vivid_emphasis(),
                        ),
                        (
                            "書籍風 (Editorial)",
                            "Warm Editorial / 書籍・Zine風",
                            ThemeConfig::preset_warm_editorial(),
                        ),
                    ];
                    let current_preset_label = presets
                        .iter()
                        .find(|(_, full_name, _)| self.theme.theme_name == *full_name)
                        .map(|(short_label, _, _)| *short_label)
                        .unwrap_or(self.theme.theme_name.as_str());

                    egui::ComboBox::from_id_salt("top_theme_preset_combo")
                        .selected_text(current_preset_label)
                        .width(165.0)
                        .show_ui(ui, |ui| {
                            for (short_label, full_name, preset) in presets {
                                let selected = self.theme.theme_name == full_name;
                                if ui
                                    .selectable_label(selected, short_label)
                                    .on_hover_text(full_name)
                                    .clicked()
                                {
                                    self.theme = preset;
                                    self.sync_pro_buffers();
                                    self.needs_relayout = true;
                                }
                            }
                        });

                    if ui
                        .button("📥 読込")
                        .on_hover_text("外部のJSONテーマまたはCSSスタイルシートを読み込みます")
                        .clicked()
                    {
                        self.import_theme_file_dialog();
                    }

                    // Right-aligned Export & UI Theme buttons
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let pdf_btn = egui::Button::new(
                            RichText::new("📄 PDF出力")
                                .strong()
                                .size(13.5)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(37, 99, 235))
                        .min_size(Vec2::new(104.0, 30.0));

                        if ui
                            .add(pdf_btn)
                            .on_hover_text("現在のプレビュー通りにPDFを出力・保存します")
                            .clicked()
                        {
                            self.export_pdf_dialog();
                        }

                        let html_btn = egui::Button::new(
                            RichText::new("🌐 HTML出力")
                                .strong()
                                .size(13.5)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(16, 185, 129))
                        .min_size(Vec2::new(106.0, 30.0));

                        if ui
                            .add(html_btn)
                            .on_hover_text("プレビューと同じデザインの単一HTMLファイルを出力します")
                            .clicked()
                        {
                            self.export_html_dialog();
                        }

                        let mode_icon = if self.ui_dark_mode { "🌙" } else { "☀" };
                        if ui
                            .button(mode_icon)
                            .on_hover_text("UIのダークモード / ライトモード切替")
                            .clicked()
                        {
                            self.ui_dark_mode = !self.ui_dark_mode;
                            apply_modern_ui_style(ctx, self.ui_dark_mode);
                        }
                    });
                });
            });

        // 2. Modern Document Tab Bar
        let tab_bar_bg = if self.ui_dark_mode {
            Color32::from_rgb(15, 17, 23)
        } else {
            Color32::from_rgb(226, 232, 240)
        };
        egui::TopBottomPanel::top("document_tab_bar")
            .exact_height(36.0)
            .frame(
                Frame::none()
                    .fill(tab_bar_bg)
                    .inner_margin(Margin::symmetric(12.0, 4.0)),
            )
            .show(ctx, |ui| {
                ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            let mut tab_to_select: Option<usize> = None;
                            let mut tab_to_close: Option<usize> = None;
                            let total_tabs = self.tabs.len();

                            for (idx, tab) in self.tabs.iter().enumerate() {
                                let is_active = idx == self.active_tab_idx;
                                let dirty_dot = if tab.dirty { " ●" } else { "" };
                                let label_str = format!("{}{}", tab.title, dirty_dot);

                                let (bg_fill, border_col, text_col) = if is_active {
                                    if self.ui_dark_mode {
                                        (
                                            Color32::from_rgb(30, 36, 48),
                                            Color32::from_rgb(59, 130, 246),
                                            Color32::WHITE,
                                        )
                                    } else {
                                        (
                                            Color32::WHITE,
                                            Color32::from_rgb(37, 99, 235),
                                            Color32::from_rgb(15, 23, 42),
                                        )
                                    }
                                } else if self.ui_dark_mode {
                                    (
                                        Color32::from_rgb(21, 24, 32),
                                        Color32::from_rgb(38, 44, 56),
                                        Color32::from_rgb(148, 163, 184),
                                    )
                                } else {
                                    (
                                        Color32::from_rgb(241, 245, 249),
                                        Color32::from_rgb(203, 213, 225),
                                        Color32::from_rgb(100, 116, 139),
                                    )
                                };

                                Frame::none()
                                    .fill(bg_fill)
                                    .stroke(Stroke::new(1.0_f32, border_col))
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(Margin::symmetric(10.0, 3.0))
                                    .show(ui, |ui| {
                                        let hover_path = tab
                                            .path
                                            .as_ref()
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|| "未保存のドキュメント".to_string());
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    RichText::new(label_str)
                                                        .size(12.5)
                                                        .color(text_col)
                                                        .strong(),
                                                )
                                                .frame(false),
                                            )
                                            .on_hover_text(hover_path)
                                            .clicked()
                                        {
                                            tab_to_select = Some(idx);
                                        }

                                        if total_tabs > 1 {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("×")
                                                            .size(12.5)
                                                            .color(text_col),
                                                    )
                                                    .frame(false),
                                                )
                                                .on_hover_text("タブを閉じる")
                                                .clicked()
                                            {
                                                tab_to_close = Some(idx);
                                            }
                                        }
                                    });
                            }

                            if let Some(sel) = tab_to_select {
                                if sel != self.active_tab_idx {
                                    self.active_tab_idx = sel;
                                    self.needs_relayout = true;
                                }
                            }

                            if let Some(close_idx) = tab_to_close {
                                if self.tabs.len() > 1 {
                                    self.tabs.remove(close_idx);
                                    if self.active_tab_idx >= self.tabs.len() {
                                        self.active_tab_idx = self.tabs.len() - 1;
                                    }
                                    self.needs_relayout = true;
                                }
                            }

                            if ui
                                .button("＋")
                                .on_hover_text("新しい空のタブを作成")
                                .clicked()
                            {
                                self.add_new_empty_tab();
                            }
                            if ui
                                .button("サンプル追加")
                                .on_hover_text("サンプルMarkdownを新しいタブで開く")
                                .clicked()
                            {
                                let id = self.next_tab_id;
                                self.next_tab_id += 1;
                                let tab = DocumentTab::new_with_content(
                                    id,
                                    format!("sample-{}.md", id),
                                    DEFAULT_SAMPLE_MD.to_string(),
                                    None,
                                );
                                self.tabs.push(tab);
                                self.active_tab_idx = self.tabs.len() - 1;
                                self.needs_relayout = true;
                            }
                        });
                    });
            });

        // 3. Minimal Bottom Status Bar
        egui::TopBottomPanel::bottom("bottom_status")
            .exact_height(24.0)
            .frame(
                Frame::side_top_panel(&ctx.style())
                    .inner_margin(Margin::symmetric(12.0, 3.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(&self.status_message)
                            .size(11.5)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} タブ  |  {} ページ  |  {} フォント",
                                self.tabs.len(),
                                self.doc_layout.pages.len(),
                                self.fonts.families().len()
                            ))
                            .size(11.0)
                            .color(Color32::from_rgb(100, 116, 139)),
                        );
                    });
                });
            });

        // 4. Left Inspector & Editor Panel
        egui::SidePanel::left("left_inspector_panel_v2")
            .default_width(420.0)
            .width_range(340.0..=580.0)
            .resizable(true)
            .frame(
                Frame::side_top_panel(&ctx.style())
                    .inner_margin(Margin::symmetric(12.0, 10.0)),
            )
            .show(ctx, |ui| {
                let panel_w = ui.available_width();
                ui.set_max_width(panel_w);

                // Full-width 3-way Segmented Mode Switcher (overflow-proof via ui.columns)
                let seg_bg = if self.ui_dark_mode {
                    Color32::from_rgb(13, 15, 20)
                } else {
                    Color32::from_rgb(226, 232, 240)
                };
                Frame::none()
                    .fill(seg_bg)
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(3.0))
                    .show(ui, |ui| {
                        ui.set_max_width((panel_w - 6.0).max(120.0));
                        let modes = [
                            (LeftTab::StyleConfig, "🎨 デザイン"),
                            (LeftTab::MarkdownEditor, "📝 エディタ"),
                            (LeftTab::ProCode, "{ } JSON / CSS"),
                        ];
                        ui.columns(3, |cols| {
                            for (i, (tab_val, label)) in modes.into_iter().enumerate() {
                                let selected = self.left_tab == tab_val;
                                let btn = egui::Button::new(
                                    RichText::new(label).size(12.5).strong().color(if selected {
                                        Color32::WHITE
                                    } else if self.ui_dark_mode {
                                        Color32::from_rgb(148, 163, 184)
                                    } else {
                                        Color32::from_rgb(71, 85, 105)
                                    }),
                                )
                                .fill(if selected {
                                    Color32::from_rgb(37, 99, 235)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .rounding(Rounding::same(6.0));

                                let col_w = cols[i].available_width();
                                if cols[i].add_sized(Vec2::new(col_w, 28.0), btn).clicked() {
                                    self.left_tab = tab_val;
                                }
                            }
                        });
                    });

                ui.add_space(6.0);

                match self.left_tab {
                    LeftTab::StyleConfig => self.ui_style_config_tab(ui, ctx),
                    LeftTab::MarkdownEditor => self.ui_markdown_editor_tab(ui),
                    LeftTab::ProCode => self.ui_pro_code_tab(ui),
                }
            });

        // 5. Right Live PDF Preview Panel
        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).inner_margin(Margin::same(0.0)))
            .show(ctx, |ui| {
                self.ui_preview_panel(ui);
            });

        self.render_drag_drop_overlay(ctx);
    }
}

impl MarkdownToPdfApp {
    fn ui_style_config_tab(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut changed = false;

        // Compact Top Action Bar (wraps cleanly when panel is narrow, never overlaps)
        ui.horizontal_wrapped(|ui| {
            if ui
                .button("↺ リセット")
                .on_hover_text("保存した初期設定に戻します")
                .clicked()
            {
                self.reset_to_user_defaults();
            }
            if ui
                .button("★ 初期値に保存")
                .on_hover_text("現在のデザイン設定を起動時の初期設定として保存します")
                .clicked()
            {
                self.save_current_as_user_defaults();
            }
            if ui
                .button("出荷時")
                .on_hover_text("工場出荷時の標準スタイルに戻します")
                .clicked()
            {
                self.reset_to_factory_defaults();
            }

            let any_open = self.sec_open_meta
                || self.sec_open_font
                || self.sec_open_emp
                || self.sec_open_heading
                || self.sec_open_page
                || self.sec_open_defaults;
            let toggle_label = if any_open {
                "▲ すべて閉じる"
            } else {
                "▼ すべて開く"
            };
            if ui.button(toggle_label).clicked() {
                self.set_all_sections_open(!any_open);
            }
        });

        ui.add_space(4.0);

        let dark = self.ui_dark_mode;

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let scroll_w = ui.available_width();
                ui.set_max_width(scroll_w);

                // Card 1: Metadata, TOC & Line Breaks
                let meta_badge = format!(
                    "{} · TOC {} · {}",
                    match self.theme.metadata_mode {
                        MetadataDisplayMode::Hidden => "非表示",
                        MetadataDisplayMode::HeaderCard => "ヘッダー",
                        MetadataDisplayMode::PropertyTable => "テーブル",
                    },
                    if self.theme.show_toc { "ON" } else { "OFF" },
                    if self.theme.strict_line_breaks {
                        "厳密改行"
                    } else {
                        "通常改行"
                    }
                );
                let mut open_meta = self.sec_open_meta;
                inspector_card(
                    ui,
                    dark,
                    "📌 メタデータ・目次・改行",
                    &meta_badge,
                    &mut open_meta,
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "プロパティ表示:");
                            for (mode, short) in [
                                (MetadataDisplayMode::Hidden, "非表示"),
                                (MetadataDisplayMode::HeaderCard, "ヘッダー"),
                                (MetadataDisplayMode::PropertyTable, "テーブル"),
                            ] {
                                if ui
                                    .selectable_label(self.theme.metadata_mode == mode, short)
                                    .clicked()
                                {
                                    self.theme.metadata_mode = mode;
                                    changed = true;
                                }
                            }
                        });

                        ui.separator();

                        ui.horizontal_wrapped(|ui| {
                            changed |= ui
                                .checkbox(&mut self.theme.show_toc, "目次 (TOC) を自動挿入")
                                .changed();
                            if self.theme.show_toc {
                                ui.add_space(4.0);
                                for lvl in 1..=4 {
                                    if ui
                                        .selectable_label(
                                            self.theme.max_toc_level == lvl,
                                            format!("H{}", lvl),
                                        )
                                        .clicked()
                                    {
                                        self.theme.max_toc_level = lvl;
                                        changed = true;
                                    }
                                }
                            }
                        });

                        ui.separator();

                        ui.horizontal_wrapped(|ui| {
                            changed |= ui
                                .checkbox(
                                    &mut self.theme.strict_line_breaks,
                                    "厳密な改行にする（行末スペース2つで改行）",
                                )
                                .on_hover_text(
                                    "OFF（初期設定）: 通常の改行（Enter 1回）をそのまま改行として反映します\nON: CommonMark標準どおり、行末に半角スペース2つがある時だけ改行します",
                                )
                                .changed();
                        });

                        ui.separator();

                        ui.horizontal_wrapped(|ui| {
                            changed |= ui
                                .checkbox(
                                    &mut self.theme.h2_style.page_break_before,
                                    "見出し2で改ページする",
                                )
                                .on_hover_text(
                                    "ONにすると、見出し2 (## H2) の直前で新しいページに切り替えます",
                                )
                                .changed();
                        });
                    },
                );
                self.sec_open_meta = open_meta;

                // Card 2: Fonts & Typography
                let font_badge = format!(
                    "{} · {:.1}pt",
                    self.theme.body_font, self.theme.base_font_size
                );
                let mut open_font = self.sec_open_font;
                inspector_card(
                    ui,
                    dark,
                    "🔤 フォント・文字サイズ",
                    &font_badge,
                    &mut open_font,
                    |ui| {
                        // Quick font chips (show installed Japanese fonts on the current OS)
                        let all_chips = [
                            ("游ゴシック", "Yu Gothic"),
                            ("游明朝", "Yu Mincho"),
                            ("メイリオ", "Meiryo"),
                            ("UDPゴシック", "BIZ UDPGothic"),
                            ("UDP明朝", "BIZ UDPMincho"),
                            ("MSゴシック", "MS Gothic"),
                            ("MS明朝", "MS Mincho"),
                            ("Noto Sans CJK", "Noto Sans CJK JP"),
                            ("Noto Serif CJK", "Noto Serif CJK JP"),
                            ("Noto Sans JP", "Noto Sans JP"),
                            ("Noto Serif JP", "Noto Serif JP"),
                            ("IPAexゴシック", "IPAexGothic"),
                            ("IPAex明朝", "IPAexMincho"),
                            ("IPAゴシック", "IPAGothic"),
                            ("IPA明朝", "IPAMincho"),
                            ("Takao Pゴシック", "TakaoPGothic"),
                            ("VL Pゴシック", "VL PGothic"),
                        ];
                        let installed_chips: Vec<(&str, &str)> = all_chips
                            .iter()
                            .copied()
                            .filter(|(_, fam)| self.fonts.has_family(fam))
                            .collect();
                        let display_chips: &[(&str, &str)] = if installed_chips.is_empty() {
                            &[
                                ("Noto Sans CJK", "Noto Sans CJK JP"),
                                ("Noto Serif CJK", "Noto Serif CJK JP"),
                                ("IPAexゴシック", "IPAexGothic"),
                            ]
                        } else {
                            &installed_chips
                        };
                        ui.horizontal_wrapped(|ui| {
                            for &(label, fam) in display_chips {
                                let active = self.theme.body_font.eq_ignore_ascii_case(fam);
                                if ui.selectable_label(active, label).clicked() {
                                    self.theme.body_font = fam.to_string();
                                    self.theme.heading_font = fam.to_string();
                                    changed = true;
                                }
                            }
                        });

                        ui.add_space(2.0);
                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "🔍 検索");
                            let search_w = (ui.available_width() - 36.0).clamp(100.0, 190.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.font_search)
                                    .hint_text("フォント名で絞り込み...")
                                    .desired_width(search_w),
                            );
                            if !self.font_search.is_empty() && ui.small_button("×").clicked() {
                                self.font_search.clear();
                            }
                        });

                        let filter_lower = self.font_search.trim().to_lowercase();
                        let filtered_families: Vec<String> = self
                            .fonts
                            .families()
                            .iter()
                            .filter(|f| {
                                filter_lower.is_empty() || f.to_lowercase().contains(&filter_lower)
                            })
                            .cloned()
                            .collect();

                        ui.spacing_mut().slider_width =
                            (ui.available_width() - 165.0).clamp(70.0, 150.0);

                        egui::Grid::new("modern_font_grid")
                            .num_columns(2)
                            .spacing([10.0, 6.0])
                            .show(ui, |ui| {
                                field_label(ui, "本文フォント");
                                changed |= font_combo(
                                    ui,
                                    "body_font_combo",
                                    &mut self.theme.body_font,
                                    &filtered_families,
                                );
                                ui.end_row();

                                field_label(ui, "見出しフォント");
                                changed |= font_combo(
                                    ui,
                                    "heading_font_combo",
                                    &mut self.theme.heading_font,
                                    &filtered_families,
                                );
                                ui.end_row();

                                field_label(ui, "コードフォント");
                                changed |= font_combo(
                                    ui,
                                    "code_font_combo",
                                    &mut self.theme.code_font,
                                    &filtered_families,
                                );
                                ui.end_row();

                                field_label(ui, "文字サイズ");
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.theme.base_font_size, 7.5..=20.0)
                                            .suffix(" pt"),
                                    )
                                    .changed();
                                ui.end_row();

                                field_label(ui, "行間");
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.theme.line_height, 1.15..=2.4)
                                            .step_by(0.05),
                                    )
                                    .changed();
                                ui.end_row();

                                field_label(ui, "段落間隔");
                                changed |= ui
                                    .add(
                                        egui::Slider::new(
                                            &mut self.theme.paragraph_spacing,
                                            2.0..=24.0,
                                        )
                                        .suffix(" pt"),
                                    )
                                    .changed();
                                ui.end_row();
                            });
                    },
                );
                self.sec_open_font = open_font;

                // Card 3: Emphasis & Markers
                let emp_badge = format!(
                    "太字 {:.0}% · マーカー",
                    self.theme.bold_style.size_scale * 100.0
                );
                let mut open_emp = self.sec_open_emp;
                inspector_card(
                    ui,
                    dark,
                    "✨ 強調・マーカー装飾",
                    &emp_badge,
                    &mut open_emp,
                    |ui| {
                        changed |= modern_emphasis_card(
                            ui,
                            dark,
                            "bold_emp",
                            "太字 (**Bold**)",
                            &mut self.theme.bold_style,
                            true,
                            false,
                        );
                        ui.add_space(4.0);
                        changed |= modern_emphasis_card(
                            ui,
                            dark,
                            "highlight_emp",
                            "マーカー (==Mark==)",
                            &mut self.theme.highlight_style,
                            true,
                            false,
                        );
                        ui.add_space(4.0);
                        changed |= modern_emphasis_card(
                            ui,
                            dark,
                            "italic_emp",
                            "斜体 (*Italic*)",
                            &mut self.theme.italic_style,
                            false,
                            true,
                        );
                        ui.add_space(4.0);
                        changed |= modern_emphasis_card(
                            ui,
                            dark,
                            "code_emp",
                            "コード (`code`)",
                            &mut self.theme.inline_code_style,
                            false,
                            false,
                        );
                    },
                );
                self.sec_open_emp = open_emp;

                // Card 4: Headings
                let h_badge = format!(
                    "H1 {:.1}x · H2 {:.1}x{}",
                    self.theme.h1_style.size_scale,
                    self.theme.h2_style.size_scale,
                    if self.theme.h2_style.page_break_before {
                        " · H2改ページ"
                    } else {
                        ""
                    }
                );
                let mut open_heading = self.sec_open_heading;
                inspector_card(
                    ui,
                    dark,
                    "📑 見出しデザイン",
                    &h_badge,
                    &mut open_heading,
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            changed |= ui
                                .checkbox(
                                    &mut self.theme.h2_style.page_break_before,
                                    "見出し2で改ページする",
                                )
                                .on_hover_text(
                                    "ONにすると、見出し2 (## H2) の直前で新しいページに切り替えます",
                                )
                                .changed();
                        });
                        ui.add_space(4.0);
                        changed |=
                            modern_heading_row(ui, dark, "h1_ed", "H1 大見出し", &mut self.theme.h1_style);
                        ui.add_space(4.0);
                        changed |=
                            modern_heading_row(ui, dark, "h2_ed", "H2 中見出し", &mut self.theme.h2_style);
                        ui.add_space(4.0);
                        changed |=
                            modern_heading_row(ui, dark, "h3_ed", "H3 小見出し", &mut self.theme.h3_style);
                    },
                );
                self.sec_open_heading = open_heading;

                // Card 5: Page & Margin Setup
                let page_badge = format!(
                    "{:?} {} · 余白 {:.0}mm",
                    self.theme.paper_size,
                    if self.theme.landscape { "横" } else { "縦" },
                    self.theme.margin_top_mm
                );
                let mut open_page = self.sec_open_page;
                inspector_card(
                    ui,
                    dark,
                    "📄 用紙・余白・配色",
                    &page_badge,
                    &mut open_page,
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "用紙サイズ");
                            for (ps, short) in [
                                (PaperSize::A4, "A4"),
                                (PaperSize::B5, "JIS B5"),
                                (PaperSize::Letter, "Letter"),
                            ] {
                                if ui
                                    .selectable_label(self.theme.paper_size == ps, short)
                                    .clicked()
                                {
                                    self.theme.paper_size = ps;
                                    changed = true;
                                }
                            }
                            ui.separator();
                            if ui
                                .selectable_label(!self.theme.landscape, "縦")
                                .clicked()
                            {
                                self.theme.landscape = false;
                                changed = true;
                            }
                            if ui
                                .selectable_label(self.theme.landscape, "横")
                                .clicked()
                            {
                                self.theme.landscape = true;
                                changed = true;
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "余白 (mm)");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.theme.margin_top_mm)
                                        .range(5.0..=50.0)
                                        .speed(0.5)
                                        .prefix("上 "),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.theme.margin_bottom_mm)
                                        .range(5.0..=50.0)
                                        .speed(0.5)
                                        .prefix("下 "),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.theme.margin_left_mm)
                                        .range(5.0..=50.0)
                                        .speed(0.5)
                                        .prefix("左 "),
                                )
                                .changed();
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut self.theme.margin_right_mm)
                                        .range(5.0..=50.0)
                                        .speed(0.5)
                                        .prefix("右 "),
                                )
                                .changed();
                        });

                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "ページ配色");
                            ui.label("文字");
                            changed |=
                                ui.color_edit_button_srgb(&mut self.theme.text_color).changed();
                            ui.add_space(4.0);
                            ui.label("背景");
                            changed |= ui
                                .color_edit_button_srgb(&mut self.theme.page_background)
                                .changed();
                            ui.add_space(4.0);
                            ui.label("リンク");
                            changed |=
                                ui.color_edit_button_srgb(&mut self.theme.link_color).changed();
                            ui.separator();
                            changed |= ui
                                .checkbox(&mut self.theme.show_page_numbers, "ページ番号")
                                .changed();
                        });
                    },
                );
                self.sec_open_page = open_page;

                // Card 6: Startup Defaults
                let def_badge = format!(
                    "{} · {:.0}%",
                    if self.user_defaults.ui_dark_mode {
                        "ダークUI"
                    } else {
                        "ライトUI"
                    },
                    self.user_defaults.default_preview_zoom * 100.0
                );
                let mut open_defaults = self.sec_open_defaults;
                inspector_card(
                    ui,
                    dark,
                    "⚙ 起動時の初期設定",
                    &def_badge,
                    &mut open_defaults,
                    |ui| {
                        let mut def_changed = false;

                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "起動時UIモード");
                            if ui
                                .selectable_label(self.user_defaults.ui_dark_mode, "🌙 ダーク")
                                .clicked()
                            {
                                self.user_defaults.ui_dark_mode = true;
                                self.ui_dark_mode = true;
                                apply_modern_ui_style(ctx, true);
                                def_changed = true;
                            }
                            if ui
                                .selectable_label(!self.user_defaults.ui_dark_mode, "☀ ライト")
                                .clicked()
                            {
                                self.user_defaults.ui_dark_mode = false;
                                self.ui_dark_mode = false;
                                apply_modern_ui_style(ctx, false);
                                def_changed = true;
                            }
                        });

                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "起動時タブ");
                            if ui
                                .selectable_label(
                                    self.user_defaults.open_sample_on_startup,
                                    "サンプルを開く",
                                )
                                .clicked()
                            {
                                self.user_defaults.open_sample_on_startup = true;
                                def_changed = true;
                            }
                            if ui
                                .selectable_label(
                                    !self.user_defaults.open_sample_on_startup,
                                    "空のタブを開く",
                                )
                                .clicked()
                            {
                                self.user_defaults.open_sample_on_startup = false;
                                def_changed = true;
                            }
                        });

                        ui.horizontal_wrapped(|ui| {
                            field_label(ui, "初期ズーム");
                            ui.spacing_mut().slider_width =
                                (ui.available_width() - 90.0).clamp(60.0, 150.0);
                            def_changed |= ui
                                .add(
                                    egui::Slider::new(
                                        &mut self.user_defaults.default_preview_zoom,
                                        0.5..=2.0,
                                    )
                                    .step_by(0.05),
                                )
                                .changed();
                        });

                        if def_changed {
                            let _ = self.user_defaults.save();
                        }

                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .button("★ 現在のデザインを初期値に保存")
                                .clicked()
                            {
                                self.save_current_as_user_defaults();
                            }
                            if ui.button("出荷時設定へ初期化").clicked() {
                                self.user_defaults = UserDefaultsConfig::default();
                                let _ = self.user_defaults.save();
                                self.ui_dark_mode = self.user_defaults.ui_dark_mode;
                                apply_modern_ui_style(ctx, self.ui_dark_mode);
                                self.reset_to_user_defaults();
                            }
                        });
                    },
                );
                self.sec_open_defaults = open_defaults;
            });

        if changed {
            self.sync_pro_buffers();
            self.needs_relayout = true;
        }
    }

    fn ui_markdown_editor_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let mut snippet: Option<&str> = None;
            if ui.button("B 太字").clicked() {
                snippet = Some("\n**強調テキスト**\n");
            }
            if ui.button("🖍 マーカー").clicked() {
                snippet = Some("\n==マーカー強調テキスト==\n");
            }
            if ui.button("💡 NOTE").clicked() {
                snippet = Some("\n> [!NOTE] ポイント\n> ここに補足事項を記述します。\n");
            }
            if ui.button("⚠ WARNING").clicked() {
                snippet = Some("\n> [!WARNING] 注意事項\n> 重要な注意点を記述します。\n");
            }
            if ui.button("∑ 数式").clicked() {
                snippet = Some("\n$$\\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}$$\n");
            }
            if ui.button("📊 表").clicked() {
                snippet = Some(
                    "\n| 項目 | 内容 | 状態 |\n| :--- | :---: | ---: |\n| サンプル | テスト | 完了 |\n",
                );
            }
            if let Some(s) = snippet {
                let tab = self.active_tab_mut();
                tab.markdown_text.push_str(s);
                tab.parsed_doc = parse_markdown(&tab.markdown_text);
                tab.dirty = true;
                self.needs_relayout = true;
            }
        });
        ui.add_space(4.0);

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                ui.set_max_width(avail_w);
                let tab = &mut self.tabs[self.active_tab_idx];
                let resp = ui.add(
                    egui::TextEdit::multiline(&mut tab.markdown_text)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(avail_w)
                        .desired_rows(34)
                        .lock_focus(true),
                );
                if resp.changed() {
                    tab.parsed_doc = parse_markdown(&tab.markdown_text);
                    tab.dirty = true;
                    self.needs_relayout = true;
                }
            });
    }

    fn ui_pro_code_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.pro_sub_tab,
                ProSubTab::Json,
                RichText::new("JSON テーマ").strong(),
            );
            ui.selectable_value(
                &mut self.pro_sub_tab,
                ProSubTab::Css,
                RichText::new("CSS スタイル").strong(),
            );
            ui.separator();

            match self.pro_sub_tab {
                ProSubTab::Json => {
                    if ui.button("▶ 適用").clicked() {
                        match serde_json::from_str::<ThemeConfig>(&self.json_buffer) {
                            Ok(new_t) => {
                                self.theme = new_t;
                                self.css_buffer = theme_to_css(&self.theme);
                                self.needs_relayout = true;
                                self.pro_status_msg =
                                    Some(("✔ JSONを反映しました".to_string(), false));
                            }
                            Err(e) => {
                                self.pro_status_msg =
                                    Some((format!("❌ JSONエラー: {}", e), true));
                            }
                        }
                    }
                    if ui.button("📂 読込").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            self.load_theme_from_path(&p);
                        }
                    }
                    if ui.button("💾 保存").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .set_file_name("theme.json")
                            .add_filter("JSON", &["json"])
                            .save_file()
                        {
                            let _ = std::fs::write(&p, &self.json_buffer);
                        }
                    }
                }
                ProSubTab::Css => {
                    if ui.button("▶ 適用").clicked() {
                        if let Ok(n) = apply_css_to_theme(&self.css_buffer, &mut self.theme) {
                            self.json_buffer =
                                serde_json::to_string_pretty(&self.theme).unwrap_or_default();
                            self.needs_relayout = true;
                            self.pro_status_msg =
                                Some((format!("✔ CSS反映 ({}項目)", n), false));
                        }
                    }
                    if ui.button("📂 読込").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("CSS", &["css"])
                            .pick_file()
                        {
                            self.load_theme_from_path(&p);
                        }
                    }
                    if ui.button("💾 保存").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .set_file_name("style.css")
                            .add_filter("CSS", &["css"])
                            .save_file()
                        {
                            let _ = std::fs::write(&p, &self.css_buffer);
                        }
                    }
                }
            }
        });

        if let Some((msg, is_err)) = &self.pro_status_msg {
            ui.colored_label(
                if *is_err {
                    Color32::from_rgb(248, 113, 113)
                } else {
                    Color32::from_rgb(52, 211, 153)
                },
                msg,
            );
        }

        ui.add_space(4.0);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail_w = ui.available_width();
                ui.set_max_width(avail_w);
                match self.pro_sub_tab {
                    ProSubTab::Json => {
                        let resp = ui.add(
                            egui::TextEdit::multiline(&mut self.json_buffer)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(avail_w)
                                .desired_rows(32),
                        );
                        if resp.changed() {
                            if let Ok(new_t) = serde_json::from_str::<ThemeConfig>(&self.json_buffer)
                            {
                                self.theme = new_t;
                                self.css_buffer = theme_to_css(&self.theme);
                                self.needs_relayout = true;
                                self.pro_status_msg =
                                    Some(("✔ ライブJSON反映中".to_string(), false));
                            }
                        }
                    }
                    ProSubTab::Css => {
                        let resp = ui.add(
                            egui::TextEdit::multiline(&mut self.css_buffer)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(avail_w)
                                .desired_rows(32),
                        );
                        if resp.changed() {
                            if let Ok(n) = apply_css_to_theme(&self.css_buffer, &mut self.theme) {
                                if n > 0 {
                                    self.json_buffer = serde_json::to_string_pretty(&self.theme)
                                        .unwrap_or_default();
                                    self.needs_relayout = true;
                                    self.pro_status_msg = Some((
                                        format!("✔ ライブCSS反映中 ({}項目)", n),
                                        false,
                                    ));
                                }
                            }
                        }
                    }
                }
            });
    }

    fn ui_preview_panel(&mut self, ui: &mut egui::Ui) {
        let total_pages = self.doc_layout.pages.len().max(1);

        // Clean Preview Header Bar
        let bar_bg = if self.ui_dark_mode {
            Color32::from_rgb(20, 23, 30)
        } else {
            Color32::from_rgb(241, 245, 249)
        };
        Frame::none()
            .fill(bar_bg)
            .inner_margin(Margin::symmetric(14.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "全 {} ページ  ({:?} {})",
                            total_pages,
                            self.theme.paper_size,
                            if self.theme.landscape { "横" } else { "縦" }
                        ))
                        .size(12.5)
                        .strong(),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("100%").clicked() {
                            self.preview_zoom = 1.0;
                            self.needs_rerender_preview = true;
                        }
                        if ui.button("＋").clicked() {
                            self.preview_zoom = (self.preview_zoom + 0.15).clamp(0.4, 2.5);
                            self.needs_rerender_preview = true;
                        }
                        if ui
                            .add(
                                egui::Slider::new(&mut self.preview_zoom, 0.4..=2.5)
                                    .show_value(false)
                                    .step_by(0.05),
                            )
                            .changed()
                        {
                            self.needs_rerender_preview = true;
                        }
                        if ui.button("−").clicked() {
                            self.preview_zoom = (self.preview_zoom - 0.15).clamp(0.4, 2.5);
                            self.needs_rerender_preview = true;
                        }
                        ui.label(
                            RichText::new(format!("{:.0}%", self.preview_zoom * 100.0))
                                .size(12.5)
                                .strong(),
                        );
                    });
                });
            });

        // Distinct dark/slate desk background so white paper sheets and page gaps stand out clearly
        let canvas_bg = if self.ui_dark_mode {
            Color32::from_rgb(11, 13, 18)
        } else {
            Color32::from_rgb(186, 198, 214)
        };

        Frame::none()
            .fill(canvas_bg)
            .inner_margin(Margin::same(0.0))
            .show(ui, |ui| {
                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let avail_w = ui.available_width();
                        ui.add_space(20.0);

                        for (page_idx, (page, tex)) in self
                            .doc_layout
                            .pages
                            .iter()
                            .zip(self.preview_textures.iter())
                            .enumerate()
                        {
                            let disp_w = page.width_pt * self.preview_zoom;
                            let disp_h = page.height_pt * self.preview_zoom;
                            let pad_x = ((avail_w - disp_w) * 0.5).max(16.0);

                            // Individual paper sheet with crisp border and drop shadow
                            ui.horizontal(|ui| {
                                ui.add_space(pad_x);
                                Frame::none()
                                    .shadow(egui::epaint::Shadow {
                                        offset: Vec2::new(0.0, 5.0),
                                        blur: 16.0,
                                        spread: 1.5,
                                        color: Color32::from_black_alpha(140),
                                    })
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if self.ui_dark_mode {
                                            Color32::from_rgb(71, 85, 105)
                                        } else {
                                            Color32::from_rgb(148, 163, 184)
                                        },
                                    ))
                                    .show(ui, |ui| {
                                        ui.image((tex.id(), Vec2::new(disp_w, disp_h)));
                                    });
                            });

                            // Visual sheet break between pages (desk gap + page indicator pill)
                            if page_idx + 1 < total_pages {
                                ui.add_space(10.0);
                                ui.vertical_centered(|ui| {
                                    let pill_fg = if self.ui_dark_mode {
                                        Color32::from_rgb(100, 116, 139)
                                    } else {
                                        Color32::from_rgb(71, 85, 105)
                                    };
                                    ui.label(
                                        RichText::new(format!(
                                            "—  {} / {} ページ  —",
                                            page_idx + 1,
                                            total_pages
                                        ))
                                        .size(11.0)
                                        .color(pill_fg),
                                    );
                                });
                                ui.add_space(12.0);
                            }
                        }

                        ui.add_space(24.0);
                    });
            });
    }
}

/// Renders a sleek, card-style collapsible inspector section with a right-aligned summary badge
fn inspector_card(
    ui: &mut egui::Ui,
    dark: bool,
    title: &str,
    badge: &str,
    is_open: &mut bool,
    add_body: impl FnOnce(&mut egui::Ui),
) {
    let outer_w = ui.available_width();
    let card_bg = if dark {
        Color32::from_rgb(27, 32, 42)
    } else {
        Color32::WHITE
    };
    let border_col = if *is_open {
        if dark {
            Color32::from_rgb(59, 130, 246)
        } else {
            Color32::from_rgb(96, 165, 250)
        }
    } else if dark {
        Color32::from_rgb(42, 49, 64)
    } else {
        Color32::from_rgb(203, 213, 225)
    };

    Frame::none()
        .fill(card_bg)
        .stroke(Stroke::new(1.0_f32, border_col))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            let inner_w = (outer_w - 26.0).max(120.0);
            ui.set_max_width(inner_w);

            let header_resp = ui
                .horizontal(|ui| {
                    let arrow = if *is_open { "▼" } else { "▶" };
                    let title_col = if dark {
                        Color32::from_rgb(241, 245, 249)
                    } else {
                        Color32::from_rgb(15, 23, 42)
                    };
                    let clicked = ui
                        .add(
                            egui::Button::new(
                                RichText::new(format!("{} {}", arrow, title))
                                    .size(13.0)
                                    .strong()
                                    .color(title_col),
                            )
                            .frame(false),
                        )
                        .clicked();

                    // Only draw the right summary badge in the remaining horizontal space so it never overlaps the title
                    let rem_w = ui.available_width();
                    if rem_w > 75.0 {
                        ui.allocate_ui_with_layout(
                            Vec2::new(rem_w, 20.0),
                            Layout::right_to_left(Align::Center),
                            |ui| {
                                let badge_bg = if dark {
                                    Color32::from_rgb(17, 20, 27)
                                } else {
                                    Color32::from_rgb(241, 245, 249)
                                };
                                let badge_fg = if dark {
                                    Color32::from_rgb(148, 163, 184)
                                } else {
                                    Color32::from_rgb(100, 116, 139)
                                };
                                Frame::none()
                                    .fill(badge_bg)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin::symmetric(6.0, 2.0))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(badge).size(10.5).color(badge_fg));
                                    });
                            },
                        );
                    }
                    clicked
                })
                .inner;

            if header_resp {
                *is_open = !*is_open;
            }

            if *is_open {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                add_body(ui);
            }
        });

    ui.add_space(6.0);
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.0)
            .color(Color32::from_rgb(148, 163, 184)),
    );
}

fn font_combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &mut String,
    families: &[String],
) -> bool {
    let mut changed = false;
    let combo_w = (ui.available_width() - 8.0).clamp(120.0, 220.0);
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected.as_str())
        .width(combo_w)
        .show_ui(ui, |ui| {
            for fam in families {
                if ui.selectable_label(selected == fam, fam).clicked() {
                    *selected = fam.clone();
                    changed = true;
                }
            }
        });
    changed
}

fn modern_emphasis_card(
    ui: &mut egui::Ui,
    dark: bool,
    id_prefix: &str,
    title: &str,
    style: &mut EmphasisStyle,
    show_weight: bool,
    show_slant: bool,
) -> bool {
    let mut changed = false;
    let outer_w = ui.available_width();
    let sub_bg = if dark {
        Color32::from_rgb(20, 23, 31)
    } else {
        Color32::from_rgb(248, 250, 252)
    };

    ui.push_id(id_prefix, |ui| {
        Frame::none()
            .fill(sub_bg)
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                let inner_w = (outer_w - 20.0).max(100.0);
                ui.set_max_width(inner_w);

                // Row 1: Live visual preview badge + Size multiplier slider (left-to-right wrapped, never overlaps!)
                ui.horizontal_wrapped(|ui| {
                    let preview_bg = if style.use_background {
                        Color32::from_rgb(
                            style.background_color[0],
                            style.background_color[1],
                            style.background_color[2],
                        )
                    } else {
                        Color32::from_rgb(255, 255, 255)
                    };
                    let preview_fg =
                        Color32::from_rgb(style.color[0], style.color[1], style.color[2]);

                    Frame::none()
                        .fill(preview_bg)
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(71, 85, 105)))
                        .rounding(Rounding::same(4.0))
                        .inner_margin(Margin::symmetric(7.0, 2.0))
                        .show(ui, |ui| {
                            let mut rt = RichText::new(title).size(12.0).color(preview_fg).strong();
                            if style.underline {
                                rt = rt.underline();
                            }
                            if show_slant && style.slant > 0.05 {
                                rt = rt.italics();
                            }
                            ui.label(rt);
                        });

                    ui.add_space(4.0);
                    field_label(ui, "サイズ");
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 62.0).clamp(55.0, 115.0);
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut style.size_scale, 0.75..=1.80)
                                .step_by(0.02)
                                .suffix("x"),
                        )
                        .changed();
                });

                // Row 2: Color pickers & toggles (wrapped so it never pushes width)
                ui.horizontal_wrapped(|ui| {
                    field_label(ui, "文字色");
                    changed |= ui.color_edit_button_srgb(&mut style.color).changed();

                    ui.add_space(4.0);
                    changed |= ui.checkbox(&mut style.use_background, "背景").changed();
                    if style.use_background {
                        changed |= ui
                            .color_edit_button_srgb(&mut style.background_color)
                            .changed();
                    }

                    ui.add_space(4.0);
                    if ui.selectable_label(style.underline, "U 下線").clicked() {
                        style.underline = !style.underline;
                        changed = true;
                    }

                    if show_weight {
                        ui.add_space(4.0);
                        field_label(ui, "太さ");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut style.stroke_boost)
                                    .range(0.0..=1.4)
                                    .speed(0.05)
                                    .prefix("+"),
                            )
                            .changed();
                    }
                    if show_slant {
                        ui.add_space(4.0);
                        field_label(ui, "傾き");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut style.slant)
                                    .range(0.0..=0.35)
                                    .speed(0.01),
                            )
                            .changed();
                    }
                });
            });
    });
    changed
}

fn modern_heading_row(
    ui: &mut egui::Ui,
    dark: bool,
    id_prefix: &str,
    title: &str,
    style: &mut HeadingStyle,
) -> bool {
    let mut changed = false;
    let outer_w = ui.available_width();
    let sub_bg = if dark {
        Color32::from_rgb(20, 23, 31)
    } else {
        Color32::from_rgb(248, 250, 252)
    };

    ui.push_id(id_prefix, |ui| {
        Frame::none()
            .fill(sub_bg)
            .rounding(Rounding::same(6.0))
            .inner_margin(Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                let inner_w = (outer_w - 20.0).max(100.0);
                ui.set_max_width(inner_w);

                // Row 1: Heading Label + Size Multiplier Slider (left-to-right wrapped)
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(title).size(12.5).strong());
                    ui.add_space(6.0);
                    field_label(ui, "サイズ");
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 62.0).clamp(60.0, 130.0);
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut style.size_scale, 1.0..=2.8)
                                .step_by(0.05)
                                .suffix("x"),
                        )
                        .changed();
                });

                // Row 2: Color & Decoration Pills (left-to-right wrapped, never overlaps!)
                ui.horizontal_wrapped(|ui| {
                    field_label(ui, "文字色");
                    changed |= ui.color_edit_button_srgb(&mut style.color).changed();
                    ui.add_space(4.0);

                    if ui.selectable_label(style.underline, "下線").clicked() {
                        style.underline = !style.underline;
                        changed = true;
                    }
                    if style.underline {
                        changed |= ui
                            .color_edit_button_srgb(&mut style.underline_color)
                            .changed();
                    }
                    ui.add_space(4.0);
                    if ui
                        .selectable_label(style.left_accent_bar, "左バー")
                        .clicked()
                    {
                        style.left_accent_bar = !style.left_accent_bar;
                        changed = true;
                    }
                    ui.add_space(4.0);
                    if ui
                        .selectable_label(style.page_break_before, "改ページ")
                        .on_hover_text("この見出しの直前で改ページします")
                        .clicked()
                    {
                        style.page_break_before = !style.page_break_before;
                        changed = true;
                    }
                });
            });
    });
    changed
}

pub const DEFAULT_SAMPLE_MD: &str = r##"---
title: Marupo — 純Rust製ドキュメント変換エンジン
author: Antigravity & User
date: 2026-10-29
version: 1.0.0
tags: [Rust, Markdown, PDF, DesktopApp]
status: Production Ready
---

# 1. はじめに（完全Rust製ネイティブ設計）

このアプリケーションは、WebViewや外部コマンドに一切依存せず、**100%純粋なRustコード**のみでMarkdownの解析・組版レイアウト・プレビュー描画・PDFバイナリ出力までを一貫して行うデスクトップアプリです[^rust]。

左側の設定パネルから、**システム内フォントの切り替え**や**太字強調の色・サイズ倍率**、*斜体スタイル*、さらに ==ハイライト（マーカー）の色と大きさ== をリアルタイムに変更できます。

> [!NOTE] 拡張記法とメタデータへの対応
> 文頭のYAML Frontmatter（プロパティ）は、「非表示」「ドキュメントヘッダー形式」「プロパティテーブル形式」の3種類から自由に選べます。

## 2. 強調・装飾と拡張記法のデモ

- **太字強調 (`**Bold**`)**: 色やサイズ倍率（1.0x〜1.8x）、下線の有無を自由に変更可能
- *斜体強調 (`*Italic*`)*: 任意のフォントに対して美しい斜体シア変換と配色を適用
- ==ハイライト (`==Highlight==`)==: 蛍光ペン風の背景マーカーと文字色をカスタマイズ可能
- ~~取り消し線 (`~~Strike~~`)~~ および `inline_code()` のスタイル調整
- タスクリスト（GFM）の自動チェックボックス描画:
  - [x] 純Rustレイアウトエンジンの実装
  - [x] システムフォント検索（`.ttc` / `.ttf` / `.otf` 対応）
  - [x] プロ向け JSON テーマ ＆ CSS スタイルシート連携
  - [ ] お好みのフォントと配色でPDFを出力してみる

> [!TIP] プロ向けのJSON / CSSカスタマイズ
> 左ペインの「{ } JSON / CSS」タブを開くと、現在のスタイル設定をJSONまたはCSSとして直接編集・インポート・エクスポートできます。

## 3. テーブル（表組み）と数式レンダリング

| 機能カテゴリ | 対応記法・仕様 | エンジン実装 | 出力品質 |
| :--- | :--- | :---: | ---: |
| メタデータ | YAML Frontmatter (`---`) | 純Rustパーサー | ベクター描画 |
| コールアウト | `[!NOTE]`, `[!TIP]`, `[!WARNING]` | 自動ボックス組版 | 100% 一致 |
| 数式 (Math) | インライン `$...$` / ブロック `$$...$$` | 2次元数式レイアウト | 無限ズーム対応 |
| フォント | OS内の全システムフォント | `fontdb` + `ttf-parser` | 文字化けゼロ |

### 数式（LaTeX記法）の組版例

文中のインライン数式 $E = mc^2$ や $\alpha^2 + \beta^2 = \gamma^2$ だけでなく、分数や根号・総和記号を含むディスプレイ数式も純Rustで2次元組版します：

$$x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$

$$\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}$$

## 4. コードブロックと脚注

```rust
fn main() {
    let doc = parse_markdown("# Hello Pure Rust PDF!");
    println!("Rendered {} blocks seamlessly!", doc.blocks.len());
}
```

> [!WARNING] 日本語フォントの禁則処理について
> 行頭に句読点（「、」「。」）や閉じ括弧（「）」）が来ないよう、実測グリフ幅に基づく自動折り返しと禁則処理を行っています。

[^rust]: 画面プレビュー（`tiny-skia`）とPDF出力（`pdf-writer`）が同一の座標プリミティブ列を共有しているため、プレビューと出力PDFで1ピクセルのズレも発生しません。
"##;
