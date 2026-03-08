//! WAD asset extractor — reads a Freedoom WAD and produces Rust source files
//! with embedded texture/sprite/palette data for the doom-engine crate.
//!
//! Usage: wad-extract <path-to-freedoom1.wad> <output-dir>

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

// ── WAD structures ──────────────────────────────────────────────────────────

struct WadFile {
    lumps: Vec<Lump>,
    lump_map: HashMap<String, usize>, // name → index
}

struct Lump {
    name: String,
    data: Vec<u8>,
}

fn read_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn read_i16(data: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([data[off], data[off + 1]])
}

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn read_i32(data: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn parse_wad(path: &str) -> io::Result<WadFile> {
    let data = fs::read(path)?;

    let magic = std::str::from_utf8(&data[0..4]).unwrap_or("????");
    assert!(
        magic == "IWAD" || magic == "PWAD",
        "Not a WAD file: magic = {}",
        magic
    );

    let num_lumps = read_i32(&data, 4) as usize;
    let dir_offset = read_i32(&data, 8) as usize;

    eprintln!("WAD: {} lumps, directory at offset {}", num_lumps, dir_offset);

    let mut lumps = Vec::with_capacity(num_lumps);
    let mut lump_map = HashMap::new();

    for i in 0..num_lumps {
        let entry_off = dir_offset + i * 16;
        let filepos = read_i32(&data, entry_off) as usize;
        let size = read_i32(&data, entry_off + 4) as usize;

        let name_bytes = &data[entry_off + 8..entry_off + 16];
        let name: String = name_bytes
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect::<String>()
            .to_uppercase();

        let lump_data = if size > 0 && filepos + size <= data.len() {
            data[filepos..filepos + size].to_vec()
        } else {
            Vec::new()
        };

        lump_map.insert(name.clone(), lumps.len());
        lumps.push(Lump {
            name,
            data: lump_data,
        });
    }

    Ok(WadFile { lumps, lump_map })
}

// ── Picture format decoder ──────────────────────────────────────────────────

/// Decoded picture: palette-indexed pixels with transparency.
struct Picture {
    width: u16,
    height: u16,
    left_offset: i16,
    top_offset: i16,
    /// Row-major palette-indexed pixels. 255 = transparent.
    pixels: Vec<u8>,
    /// Same picture as RGBA (for textures that need compositing).
    rgba: Vec<u8>,
}

/// Decode a DOOM picture-format lump (used for patches, sprites, STBAR).
fn decode_picture(data: &[u8], palette: &[[u8; 3]; 256]) -> Option<Picture> {
    if data.len() < 8 {
        return None;
    }

    let width = read_u16(data, 0);
    let height = read_u16(data, 2);
    let left_offset = read_i16(data, 4);
    let top_offset = read_i16(data, 6);

    if width == 0 || height == 0 || width > 2048 || height > 2048 {
        return None;
    }

    let num_cols = width as usize;
    if data.len() < 8 + num_cols * 4 {
        return None;
    }

    let pixel_count = num_cols * height as usize;
    let mut indexed = vec![255u8; pixel_count]; // 255 = transparent
    let mut rgba = vec![0u8; pixel_count * 4];

    for col in 0..num_cols {
        let col_offset = read_u32(data, 8 + col * 4) as usize;
        if col_offset >= data.len() {
            continue;
        }

        let mut pos = col_offset;
        loop {
            if pos >= data.len() {
                break;
            }
            let top_delta = data[pos];
            if top_delta == 0xFF {
                break;
            }
            pos += 1;
            if pos >= data.len() {
                break;
            }
            let length = data[pos] as usize;
            pos += 1;
            // Skip padding byte
            pos += 1;

            for i in 0..length {
                let row = top_delta as usize + i;
                if row >= height as usize || pos >= data.len() {
                    pos += 1;
                    continue;
                }
                let pal_idx = data[pos] as usize;
                pos += 1;
                let idx = row * num_cols + col;
                if idx < pixel_count {
                    indexed[idx] = pal_idx as u8;
                    let px_off = idx * 4;
                    rgba[px_off] = palette[pal_idx][0];
                    rgba[px_off + 1] = palette[pal_idx][1];
                    rgba[px_off + 2] = palette[pal_idx][2];
                    rgba[px_off + 3] = 255;
                }
            }
            // Skip padding byte
            pos += 1;
        }
    }

    Some(Picture {
        width,
        height,
        left_offset,
        top_offset,
        pixels: indexed,
        rgba,
    })
}

// ── Flat decoder ────────────────────────────────────────────────────────────

/// Decode a flat (64×64 raw palette-indexed floor/ceiling texture).
fn decode_flat(data: &[u8], palette: &[[u8; 3]; 256]) -> Option<Vec<u8>> {
    if data.len() < 4096 {
        return None;
    }
    let mut rgba = vec![0u8; 64 * 64 * 4];
    for i in 0..4096 {
        let pal_idx = data[i] as usize;
        rgba[i * 4] = palette[pal_idx][0];
        rgba[i * 4 + 1] = palette[pal_idx][1];
        rgba[i * 4 + 2] = palette[pal_idx][2];
        rgba[i * 4 + 3] = 255;
    }
    Some(rgba)
}

// ── Main extraction ─────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: wad-extract <freedoom1.wad> <output-dir>");
        std::process::exit(1);
    }

    let wad_path = &args[1];
    let out_dir = &args[2];
    fs::create_dir_all(out_dir)?;

    let wad = parse_wad(wad_path)?;
    eprintln!("Loaded WAD with {} lumps", wad.lumps.len());

    // 1. Extract PLAYPAL (palette 0 — first 768 bytes)
    let palette = extract_palette(&wad);
    eprintln!("Extracted palette (256 RGB colors)");

    // 2. Extract wall textures — we want a handful of distinct ones
    let wall_textures = extract_wall_textures(&wad, &palette);
    eprintln!("Extracted {} wall textures", wall_textures.len());

    // 3. Extract flats (floor/ceiling)
    let flats = extract_flats(&wad, &palette);
    eprintln!("Extracted {} flats", flats.len());

    // 4. Extract sprites (enemies, items, weapons, STBAR)
    let sprites = extract_sprites(&wad, &palette);
    eprintln!("Extracted {} sprites", sprites.len());

    // 5. Extract status bar
    let stbar = extract_stbar(&wad, &palette);

    // 6. Extract STTNUM digit sprites (big red numbers for status bar)
    let sttnum = extract_sttnum(&wad, &palette);
    eprintln!("Extracted {} STTNUM digit sprites", sttnum.len());

    // 7. Write output
    write_palette_rs(out_dir, &palette)?;
    write_textures_rs(out_dir, &wall_textures)?;
    write_flats_rs(out_dir, &flats)?;
    write_sprites_rs(out_dir, &sprites)?;
    if let Some(ref bar) = stbar {
        write_stbar_rs(out_dir, bar)?;
    }
    if !sttnum.is_empty() {
        write_sttnum_rs(out_dir, &sttnum)?;
    }
    write_mod_rs(out_dir, stbar.is_some(), !sttnum.is_empty())?;

    eprintln!("Done! Assets written to {}/", out_dir);
    Ok(())
}

