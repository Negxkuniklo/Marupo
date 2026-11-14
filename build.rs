use std::env;
use std::fs;
use std::path::PathBuf;

#[allow(dead_code)]
mod icon_gen {
    include!("src/icon.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Save assets/marupo_icon.png for desktop integration / packaging (ignore errors on read-only trees)
    let _ = fs::create_dir_all("assets");
    if let Some(mut pixmap) = tiny_skia::Pixmap::new(256, 256) {
        let rgba = icon_gen::render_mrp_icon_rgba(256);
        pixmap.data_mut().copy_from_slice(&rgba);
        if let Ok(png) = pixmap.encode_png() {
            write_if_changed("assets/marupo_icon.png", &png);
        }
    }

    if target_os == "windows" {
        let ico_bytes = icon_gen::encode_ico_bytes();
        write_if_changed("assets/marupo_icon.ico", &ico_bytes);

        // Build a standard Windows .res file containing RT_ICON and RT_GROUP_ICON
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let res_path = out_dir.join("marupo_icon.res");
        let res_bytes = build_windows_res_from_ico(&ico_bytes);
        fs::write(&res_path, &res_bytes).expect("Failed to write marupo_icon.res");

        println!("cargo:rustc-link-arg-bins={}", res_path.display());
    }
}

fn write_if_changed(path: &str, bytes: &[u8]) {
    if let Ok(existing) = fs::read(path) {
        if existing == bytes {
            return;
        }
    }
    let _ = fs::write(path, bytes);
}

fn build_windows_res_from_ico(ico: &[u8]) -> Vec<u8> {
    let mut res = Vec::new();

    // 1. Empty 32-byte null resource header required at the start of every .res file
    write_res_header(&mut res, 0, 0, 0, 0);

    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    let mut grp_data = Vec::new();
    grp_data.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    grp_data.extend_from_slice(&1u16.to_le_bytes()); // Type = 1
    grp_data.extend_from_slice(&(count as u16).to_le_bytes());

    for i in 0..count {
        let entry_off = 6 + i * 16;
        let entry = &ico[entry_off..entry_off + 16];
        let bytes_in_res = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let img_off = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let img_data = &ico[img_off..img_off + bytes_in_res];

        let icon_id = (i + 1) as u16;
        // RT_ICON = 3
        write_res_header(&mut res, bytes_in_res as u32, 3, icon_id, 0x1030);
        res.extend_from_slice(img_data);
        while res.len() % 4 != 0 {
            res.push(0);
        }

        // GRPICONDIRENTRY (14 bytes: first 12 bytes of ICONDIRENTRY + 2-byte nID)
        grp_data.extend_from_slice(&entry[0..12]);
        grp_data.extend_from_slice(&icon_id.to_le_bytes());
    }

    // RT_GROUP_ICON = 14, ID = 1 (Main application icon in Windows Explorer)
    write_res_header(&mut res, grp_data.len() as u32, 14, 1, 0x1030);
    res.extend_from_slice(&grp_data);
    while res.len() % 4 != 0 {
        res.push(0);
    }

    res
}

fn write_res_header(out: &mut Vec<u8>, data_size: u32, res_type: u16, res_name: u16, memory_flags: u16) {
    out.extend_from_slice(&data_size.to_le_bytes()); // DataSize
    out.extend_from_slice(&32u32.to_le_bytes()); // HeaderSize = 32 bytes
    // TYPE (ordinal: 0xFFFF followed by u16)
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_type.to_le_bytes());
    // NAME (ordinal: 0xFFFF followed by u16)
    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
    out.extend_from_slice(&res_name.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&memory_flags.to_le_bytes()); // MemoryFlags
    out.extend_from_slice(&0x0409u16.to_le_bytes()); // LanguageId (en-US / neutral)
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
}
