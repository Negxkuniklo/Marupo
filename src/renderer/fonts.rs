use fontdb::{Database, ID as DbFaceId};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use ttf_parser::{Face, GlyphId, OutlineBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadedFontId(pub usize);

#[derive(Clone)]
pub struct LoadedFont {
    pub family_name: String,
    pub data: Arc<Vec<u8>>,
    pub face_index: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CurveTo(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub font_id: LoadedFontId,
    pub glyph_id: u16,
    pub advance_em: f32,
}

pub struct FontSystem {
    db: Database,
    families: Vec<String>,
    loaded_fonts: Vec<LoadedFont>,
    family_to_loaded: HashMap<String, LoadedFontId>,
    fallback_chain: Vec<LoadedFontId>,
}

impl FontSystem {
    pub fn new() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();

        #[cfg(target_os = "linux")]
        {
            // Inside Flatpak, host fonts are exposed under /run/host/* and app fonts under /app/share/fonts.
            // Only load directories not already scanned via fontconfig to avoid duplicate faces.
            let extra_dirs = [
                "/run/host/fonts",
                "/run/host/user-fonts",
                "/run/host/local-fonts",
                "/app/share/fonts",
            ];
            for dir_str in extra_dirs {
                let dir_path = std::path::Path::new(dir_str);
                if !dir_path.is_dir() {
                    continue;
                }
                let already_loaded = db.faces().any(|f| match &f.source {
                    fontdb::Source::File(p) => p.starts_with(dir_path),
                    fontdb::Source::SharedFile(p, _) => p.starts_with(dir_path),
                    fontdb::Source::Binary(_) => false,
                });
                if !already_loaded {
                    db.load_fonts_dir(dir_path);
                }
            }
        }

        let mut family_set = BTreeSet::new();
        for face in db.faces() {
            for (name, _) in &face.families {
                if !name.starts_with('@') && !name.trim().is_empty() {
                    family_set.insert(name.clone());
                }
            }
        }

        let families: Vec<String> = family_set.into_iter().collect();

        let mut sys = Self {
            db,
            families,
            loaded_fonts: Vec::new(),
            family_to_loaded: HashMap::new(),
            fallback_chain: Vec::new(),
        };

        // Preload reliable fallback chain (Japanese + Latin + Monospace + Math/Symbols across Windows & Linux)
        let fallbacks = [
            // Windows Japanese & UI/Math
            "Yu Gothic",
            "游ゴシック",
            "Meiryo",
            "メイリオ",
            "BIZ UDPGothic",
            "MS Gothic",
            "Segoe UI",
            "Segoe UI Symbol",
            "Cambria Math",
            "Arial",
            // Linux / Open-source Japanese (Sans & Serif)
            "Noto Sans CJK JP",
            "Noto Sans JP",
            "Noto Sans CJK",
            "Source Han Sans JP",
            "Source Han Sans",
            "IPAexGothic",
            "IPAGothic",
            "IPAPGothic",
            "TakaoPGothic",
            "TakaoGothic",
            "VL PGothic",
            "VL Gothic",
            "IBM Plex Sans JP",
            "M+ 1p",
            "M PLUS 1p",
            "Droid Sans Japanese",
            "Droid Sans Fallback",
            "Noto Serif CJK JP",
            "Noto Serif JP",
            "IPAexMincho",
            "IPAMincho",
            // Linux Latin, Monospace & Math/Symbols
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
            "Ubuntu",
            "Cantarell",
            "Noto Sans Mono",
            "DejaVu Sans Mono",
            "Liberation Mono",
            "Noto Sans Symbols",
            "Noto Sans Symbols 2",
            "DejaVu Math TeX Gyre",
            "STIX Two Math",
            "Latin Modern Math",
            "OpenSymbol",
        ];

        for fb in fallbacks {
            if let Some(id) = sys.load_family(fb) {
                if !sys.fallback_chain.contains(&id) {
                    sys.fallback_chain.push(id);
                }
            }
        }

        // Last-resort scan: ensure at least one Japanese-capable font is in fallback_chain
        let has_jp = sys
            .fallback_chain
            .iter()
            .any(|&id| sys.try_char_in_font(id, 'あ').is_some());
        if !has_jp {
            if let Some(id) = sys.find_and_load_face_with_char('あ') {
                if !sys.fallback_chain.contains(&id) {
                    sys.fallback_chain.push(id);
                }
            }
        }

        // Ensure at least one math-capable font is in fallback_chain
        let has_math = sys
            .fallback_chain
            .iter()
            .any(|&id| sys.try_char_in_font(id, '∑').is_some());
        if !has_math {
            if let Some(id) = sys.find_and_load_face_with_char('∑') {
                if !sys.fallback_chain.contains(&id) {
                    sys.fallback_chain.push(id);
                }
            }
        }

        sys
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }

    pub fn has_family(&self, requested: &str) -> bool {
        let req = requested.trim();
        self.families.iter().any(|f| f.eq_ignore_ascii_case(req))
    }

    fn find_and_load_face_with_char(&mut self, target_ch: char) -> Option<LoadedFontId> {
        let candidate_ids: Vec<(DbFaceId, u32, String)> = self
            .db
            .faces()
            .filter(|f| f.style == fontdb::Style::Normal)
            .filter_map(|f| {
                let fam = f.families.first()?.0.clone();
                if fam.starts_with('@') || fam.trim().is_empty() {
                    return None;
                }
                Some((f.id, f.index, fam))
            })
            .collect();

        for (face_id, face_index, fam) in candidate_ids {
            let mut bytes_opt: Option<Vec<u8>> = None;
            self.db.with_face_data(face_id, |data, _| {
                if let Ok(face) = Face::parse(data, face_index) {
                    if face.glyph_index(target_ch).is_some_and(|g| g.0 != 0) {
                        bytes_opt = Some(data.to_vec());
                    }
                }
            });
            if let Some(bytes) = bytes_opt {
                let key = fam.trim().to_lowercase();
                if let Some(&existing) = self.family_to_loaded.get(&key) {
                    return Some(existing);
                }
                let id = LoadedFontId(self.loaded_fonts.len());
                self.loaded_fonts.push(LoadedFont {
                    family_name: fam,
                    data: Arc::new(bytes),
                    face_index,
                });
                self.family_to_loaded.insert(key, id);
                return Some(id);
            }
        }
        None
    }

    /// Finds raw font bytes and TTC face index suitable for egui GUI Japanese rendering
    pub fn find_egui_japanese_font(&mut self) -> Option<(Vec<u8>, u32)> {
        let candidates = [
            // Windows
            "Yu Gothic UI",
            "Meiryo",
            "BIZ UDPGothic",
            "Yu Gothic",
            "MS UI Gothic",
            "MS Gothic",
            // Linux / Open-source
            "Noto Sans CJK JP",
            "Noto Sans JP",
            "Noto Sans CJK",
            "Source Han Sans JP",
            "Source Han Sans",
            "IPAexGothic",
            "IPAGothic",
            "IPAPGothic",
            "TakaoPGothic",
            "TakaoGothic",
            "VL PGothic",
            "VL Gothic",
            "IBM Plex Sans JP",
            "M+ 1p",
            "M PLUS 1p",
            "Droid Sans Japanese",
            "Droid Sans Fallback",
        ];
        for cand in candidates {
            if let Some(id) = self.load_family(cand) {
                let font = &self.loaded_fonts[id.0];
                if ab_glyph::FontRef::try_from_slice_and_index(&font.data, font.face_index).is_ok()
                {
                    if self.try_char_in_font(id, 'あ').is_some() {
                        return Some(((*font.data).clone(), font.face_index));
                    }
                }
            }
        }
        if let Some(id) = self.find_and_load_face_with_char('あ') {
            let font = &self.loaded_fonts[id.0];
            if ab_glyph::FontRef::try_from_slice_and_index(&font.data, font.face_index).is_ok() {
                return Some(((*font.data).clone(), font.face_index));
            }
        }
        None
    }

    pub fn load_family(&mut self, requested: &str) -> Option<LoadedFontId> {
        let key = requested.trim().to_lowercase();
        if let Some(&id) = self.family_to_loaded.get(&key) {
            return Some(id);
        }

        let aliases = family_aliases(requested);
        let mut matched_face_id: Option<DbFaceId> = None;

        // First try exact Regular (400) weight + Normal style match
        for alias in &aliases {
            for face in self.db.faces() {
                if face
                    .families
                    .iter()
                    .any(|(f, _)| f.eq_ignore_ascii_case(alias))
                    && face.style == fontdb::Style::Normal
                    && face.weight == fontdb::Weight::NORMAL
                {
                    matched_face_id = Some(face.id);
                    break;
                }
            }
            if matched_face_id.is_some() {
                break;
            }
        }

        // Next try Normal weight range (350..=550) + Normal style match
        if matched_face_id.is_none() {
            for alias in &aliases {
                for face in self.db.faces() {
                    if face
                        .families
                        .iter()
                        .any(|(f, _)| f.eq_ignore_ascii_case(alias))
                        && face.style == fontdb::Style::Normal
                        && (face.weight.0 >= 350 && face.weight.0 <= 550)
                    {
                        matched_face_id = Some(face.id);
                        break;
                    }
                }
                if matched_face_id.is_some() {
                    break;
                }
            }
        }

        // Next try any weight/style match
        if matched_face_id.is_none() {
            for alias in &aliases {
                for face in self.db.faces() {
                    if face
                        .families
                        .iter()
                        .any(|(f, _)| f.eq_ignore_ascii_case(alias))
                    {
                        matched_face_id = Some(face.id);
                        break;
                    }
                }
                if matched_face_id.is_some() {
                    break;
                }
            }
        }

        let face_id = matched_face_id?;
        let face_info = self.db.face(face_id)?;
        let face_index = face_info.index;

        let mut bytes_opt: Option<Vec<u8>> = None;
        self.db.with_face_data(face_id, |data, _| {
            bytes_opt = Some(data.to_vec());
        });
        let bytes = bytes_opt?;

        // Validate with ttf_parser
        if Face::parse(&bytes, face_index).is_err() {
            return None;
        }

        let id = LoadedFontId(self.loaded_fonts.len());
        self.loaded_fonts.push(LoadedFont {
            family_name: requested.to_string(),
            data: Arc::new(bytes),
            face_index,
        });
        self.family_to_loaded.insert(key, id);
        Some(id)
    }

    pub fn resolve_family_or_default(&mut self, requested: &str) -> LoadedFontId {
        if let Some(id) = self.load_family(requested) {
            return id;
        }
        if let Some(&first_fb) = self.fallback_chain.first() {
            return first_fb;
        }
        // Fallback to any face in the database
        let all_ids: Vec<DbFaceId> = self.db.faces().map(|f| f.id).collect();
        for fid in all_ids {
            if let Some(f) = self.db.face(fid) {
                if let Some((name, _)) = f.families.first() {
                    let name_cloned = name.clone();
                    if let Some(id) = self.load_family(&name_cloned) {
                        return id;
                    }
                }
            }
        }
        LoadedFontId(0)
    }

    pub fn resolve_char(&self, primary: LoadedFontId, ch: char) -> GlyphInfo {
        if let Some(info) = self.try_char_in_font(primary, ch) {
            return info;
        }
        for &fb in &self.fallback_chain {
            if fb != primary {
                if let Some(info) = self.try_char_in_font(fb, ch) {
                    return info;
                }
            }
        }
        // Fallback space width if character is completely missing
        let default_adv = if ch.is_ascii() { 0.55 } else { 1.0 };
        GlyphInfo {
            font_id: primary,
            glyph_id: 0,
            advance_em: default_adv,
        }
    }

    fn try_char_in_font(&self, font_id: LoadedFontId, ch: char) -> Option<GlyphInfo> {
        let font = self.loaded_fonts.get(font_id.0)?;
        let face = Face::parse(&font.data, font.face_index).ok()?;
        let gid = face.glyph_index(ch)?;
        if gid.0 == 0 {
            return None;
        }
        let upem = face.units_per_em().max(1) as f32;
        let adv = face.glyph_hor_advance(gid).unwrap_or(face.units_per_em()) as f32 / upem;
        Some(GlyphInfo {
            font_id,
            glyph_id: gid.0,
            advance_em: adv,
        })
    }

    /// Outlines a glyph in normalized EM units (1.0 = font_size), where Y+ is UP (font coordinate space)
    pub fn outline_glyph_em(&self, font_id: LoadedFontId, glyph_id: u16) -> Vec<PathCmd> {
        if glyph_id == 0 {
            return Vec::new();
        }
        let Some(font) = self.loaded_fonts.get(font_id.0) else {
            return Vec::new();
        };
        let Ok(face) = Face::parse(&font.data, font.face_index) else {
            return Vec::new();
        };
        let upem = face.units_per_em().max(1) as f32;
        let mut collector = PathCollector {
            scale: 1.0 / upem,
            cmds: Vec::with_capacity(32),
        };
        let _ = face.outline_glyph(GlyphId(glyph_id), &mut collector);
        collector.cmds
    }
}

struct PathCollector {
    scale: f32,
    cmds: Vec<PathCmd>,
}

impl OutlineBuilder for PathCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cmds.push(PathCmd::MoveTo(x * self.scale, y * self.scale));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.cmds.push(PathCmd::LineTo(x * self.scale, y * self.scale));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.cmds.push(PathCmd::QuadTo(
            x1 * self.scale,
            y1 * self.scale,
            x * self.scale,
            y * self.scale,
        ));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cmds.push(PathCmd::CurveTo(
            x1 * self.scale,
            y1 * self.scale,
            x2 * self.scale,
            y2 * self.scale,
            x * self.scale,
            y * self.scale,
        ));
    }

    fn close(&mut self) {
        self.cmds.push(PathCmd::Close);
    }
}