fn extract_palette(wad: &WadFile) -> [[u8; 3]; 256] {
    let mut pal = [[0u8; 3]; 256];
    if let Some(&idx) = wad.lump_map.get("PLAYPAL") {
        let data = &wad.lumps[idx].data;
        for i in 0..256 {
            pal[i][0] = data[i * 3];
            pal[i][1] = data[i * 3 + 1];
            pal[i][2] = data[i * 3 + 2];
        }
    }
    pal
}

/// Extract key wall textures by compositing patches.
fn extract_wall_textures(
    wad: &WadFile,
    palette: &[[u8; 3]; 256],
) -> Vec<(String, u16, u16, Vec<u8>)> {
    // First read PNAMES
    let pnames = read_pnames(wad);

    // Target textures we want (common DOOM wall textures present in Freedoom)
    let wanted = [
        "STARTAN3", "STARG3", "STARGR1", "BROWN1", "BROWNGRN", "STONE2",
        "STONE3", "COMP2", "COMPSPAN", "METAL1", "LITE3", "TEKWALL1",
        "DOOR1", "DOOR3", "DOORTRAK", "EXITDOOR",
    ];

    let mut results = Vec::new();

    // Read TEXTURE1
    if let Some(&idx) = wad.lump_map.get("TEXTURE1") {
        let tex_data = &wad.lumps[idx].data;
        let composited = composite_textures(tex_data, &pnames, wad, palette, &wanted);
        results.extend(composited);
    }

    // Read TEXTURE2 if present
    if let Some(&idx) = wad.lump_map.get("TEXTURE2") {
        let tex_data = &wad.lumps[idx].data;
        let composited = composite_textures(tex_data, &pnames, wad, palette, &wanted);
        results.extend(composited);
    }

    results
}

