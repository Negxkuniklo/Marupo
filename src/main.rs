#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod app;
pub mod config;
pub mod icon;
pub mod markdown;
pub mod renderer;

use app::MarkdownToPdfApp;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == "--export" {
        run_cli_export(&args[2..]);
        return Ok(());
    }

    let icon_rgba = icon::render_mrp_icon_rgba(64);
    let app_icon = std::sync::Arc::new(egui::IconData {
        rgba: icon_rgba,
        width: 64,
        height: 64,
    });

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Marupo (マルポ) — MarkdownをRustでPDFにするアプリ")
            .with_icon(app_icon)
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([900.0, 600.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Marupo",
        native_options,
        Box::new(|cc| Ok(Box::new(MarkdownToPdfApp::new(cc)))),
    )
}

fn run_cli_export(args: &[String]) {
    let input_md = &args[0];
    let output_pdf = &args[1];
    let mut theme = config::ThemeConfig::default();

    if args.len() >= 4 && args[2] == "--theme" {
        let theme_path = &args[3];
        if let Ok(content) = std::fs::read_to_string(theme_path) {
            if theme_path.to_lowercase().ends_with(".css") {
                let _ = config::apply_css_to_theme(&content, &mut theme);
            } else if let Ok(t) = serde_json::from_str::<config::ThemeConfig>(&content) {
                theme = t;
            }
        }
    }

    let md_content = std::fs::read_to_string(input_md).expect("Failed to read input markdown");
    let parsed = markdown::parse_markdown(&md_content);

    if output_pdf.to_lowercase().ends_with(".html") || output_pdf.to_lowercase().ends_with(".htm") {
        let html_str = renderer::export_html_string(&parsed, &theme);
        std::fs::write(output_pdf, html_str.as_bytes()).expect("Failed to write output HTML");
        println!("Exported HTML ({} bytes) to {}", html_str.len(), output_pdf);
        return;
    }

    let mut fonts = renderer::FontSystem::new();
    let layout = renderer::layout_document(&parsed, &theme, &mut fonts);
    let pdf_bytes = renderer::export_pdf_bytes(&layout, &fonts);
    std::fs::write(output_pdf, &pdf_bytes).expect("Failed to write output PDF");
    println!(
        "Exported {} pages ({} bytes) to {}",
        layout.pages.len(),
        pdf_bytes.len(),
        output_pdf
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_end_to_end_pipeline() {
        let parsed = markdown::parse_markdown(app::DEFAULT_SAMPLE_MD);
        assert!(!parsed.frontmatter.is_empty());
        assert_eq!(
            parsed.frontmatter.get("title"),
            Some("Marupo — 純Rust製ドキュメント変換エンジン")
        );
        assert!(!parsed.blocks.is_empty());
        assert_eq!(parsed.footnotes.len(), 1);

        let mut theme = config::ThemeConfig::preset_vivid_emphasis();
        let css = config::theme_to_css(&theme);
        let applied = config::apply_css_to_theme(&css, &mut theme).unwrap();
        assert!(applied > 10);

        let html = renderer::export_html_string(&parsed, &theme);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("class=\"page-sheet\""));
        assert!(html.contains("class=\"math-frac\""));

        let mut fonts = renderer::FontSystem::new();
        assert!(!fonts.families().is_empty());

        let layout = renderer::layout_document(&parsed, &theme, &mut fonts);
        assert!(!layout.pages.is_empty());

        let preview = renderer::render_page_preview(&layout.pages[0], &fonts, 1.0);
        assert!(preview.is_some());

        let pdf_bytes = renderer::export_pdf_bytes(&layout, &fonts);
        assert!(pdf_bytes.starts_with(b"%PDF-"));
        assert!(pdf_bytes.len() > 1000);
    }
}