fn family_aliases(name: &str) -> Vec<String> {
    let trimmed = name.trim();
    let mut out = vec![trimmed.to_string()];
    match trimmed.to_lowercase().as_str() {
        "yu gothic" | "游ゴシック" | "yugothic" => {
            out.extend([
                "Yu Gothic".to_string(),
                "游ゴシック".to_string(),
                "Yu Gothic UI".to_string(),
                "Noto Sans CJK JP".to_string(),
                "Noto Sans JP".to_string(),
                "IPAexGothic".to_string(),
                "IPAGothic".to_string(),
                "TakaoPGothic".to_string(),
                "VL PGothic".to_string(),
                "BIZ UDPGothic".to_string(),
            ]);
        }
        "yu mincho" | "游明朝" | "yumincho" => {
            out.extend([
                "Yu Mincho".to_string(),
                "游明朝".to_string(),
                "Noto Serif CJK JP".to_string(),
                "Noto Serif JP".to_string(),
                "IPAexMincho".to_string(),
                "IPAMincho".to_string(),
                "TakaoMincho".to_string(),
            ]);
        }
        "meiryo" | "メイリオ" => {
            out.extend([
                "Meiryo".to_string(),
                "メイリオ".to_string(),
                "Meiryo UI".to_string(),
                "Noto Sans CJK JP".to_string(),
                "Noto Sans JP".to_string(),
                "IPAexGothic".to_string(),
                "IPAGothic".to_string(),
                "TakaoPGothic".to_string(),
            ]);
        }
        "ms gothic" | "ｍｓ ゴシック" | "ms ゴシック" => {
            out.extend([
                "MS Gothic".to_string(),
                "ＭＳ ゴシック".to_string(),
                "MS UI Gothic".to_string(),
                "Noto Sans CJK JP".to_string(),
                "IPAGothic".to_string(),
                "VL Gothic".to_string(),
            ]);
        }
        "ms mincho" | "ｍｓ 明朝" | "ms 明朝" => {
            out.extend([
                "MS Mincho".to_string(),
                "ＭＳ 明朝".to_string(),
                "Noto Serif CJK JP".to_string(),
                "IPAMincho".to_string(),
            ]);
        }
        "biz udpgothic" | "biz udpゴシック" => {
            out.extend([
                "BIZ UDPGothic".to_string(),
                "BIZ UDPゴシック".to_string(),
                "Noto Sans CJK JP".to_string(),
                "Noto Sans JP".to_string(),
                "IPAexGothic".to_string(),
            ]);
        }
        "biz udpmincho" | "biz udp明朝" => {
            out.extend([
                "BIZ UDPMincho".to_string(),
                "BIZ UDP明朝".to_string(),
                "Noto Serif CJK JP".to_string(),
                "Noto Serif JP".to_string(),
                "IPAexMincho".to_string(),
            ]);
        }
        "noto sans cjk jp" | "noto sans jp" | "noto sans cjk" | "source han sans jp" => {
            out.extend([
                "Noto Sans CJK JP".to_string(),
                "Noto Sans JP".to_string(),
                "Noto Sans CJK".to_string(),
                "Source Han Sans JP".to_string(),
                "Source Han Sans".to_string(),
                "IPAexGothic".to_string(),
                "IPAGothic".to_string(),
                "TakaoPGothic".to_string(),
                "VL PGothic".to_string(),
                "Yu Gothic".to_string(),
                "Meiryo".to_string(),
            ]);
        }
        "noto serif cjk jp" | "noto serif jp" | "noto serif cjk" | "source han serif jp" => {
            out.extend([
                "Noto Serif CJK JP".to_string(),
                "Noto Serif JP".to_string(),
                "Noto Serif CJK".to_string(),
                "Source Han Serif JP".to_string(),
                "Source Han Serif".to_string(),
                "IPAexMincho".to_string(),
                "IPAMincho".to_string(),
                "TakaoMincho".to_string(),
                "Yu Mincho".to_string(),
            ]);
        }
        "consolas" | "dejavu sans mono" | "noto sans mono" | "liberation mono" => {
            out.extend([
                "Consolas".to_string(),
                "DejaVu Sans Mono".to_string(),
                "Noto Sans Mono".to_string(),
                "Liberation Mono".to_string(),
                "Ubuntu Mono".to_string(),
                "JetBrains Mono".to_string(),
            ]);
        }
        _ => {}
    }
    out
}