fn read_pnames(wad: &WadFile) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(&idx) = wad.lump_map.get("PNAMES") {
        let data = &wad.lumps[idx].data;
        let count = read_i32(data, 0) as usize;
        for i in 0..count {
            let off = 4 + i * 8;
            let name: String = data[off..off + 8]
                .iter()
                .take_while(|&&b| b != 0)
                .map(|&b| b as char)
                .collect::<String>()
                .to_uppercase();
            names.push(name);
        }
    }
    names
}

fn composite_textures(
    tex_data: &[u8],
    pnames: &[String],
    wad: &WadFile,
    palette: &[[u8; 3]; 256],
    wanted: &[&str],
) -> Vec<(String, u16, u16, Vec<u8>)> {
    let mut results = Vec::new();
    let num_textures = read_i32(tex_data, 0) as usize;

    for i in 0..num_textures {
        let tex_offset = read_i32(tex_data, 4 + i * 4) as usize;
        if tex_offset + 22 > tex_data.len() {
            continue;
        }

        let name: String = tex_data[tex_offset..tex_offset + 8]
            .iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
            .collect::<String>()
            .to_uppercase();

        if !wanted.iter().any(|w| w.to_uppercase() == name) {
            continue;
        }

        let width = read_i16(tex_data, tex_offset + 12) as u16;
        let height = read_i16(tex_data, tex_offset + 14) as u16;
        let patch_count = read_i16(tex_data, tex_offset + 20) as usize;

        if width == 0 || height == 0 || width > 512 || height > 512 {
            continue;
        }

        let mut rgba = vec![0u8; width as usize * height as usize * 4];

        // Composite each patch
        for p in 0..patch_count {
            let mp_off = tex_offset + 22 + p * 10;
            if mp_off + 10 > tex_data.len() {
                break;
            }
            let origin_x = read_i16(tex_data, mp_off) as i32;
            let origin_y = read_i16(tex_data, mp_off + 2) as i32;
            let patch_idx = read_i16(tex_data, mp_off + 4) as usize;

            if patch_idx >= pnames.len() {
                continue;
            }
            let patch_name = &pnames[patch_idx];

            // Find this patch lump
            if let Some(&lump_idx) = wad.lump_map.get(patch_name) {
                let patch_data = &wad.lumps[lump_idx].data;
                if let Some(pic) = decode_picture(patch_data, palette) {
                    // Blit patch onto texture
                    for py in 0..pic.height as i32 {
                        for px in 0..pic.width as i32 {
                            let dst_x = origin_x + px;
                            let dst_y = origin_y + py;
                            if dst_x < 0
                                || dst_y < 0
                                || dst_x >= width as i32
                                || dst_y >= height as i32
                            {
                                continue;
                            }
                            let src_off = (py as usize * pic.width as usize + px as usize) * 4;
                            if src_off + 3 >= pic.rgba.len() {
                                continue;
                            }
                            if pic.rgba[src_off + 3] == 0 {
                                continue; // transparent
                            }
                            let dst_off = (dst_y as usize * width as usize + dst_x as usize) * 4;
                            if dst_off + 3 < rgba.len() {
                                rgba[dst_off] = pic.rgba[src_off];
                                rgba[dst_off + 1] = pic.rgba[src_off + 1];
                                rgba[dst_off + 2] = pic.rgba[src_off + 2];
                                rgba[dst_off + 3] = 255;
                            }
                        }
                    }
                }
            }
        }

        eprintln!("  Wall texture: {} ({}×{})", name, width, height);
        results.push((name, width, height, rgba));
    }

    results
}

/// Extract floor/ceiling flats.
fn extract_flats(wad: &WadFile, palette: &[[u8; 3]; 256]) -> Vec<(String, Vec<u8>)> {
    let wanted = [
        "FLOOR0_1", "FLOOR0_3", "FLOOR4_8", "FLOOR5_1", "FLAT1",
        "FLAT5_4", "CEIL3_5", "NUKAGE1",
    ];

    let mut results = Vec::new();

    // Find lumps between F_START and F_END
    let f_start = wad.lump_map.get("F_START").or(wad.lump_map.get("FF_START"));
    let f_end = wad.lump_map.get("F_END").or(wad.lump_map.get("FF_END"));

    if let (Some(&start), Some(&end)) = (f_start, f_end) {
        for i in (start + 1)..end {
            let lump = &wad.lumps[i];
            if !wanted.iter().any(|w| w.to_uppercase() == lump.name) {
                continue;
            }
            if let Some(rgba) = decode_flat(&lump.data, palette) {
                eprintln!("  Flat: {} (64×64)", lump.name);
                results.push((lump.name.clone(), rgba));
            }
        }
    }

    results
}

/// Extract enemy, item, weapon, and HUD sprites.
fn extract_sprites(wad: &WadFile, palette: &[[u8; 3]; 256]) -> Vec<(String, u16, u16, i16, i16, Vec<u8>)> {
    // Key sprites for our 3 enemy types + items + weapon
    // Format: PREFIX + FRAME + ROTATION (e.g., TROOA1 = Imp frame A angle 1)
    let wanted_prefixes = [
        // Enemies
        "TROO", // Imp
        "SARG", // Demon
        "SPOS", // Sergeant (Shotgun Guy)
        // Items
        "STIM", // Stimpack (health)
        "MEDI", // Medikit
        "CLIP", // Ammo clip
        "AMMO", // Ammo box
        "ARM1", // Green armor
        "RKEY", // Red key
        "BKEY", // Blue key
        // Weapons
        "PISG", // Pistol
        "PUNG", // Fist
        "SHTG", // Shotgun
        "SHEL", // Shell box
        "SHOT", // Shotgun pickup
        // Decorative props
        "BAR1", // Barrel (explosive)
        "TLMP", // Tall tech lamp
        "TLP2", // Short tech lamp
        "COLU", // Column
        "CBRA", // Candelabra
        "CAND", // Candlestick
        "SMIT", // Dead player (bloody mess)
        "POL1", // Pile of skulls and candles
        "POL2", // Skullpile
        "POL3", // Skull column
        "POL4", // Skull on stick
        "POL6", // Hanging twitching
        "GOR1", // Hanging body
        "TBLU", // Tall blue torch
        "TGRN", // Tall green torch
        "TRED", // Tall red torch
        "SMBT", // Short blue torch
        "SMGT", // Short green torch
        "SMRT", // Short red torch
        "CEYE", // Evil eye
        "FSKU", // Floating skull rock
        "ELEC", // Tech pillar
        "COL1", // Tall green pillar
        "COL2", // Short green pillar
        "COL3", // Tall red pillar
        "COL4", // Short red pillar
        "COL5", // Heart column
        "COL6", // Skull column short
        // Player
        "PLAY", // Player sprites
    ];

    // Sprites we need — enemies get all 8 rotations for walk A+B, plus attack/pain/death
    let wanted_lumps = [
        // Imp — all 8 rotations for walk frames, plus action frames
        "TROOA1", "TROOA2A8", "TROOA3A7", "TROOA4A6", "TROOA5",
        "TROOB1", "TROOB2B8", "TROOB3B7", "TROOB4B6", "TROOB5",
        "TROOC1", "TROOE1", "TROOH1",
        // Demon — all 8 rotations for walk frames
        "SARGA1", "SARGA2A8", "SARGA3A7", "SARGA4A6", "SARGA5",
        "SARGB1", "SARGB2B8", "SARGB3B7", "SARGB4B6", "SARGB5",
        "SARGC1", "SARGE1", "SARGH1",
        // Sergeant — all 8 rotations for walk frames
        "SPOSA1", "SPOSA2A8", "SPOSA3A7", "SPOSA4A6", "SPOSA5",
        "SPOSB1", "SPOSB2B8", "SPOSB3B7", "SPOSB4B6", "SPOSB5",
        "SPOSC1", "SPOSE1", "SPOSH0",
        // Items
        "STIMA0", "MEDIA0", "CLIPA0", "AMMOA0", "ARM1A0",
        "RKEYA0", "BKEYA0",
        // Weapons
        "PISGA0", "PISGB0", "PISGC0", "PISGD0", "PISGE0",
        "PUNGA0", "PUNGB0", "PUNGC0", "PUNGD0", "PUNGE0",
        "SHTGA0", "SHTGB0", "SHTGC0", "SHTGD0", "SHTGE0",
        "SHELA0", "SHOTA0",
        // Imp fireball
        "BAL1A0", "BAL1B0",
        // Decorative props — two frames for animated ones
        "BAR1A0", "BAR1B0",       // Barrel
        "TLMPA0", "TLMPB0",       // Tall tech lamp
        "TLP2A0", "TLP2B0",       // Short tech lamp
        "COLUA0",                   // Column
        "CBRAA0",                   // Candelabra
        "CANDA0",                   // Candlestick
        "SMITA0",                   // Dead player
        "POL1A0",                   // Pile of skulls
        "POL2A0",                   // Skullpile
        "POL3A0", "POL3B0",       // Skull column
        "POL4A0",                   // Skull on stick
        "POL6A0", "POL6B0",       // Hanging twitching
        "GOR1A0",                   // Hanging body
        "TBLUA0", "TBLUB0",       // Tall blue torch
        "TGRNA0", "TGRNB0",       // Tall green torch
        "TREDA0", "TREDB0",       // Tall red torch
        "SMBTA0", "SMBTB0",       // Short blue torch
        "SMGTA0", "SMGTB0",       // Short green torch
        "SMRTA0", "SMRTB0",       // Short red torch
        "CEYEA0", "CEYEB0",       // Evil eye
        "FSKUA0", "FSKUB0",       // Floating skull rock
        "ELECA0",                   // Tech pillar
        "COL1A0",                   // Tall green pillar
        "COL2A0",                   // Short green pillar
        "COL3A0",                   // Tall red pillar
        "COL4A0",                   // Short red pillar
        "COL5A0",                   // Heart column
        "COL6A0",                   // Skull column short
    ];

    let mut results = Vec::new();

    // Find sprites between S_START and S_END
    let s_start = wad.lump_map.get("S_START").or(wad.lump_map.get("SS_START"));
    let s_end = wad.lump_map.get("S_END").or(wad.lump_map.get("SS_END"));

    if let (Some(&start), Some(&end)) = (s_start, s_end) {
        for i in (start + 1)..end {
            let lump = &wad.lumps[i];
            if !wanted_lumps.iter().any(|w| *w == lump.name) {
                continue;
            }

            if let Some(pic) = decode_picture(&lump.data, palette) {
                eprintln!(
                    "  Sprite: {} ({}×{}, offset {}, {})",
                    lump.name, pic.width, pic.height, pic.left_offset, pic.top_offset
                );
                results.push((
                    lump.name.clone(),
                    pic.width,
                    pic.height,
                    pic.left_offset,
                    pic.top_offset,
                    pic.pixels,
                ));
            }
        }
    } else {
        eprintln!("WARNING: No S_START/S_END markers found!");
        // Try to find sprites by name directly
        for name in &wanted_lumps {
            if let Some(&idx) = wad.lump_map.get(*name) {
                let lump = &wad.lumps[idx];
                if let Some(pic) = decode_picture(&lump.data, palette) {
                    eprintln!("  Sprite (direct): {} ({}×{})", name, pic.width, pic.height);
                    results.push((
                        name.to_string(),
                        pic.width,
                        pic.height,
                        pic.left_offset,
                        pic.top_offset,
                        pic.pixels,
                    ));
                }
            }
        }
    }

    results
}

/// Extract the STBAR (status bar background).
fn extract_stbar(wad: &WadFile, palette: &[[u8; 3]; 256]) -> Option<Picture> {
    // Try STBAR first, then STBAR2
    for name in &["STBAR", "STBAR2"] {
        if let Some(&idx) = wad.lump_map.get(*name) {
            if let Some(pic) = decode_picture(&wad.lumps[idx].data, palette) {
                eprintln!("  Status bar: {} ({}×{})", name, pic.width, pic.height);
                return Some(pic);
            }
        }
    }

    None
}

/// Extract STTNUM0-9 and STTPRCNT (big red digit sprites for status bar).
/// These are regular WAD lumps (not in S_START/S_END), parsed as picture format.
fn extract_sttnum(wad: &WadFile, palette: &[[u8; 3]; 256]) -> Vec<(String, Picture)> {
    let names = [
        "STTNUM0", "STTNUM1", "STTNUM2", "STTNUM3", "STTNUM4",
        "STTNUM5", "STTNUM6", "STTNUM7", "STTNUM8", "STTNUM9",
        "STTPRCNT", "STTMINUS",
    ];

    let mut results = Vec::new();
    for name in &names {
        if let Some(&idx) = wad.lump_map.get(*name) {
            if let Some(pic) = decode_picture(&wad.lumps[idx].data, palette) {
                eprintln!(
                    "  STTNUM: {} ({}×{}, offset {}, {})",
                    name, pic.width, pic.height, pic.left_offset, pic.top_offset
                );
                results.push((name.to_string(), pic));
            }
        }
    }
    results
}

// ── Output writers ──────────────────────────────────────────────────────────

fn write_palette_rs(dir: &str, palette: &[[u8; 3]; 256]) -> io::Result<()> {
    let path = format!("{}/palette.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// Freedoom PLAYPAL palette (256 RGB triplets).")?;
    writeln!(f, "pub const PALETTE: [[u8; 3]; 256] = [")?;
    for (i, c) in palette.iter().enumerate() {
        if i % 8 == 0 {
            write!(f, "    ")?;
        }
        write!(f, "[{}, {}, {}], ", c[0], c[1], c[2])?;
        if i % 8 == 7 {
            writeln!(f)?;
        }
    }
    writeln!(f, "];")?;
    Ok(())
}

fn write_textures_rs(
    dir: &str,
    textures: &[(String, u16, u16, Vec<u8>)],
) -> io::Result<()> {
    let path = format!("{}/textures.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// Wall textures extracted from Freedoom WAD.")?;
    writeln!(f, "/// Each texture is (name, width, height, RGBA data).")?;
    writeln!(f)?;

    // Write each texture as a const
    for (name, w, h, rgba) in textures {
        let const_name = name.replace('-', "_").to_uppercase();
        writeln!(f, "pub const TEX_{}_W: u16 = {};", const_name, w)?;
        writeln!(f, "pub const TEX_{}_H: u16 = {};", const_name, h)?;
        writeln!(
            f,
            "pub const TEX_{}: &[u8] = &{:?};",
            const_name,
            compress_rgba(rgba)
        )?;
        writeln!(f)?;
    }

    // Write texture list
    writeln!(f, "pub struct WallTexture {{")?;
    writeln!(f, "    pub name: &'static str,")?;
    writeln!(f, "    pub width: u16,")?;
    writeln!(f, "    pub height: u16,")?;
    writeln!(f, "    pub data: &'static [u8],")?;
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "pub const WALL_TEXTURES: &[WallTexture] = &[")?;
    for (name, _, _, _) in textures {
        let cn = name.replace('-', "_").to_uppercase();
        writeln!(
            f,
            "    WallTexture {{ name: \"{}\", width: TEX_{}_W, height: TEX_{}_H, data: TEX_{} }},",
            name, cn, cn, cn
        )?;
    }
    writeln!(f, "];")?;

    Ok(())
}

fn write_flats_rs(dir: &str, flats: &[(String, Vec<u8>)]) -> io::Result<()> {
    let path = format!("{}/flats.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// Floor/ceiling textures (64×64 RGBA) from Freedoom.")?;
    writeln!(f)?;

    for (name, rgba) in flats {
        let cn = name.replace('-', "_").replace('.', "_").to_uppercase();
        writeln!(
            f,
            "pub const FLAT_{}: &[u8] = &{:?};",
            cn,
            compress_rgba(rgba)
        )?;
    }

    writeln!(f)?;
    writeln!(f, "pub struct Flat {{")?;
    writeln!(f, "    pub name: &'static str,")?;
    writeln!(f, "    pub data: &'static [u8],")?;
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "pub const FLATS: &[Flat] = &[")?;
    for (name, _) in flats {
        let cn = name.replace('-', "_").replace('.', "_").to_uppercase();
        writeln!(
            f,
            "    Flat {{ name: \"{}\", data: FLAT_{} }},",
            name, cn
        )?;
    }
    writeln!(f, "];")?;

    Ok(())
}

fn write_sprites_rs(
    dir: &str,
    sprites: &[(String, u16, u16, i16, i16, Vec<u8>)],
) -> io::Result<()> {
    let path = format!("{}/sprites.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// Sprites extracted from Freedoom WAD.")?;
    writeln!(f)?;

    for (name, w, h, lx, ty, rgba) in sprites {
        let cn = name.replace('-', "_").to_uppercase();
        writeln!(f, "pub const SPR_{}_W: u16 = {};", cn, w)?;
        writeln!(f, "pub const SPR_{}_H: u16 = {};", cn, h)?;
        writeln!(f, "pub const SPR_{}_LX: i16 = {};", cn, lx)?;
        writeln!(f, "pub const SPR_{}_TY: i16 = {};", cn, ty)?;
        writeln!(
            f,
            "pub const SPR_{}: &[u8] = &{:?};",
            cn,
            compress_rgba(rgba)
        )?;
        writeln!(f)?;
    }

    writeln!(f, "pub struct Sprite {{")?;
    writeln!(f, "    pub name: &'static str,")?;
    writeln!(f, "    pub width: u16,")?;
    writeln!(f, "    pub height: u16,")?;
    writeln!(f, "    pub left_offset: i16,")?;
    writeln!(f, "    pub top_offset: i16,")?;
    writeln!(f, "    pub data: &'static [u8],")?;
    writeln!(f, "}}")?;
    writeln!(f)?;
    writeln!(f, "pub const SPRITES: &[Sprite] = &[")?;
    for (name, _, _, _, _, _) in sprites {
        let cn = name.replace('-', "_").to_uppercase();
        writeln!(
            f,
            "    Sprite {{ name: \"{}\", width: SPR_{}_W, height: SPR_{}_H, left_offset: SPR_{}_LX, top_offset: SPR_{}_TY, data: SPR_{} }},",
            name, cn, cn, cn, cn, cn
        )?;
    }
    writeln!(f, "];")?;

    Ok(())
}

fn write_stbar_rs(dir: &str, pic: &Picture) -> io::Result<()> {
    let path = format!("{}/stbar.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// Status bar background from Freedoom WAD.")?;
    writeln!(f, "pub const STBAR_W: u16 = {};", pic.width)?;
    writeln!(f, "pub const STBAR_H: u16 = {};", pic.height)?;
    writeln!(
        f,
        "pub const STBAR: &[u8] = &{:?};",
        compress_rgba(&pic.pixels)
    )?;
    Ok(())
}

fn write_sttnum_rs(dir: &str, digits: &[(String, Picture)]) -> io::Result<()> {
    let path = format!("{}/sttnum.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "/// STTNUM digit sprites from Freedoom WAD (big red numbers for status bar).")?;
    writeln!(f)?;

    for (name, pic) in digits {
        let cn = name.to_uppercase();
        writeln!(f, "pub const {}_W: u16 = {};", cn, pic.width)?;
        writeln!(f, "pub const {}_H: u16 = {};", cn, pic.height)?;
        writeln!(
            f,
            "pub const {}: &[u8] = &{:?};",
            cn,
            compress_rgba(&pic.pixels)
        )?;
        writeln!(f)?;
    }

    // Write a lookup array for digits 0-9
    writeln!(f, "pub struct SttDigit {{")?;
    writeln!(f, "    pub width: u16,")?;
    writeln!(f, "    pub height: u16,")?;
    writeln!(f, "    pub data: &'static [u8],")?;
    writeln!(f, "}}")?;
    writeln!(f)?;

    writeln!(f, "/// Digits 0-9, indexed by digit value.")?;
    writeln!(f, "pub const DIGITS: &[SttDigit] = &[")?;
    for i in 0..10 {
        let name = format!("STTNUM{}", i);
        writeln!(
            f,
            "    SttDigit {{ width: {}_W, height: {}_H, data: {} }},",
            name, name, name
        )?;
    }
    writeln!(f, "];")?;
    writeln!(f)?;

    // Percent sign
    if digits.iter().any(|(n, _)| n == "STTPRCNT") {
        writeln!(f, "pub const PERCENT: SttDigit = SttDigit {{ width: STTPRCNT_W, height: STTPRCNT_H, data: STTPRCNT }};")?;
    }

    // Minus sign
    if digits.iter().any(|(n, _)| n == "STTMINUS") {
        writeln!(f, "pub const MINUS: SttDigit = SttDigit {{ width: STTMINUS_W, height: STTMINUS_H, data: STTMINUS }};")?;
    }

    Ok(())
}

fn write_mod_rs(dir: &str, has_stbar: bool, has_sttnum: bool) -> io::Result<()> {
    let path = format!("{}/mod.rs", dir);
    let mut f = fs::File::create(&path)?;
    writeln!(f, "//! Freedoom WAD assets — auto-generated by wad-extract.")?;
    writeln!(f, "pub mod palette;")?;
    writeln!(f, "pub mod textures;")?;
    writeln!(f, "pub mod flats;")?;
    writeln!(f, "pub mod sprites;")?;
    if has_stbar {
        writeln!(f, "pub mod stbar;")?;
    }
    if has_sttnum {
        writeln!(f, "pub mod sttnum;")?;
    }
    Ok(())
}

/// Simple RLE compression for RGBA data to reduce binary size.
/// Format: pairs of (count, r, g, b, a) where count is 1-255.
/// This is very basic but cuts down on repetitive pixels.
fn compress_rgba(rgba: &[u8]) -> Vec<u8> {
    // For simplicity in the first pass, just store raw palette-indexed data.
    // Full RLE would be more complex. Just return the raw RGBA for now.
    // The compiler's LTO and wasm-opt will handle deduplication.
    rgba.to_vec()
}
