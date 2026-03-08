use crate::assets::faces;
use crate::assets::flats;
use crate::assets::palette::PALETTE;
use crate::assets::sprites;
use crate::assets::stbar;
use crate::assets::sttnum;
use crate::assets::textures;
use crate::game::GameState;
use crate::map::RayHitType;
use crate::types::*;

/// Screen dimensions — classic DOOM resolution.
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 200;
/// Game viewport height (above the 32px status bar).
pub const VIEW_HEIGHT: usize = 168;
pub const FRAMEBUFFER_SIZE: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

/// Field of view in milliradians (~60 degrees).
const FOV: i32 = 1047; // π/3 × 1000
const HALF_FOV: i32 = 523;

/// Flat texture dimensions.
const FLAT_SIZE: usize = 64;

/// Depth buffer — one entry per screen column.
pub type DepthBuffer = [i32; SCREEN_WIDTH];

/// The framebuffer — RGBA pixels, row-major.
/// Pure data, no I/O — designed to move on-chain.
#[derive(Clone)]
pub struct Framebuffer {
    /// RGBA pixels: 4 bytes per pixel, row-major.
    pub rgba: Vec<u8>,
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            rgba: vec![0u8; FRAMEBUFFER_SIZE * 4],
        }
    }

    #[inline]
    fn set_rgb(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            let off = (y * SCREEN_WIDTH + x) * 4;
            self.rgba[off] = r;
            self.rgba[off + 1] = g;
            self.rgba[off + 2] = b;
            self.rgba[off + 3] = 255;
        }
    }

    #[inline]
    fn set_rgb_lit(&mut self, x: usize, y: usize, r: u8, g: u8, b: u8, bright: u8) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            let off = (y * SCREEN_WIDTH + x) * 4;
            let br = bright as u32;
            self.rgba[off] = (r as u32 * br / 255) as u8;
            self.rgba[off + 1] = (g as u32 * br / 255) as u8;
            self.rgba[off + 2] = (b as u32 * br / 255) as u8;
            self.rgba[off + 3] = 255;
        }
    }

    /// Set pixel from palette index with brightness.
    #[inline]
    fn set_pal(&mut self, x: usize, y: usize, pal_idx: u8, bright: u8) {
        let c = &PALETTE[pal_idx as usize];
        self.set_rgb_lit(x, y, c[0], c[1], c[2], bright);
    }

    /// Set pixel from palette index, full brightness.
    #[inline]
    fn set_pal_full(&mut self, x: usize, y: usize, pal_idx: u8) {
        let c = &PALETTE[pal_idx as usize];
        self.set_rgb(x, y, c[0], c[1], c[2]);
    }
}

/// Reference height: 64 sprite pixels = 1 world tile (FP_SCALE units).
/// This matches Doom's internal scale where textures are 64px per tile.
const SPRITE_PIXELS_PER_TILE: i32 = 64;

/// Tall torch/pillar sprites need extra scaling since they're ~96px tall
/// (designed for Doom's 128-unit rooms) but our rooms are only 1 tile.
const TORCH_PIXELS_PER_TILE: i32 = 128;

/// Sprite to render — computed during the render pass.
struct SpriteRender {
    screen_x: i32,
    distance: i32,
    sprite_name: &'static str,
    floor_height: i32, // sector floor height for sprite anchoring
}

/// Per-column ray result cached for floor/ceiling casting.
struct ColumnRay {
    dir_x: f64,
    dir_y: f64,
}

/// Per-column wall rendering info saved from ray pass.
struct WallColumn {
    tex_idx: usize,
    tex_u: usize,
    draw_start: usize,
    draw_end: usize,
    proj_top: i64,
    wall_height: usize,
    bright: u8,
    has_hit: bool,
    hit_sector_has_sky: bool, // true if the wall's sector has ceiling_tex == 255
}

/// Render a complete frame from the game state.
/// Pure computation — no I/O. Designed for on-chain execution.
pub fn render_frame(state: &GameState, fb: &mut Framebuffer) {
    let view_mid = VIEW_HEIGHT as i32 / 2;
    let player_gx = (state.player.x / FP_SCALE) as u32;
    let player_gy = (state.player.y / FP_SCALE) as u32;
    let player_sector = state.map.get_sector(player_gx, player_gy);
    let player_floor = player_sector.floor_height;

    // 1. Cast rays — compute wall info per column (don't draw yet)
    let mut depth_buf: DepthBuffer = [i32::MAX; SCREEN_WIDTH];
    let mut col_rays: Vec<ColumnRay> = Vec::with_capacity(SCREEN_WIDTH);
    let mut wall_cols: Vec<WallColumn> = Vec::with_capacity(SCREEN_WIDTH);

    for col in 0..SCREEN_WIDTH {
        let ray_offset = (col as i32 * FOV / SCREEN_WIDTH as i32) - HALF_FOV;
        let ray_angle = normalize_angle(state.player.angle + ray_offset);
        let ray_rad = ray_angle as f64 / 1000.0;
        let dir_x = ray_rad.cos();
        let dir_y = ray_rad.sin();

        let hit = state.map.cast_ray(state.player.x, state.player.y, ray_angle);
        let dist = hit.distance.max(1);
        depth_buf[col] = dist;

        let hit_sector = state.map.get_sector(hit.grid_x, hit.grid_y);
        let eye_height = player_floor + 500;
        let wall_top = hit_sector.ceiling_height - eye_height;
        let wall_bot = hit_sector.floor_height - eye_height;

        let scale = VIEW_HEIGHT as i64 * FP_SCALE as i64 / dist as i64;
        let proj_top = view_mid as i64 - (wall_top as i64 * scale / FP_SCALE as i64);
        let proj_bot = view_mid as i64 - (wall_bot as i64 * scale / FP_SCALE as i64);

        let draw_start = (proj_top.max(0) as usize).min(VIEW_HEIGHT);
        let draw_end = (proj_bot.min(VIEW_HEIGHT as i64) as usize).min(VIEW_HEIGHT);

        let dist_bright = distance_brightness(dist);
        let sector_bright = hit_sector.effective_light(state.tick);
        let bright = combine_brightness(dist_bright, sector_bright);
        let bright = if hit.side == 1 {
            (bright as u32 * 192 / 256) as u8
        } else {
            bright
        };

        col_rays.push(ColumnRay { dir_x, dir_y });

        let has_hit = !matches!(hit.hit_type, RayHitType::None);
        let tex_idx = if has_hit {
            match hit.hit_type {
                RayHitType::Wall => (hit.texture as usize) % textures::WALL_TEXTURES.len(),
                RayHitType::Door => find_texture_index("DOOR1").unwrap_or(4),
                RayHitType::None => 0,
            }
        } else {
            0
        };

        let tex = &textures::WALL_TEXTURES[tex_idx];
        let tex_w = tex.width as usize;
        let tex_u = if has_hit {
            ((hit.wall_x_frac as usize * tex_w) / FP_SCALE as usize) % tex_w
        } else {
            0
        };

        wall_cols.push(WallColumn {
            tex_idx,
            tex_u,
            draw_start,
            draw_end,
            proj_top,
            wall_height: (proj_bot - proj_top).max(1) as usize,
            bright,
            has_hit,
            hit_sector_has_sky: hit_sector.ceiling_tex == 255,
        });
    }

    // 2. Render floor/ceiling for FULL viewport (no gaps at height transitions)
    render_floors_ceilings_full(state, fb, &col_rays, &wall_cols);

    // 3. Draw walls ON TOP of floor/ceiling
    for col in 0..SCREEN_WIDTH {
        let wc = &wall_cols[col];
        if !wc.has_hit { continue; }

        let tex = &textures::WALL_TEXTURES[wc.tex_idx];
        let tex_w = tex.width as usize;
        let tex_h = tex.height as usize;

        for y in wc.draw_start..wc.draw_end {
            let y_in_wall = (y as i64 - wc.proj_top) as usize;
            let tex_v = if wc.wall_height > 0 {
                (y_in_wall * tex_h / wc.wall_height) % tex_h
            } else {
                0
            };

            let px_off = (tex_v * tex_w + wc.tex_u) * 4;
            if px_off + 3 < tex.data.len() {
                let r = tex.data[px_off];
                let g = tex.data[px_off + 1];
                let b = tex.data[px_off + 2];
                fb.set_rgb_lit(col, y, r, g, b, wc.bright);
            }
        }
    }

    // 3b. Sky cleanup — fill any remaining ceiling gaps above sky-sector walls
    for col in 0..SCREEN_WIDTH {
        let wc = &wall_cols[col];
        if wc.hit_sector_has_sky {
            for y in 0..wc.draw_start {
                render_sky_pixel(fb, col, y, state.player.angle, view_mid as usize);
            }
        }
    }

    // 4. Render sprites (enemies + items + projectiles + decorations)
    render_sprites(state, fb, &depth_buf, player_floor);

    // 5. Render weapon overlay
    render_weapon(state, fb);

    // 6. Draw status bar
    render_stbar(state, fb);

    // 6.5 Automap overlay (Tab toggle)
    if state.player.show_automap {
        render_automap(state, fb);
    }

    // 7. Death/victory overlay
    if state.game_over {
        // Red tint over viewport
        for y in 0..VIEW_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let off = (y * SCREEN_WIDTH + x) * 4;
                if off + 3 < fb.rgba.len() {
                    let r = fb.rgba[off];
                    let g = fb.rgba[off + 1];
                    let b = fb.rgba[off + 2];
                    fb.rgba[off] = (r as u32 * 180 / 255 + 75).min(255) as u8;
                    fb.rgba[off + 1] = g / 3;
                    fb.rgba[off + 2] = b / 3;
                }
            }
        }
        draw_small_text(fb, "PRESS ANY KEY", 160, VIEW_HEIGHT / 2, 200, 200, 200);
    } else if state.level_complete {
        // Green tint over viewport
        for y in 0..VIEW_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let off = (y * SCREEN_WIDTH + x) * 4;
                if off + 3 < fb.rgba.len() {
                    let r = fb.rgba[off];
                    let g = fb.rgba[off + 1];
                    let b = fb.rgba[off + 2];
                    fb.rgba[off] = r / 3;
                    fb.rgba[off + 1] = (g as u32 * 180 / 255 + 75).min(255) as u8;
                    fb.rgba[off + 2] = b / 3;
                }
            }
        }
        draw_small_text(fb, "PRESS ANY KEY", 160, VIEW_HEIGHT / 2, 200, 200, 200);
    }
}

/// Render textured floors and ceilings for the FULL viewport.
/// Renders ceiling for all rows above midpoint and floor for all rows below.
/// Walls are drawn ON TOP afterward, eliminating gaps at sector height transitions.
fn render_floors_ceilings_full(
    state: &GameState,
    fb: &mut Framebuffer,
    col_rays: &[ColumnRay],
    wall_cols: &[WallColumn],
) {
    let view_mid = VIEW_HEIGHT as i32 / 2;
    let px = state.player.x as f64 / FP_SCALE as f64;
    let py = state.player.y as f64 / FP_SCALE as f64;

    for col in 0..SCREEN_WIDTH {
        let cr = &col_rays[col];

        // --- Floor (entire bottom half of viewport) ---
        for y in (view_mid as usize)..VIEW_HEIGHT {
            let row_dist_from_mid = y as i32 - view_mid;
            if row_dist_from_mid <= 0 { continue; }

            let floor_dist = (VIEW_HEIGHT as f64 / 2.0) / row_dist_from_mid as f64;
            let world_x = px + cr.dir_x * floor_dist;
            let world_y = py + cr.dir_y * floor_dist;

            let gx = world_x.floor() as i32;
            let gy = world_y.floor() as i32;

            if gx < 0 || gy < 0 || gx >= state.map.width as i32 || gy >= state.map.height as i32 {
                fb.set_rgb(col, y, 20, 20, 20);
                continue;
            }

            let sector = state.map.get_sector(gx as u32, gy as u32);
            let tx = ((world_x - world_x.floor()) * FLAT_SIZE as f64) as usize % FLAT_SIZE;
            let ty = ((world_y - world_y.floor()) * FLAT_SIZE as f64) as usize % FLAT_SIZE;

            let flat_idx = (sector.floor_tex as usize).min(flats::FLATS.len() - 1);
            let flat = &flats::FLATS[flat_idx];

            let pixel_off = (ty * FLAT_SIZE + tx) * 4;
            if pixel_off + 3 < flat.data.len() {
                let dist_fp = (floor_dist * FP_SCALE as f64) as i32;
                let bright = combine_brightness(
                    distance_brightness(dist_fp),
                    sector.effective_light(state.tick),
                );
                fb.set_rgb_lit(col, y, flat.data[pixel_off], flat.data[pixel_off + 1], flat.data[pixel_off + 2], bright);
            }
        }

        // --- Ceiling (entire top half of viewport) ---
        // DOOM sky logic: if the wall hit for this column is in a sky sector,
        // render sky for all ceiling pixels above that wall. Otherwise use flats.
        let wc = &wall_cols[col];
        let col_has_sky = wc.hit_sector_has_sky;

        for y in 0..(view_mid as usize) {
            if col_has_sky {
                // Sky above — render sky for pixels above the wall
                render_sky_pixel(fb, col, y, state.player.angle, view_mid as usize);
                continue;
            }

            let row_dist_from_mid = view_mid - y as i32;
            if row_dist_from_mid <= 0 { continue; }

            let ceil_dist = (VIEW_HEIGHT as f64 / 2.0) / row_dist_from_mid as f64;
            let world_x = px + cr.dir_x * ceil_dist;
            let world_y = py + cr.dir_y * ceil_dist;

            let gx = world_x.floor() as i32;
            let gy = world_y.floor() as i32;

            if gx < 0 || gy < 0 || gx >= state.map.width as i32 || gy >= state.map.height as i32 {
                fb.set_rgb(col, y, 20, 20, 20);
                continue;
            }

            let sector = state.map.get_sector(gx as u32, gy as u32);

            // Sky sector check — ceiling_tex 255 means sky
            // Only apply if ceiling ray is closer than the wall hit for this column
            // (prevents sky bleeding through non-sky walls from sectors behind them)
            let wall_dist_approx = if wc.wall_height > 0 {
                VIEW_HEIGHT as f64 / wc.wall_height as f64
            } else {
                f64::MAX
            };
            if sector.ceiling_tex == 255 && ceil_dist <= wall_dist_approx {
                render_sky_pixel(fb, col, y, state.player.angle, view_mid as usize);
                continue;
            }

            let tx = ((world_x - world_x.floor()) * FLAT_SIZE as f64) as usize % FLAT_SIZE;
            let ty = ((world_y - world_y.floor()) * FLAT_SIZE as f64) as usize % FLAT_SIZE;

            let flat_idx = (sector.ceiling_tex as usize).min(flats::FLATS.len() - 1);
            let flat = &flats::FLATS[flat_idx];

            let pixel_off = (ty * FLAT_SIZE + tx) * 4;
            if pixel_off + 3 < flat.data.len() {
                let dist_fp = (ceil_dist * FP_SCALE as f64) as i32;
                let bright = combine_brightness(
                    distance_brightness(dist_fp),
                    sector.effective_light(state.tick),
                );
                fb.set_rgb_lit(col, y, flat.data[pixel_off], flat.data[pixel_off + 1], flat.data[pixel_off + 2], bright);
            }
        }
    }
}

fn find_texture_index(name: &str) -> Option<usize> {
    textures::WALL_TEXTURES
        .iter()
        .position(|t| t.name == name)
}

/// Find a sprite by name in the extracted assets.
fn find_sprite(name: &str) -> Option<&'static sprites::Sprite> {
    sprites::SPRITES.iter().find(|s| s.name == name)
}

/// Get the sprite name for a decoration type, with animation frame.
fn decoration_sprite_name(deco_type: DecorationType, tick: u64) -> &'static str {
    let frame_b = tick % 8 >= 4; // toggle every 4 ticks for animated sprites
    match deco_type {
        DecorationType::Barrel => if frame_b { "BAR1B0" } else { "BAR1A0" },
        DecorationType::Column => "COLUA0",
        DecorationType::Candelabra => "CBRAA0",
        DecorationType::Candlestick => "CANDA0",
        DecorationType::TallBlueTorch => if frame_b { "TBLUB0" } else { "TBLUA0" },
        DecorationType::TallGreenTorch => if frame_b { "TGRNB0" } else { "TGRNA0" },
        DecorationType::TallRedTorch => if frame_b { "TREDB0" } else { "TREDA0" },
        DecorationType::ShortBlueTorch => if frame_b { "SMBTB0" } else { "SMBTA0" },
        DecorationType::ShortGreenTorch => if frame_b { "SMGTB0" } else { "SMGTA0" },
        DecorationType::ShortRedTorch => if frame_b { "SMRTB0" } else { "SMRTA0" },
        DecorationType::EvilEye => if frame_b { "CEYEB0" } else { "CEYEA0" },
        DecorationType::FloatingSkull => if frame_b { "FSKUB0" } else { "FSKUA0" },
        DecorationType::TechPillar => "ELECA0",
        DecorationType::TallGreenPillar => "COL1A0",
        DecorationType::ShortGreenPillar => "COL2A0",
        DecorationType::TallRedPillar => "COL3A0",
        DecorationType::ShortRedPillar => "COL4A0",
        DecorationType::HeartColumn => "COL5A0",
        DecorationType::SkullColumn => "COL6A0",
        DecorationType::SkullPile => "POL2A0",
        DecorationType::SkullsAndCandles => "POL1A0",
        DecorationType::SkullColumnTall => if frame_b { "POL3B0" } else { "POL3A0" },
        DecorationType::SkullOnStick => "POL4A0",
        DecorationType::HangingTwitching => if frame_b { "POL6B0" } else { "POL6A0" },
        DecorationType::HangingBody => "GOR1A0",
        DecorationType::DeadPlayer => "SMITA0",
    }
}

/// Get the sprite name for an enemy based on type, AI state, and rotation angle.
/// Rotation is 1-8 where 1=facing viewer, 5=facing away.
fn enemy_sprite_name_rotated(enemy_type: EnemyType, ai_state: &EnemyAiState, rotation: u8, tick: u64) -> &'static str {
    // For action frames (attack, pain, death), use rotation 1 only
    match (enemy_type, ai_state) {
        (EnemyType::Imp, EnemyAiState::Dead) => return "TROOH1",
        (EnemyType::Imp, EnemyAiState::Pain) => return "TROOE1",
        (EnemyType::Imp, EnemyAiState::Attacking) => return "TROOC1",
        (EnemyType::Demon, EnemyAiState::Dead) => return "SARGH1",
        (EnemyType::Demon, EnemyAiState::Pain) => return "SARGE1",
        (EnemyType::Demon, EnemyAiState::Attacking) => return "SARGC1",
        (EnemyType::Sergeant, EnemyAiState::Dead) => return "SPOSH0",
        (EnemyType::Sergeant, EnemyAiState::Pain) => return "SPOSE1",
        (EnemyType::Sergeant, EnemyAiState::Attacking) => return "SPOSC1",
        _ => {}
    }

    // Walk animation — alternate A/B frames based on tick
    let walk_b = tick % 8 >= 4;

    // Map rotation to sprite name. Doom uses mirrored rotations:
    // Rot 1=front, 2/8=mirror pair, 3/7=mirror pair, 4/6=mirror pair, 5=back
    // In the WAD: "TROOA2A8" means rotation 2 and 8 share the same sprite (mirrored)
    match (enemy_type, walk_b, rotation) {
        // Imp walk frames with rotation
        (EnemyType::Imp, false, 1) => "TROOA1",
        (EnemyType::Imp, false, 2) | (EnemyType::Imp, false, 8) => "TROOA2A8",
        (EnemyType::Imp, false, 3) | (EnemyType::Imp, false, 7) => "TROOA3A7",
        (EnemyType::Imp, false, 4) | (EnemyType::Imp, false, 6) => "TROOA4A6",
        (EnemyType::Imp, false, 5) => "TROOA5",
        (EnemyType::Imp, true, 1) => "TROOB1",
        (EnemyType::Imp, true, 2) | (EnemyType::Imp, true, 8) => "TROOB2B8",
        (EnemyType::Imp, true, 3) | (EnemyType::Imp, true, 7) => "TROOB3B7",
        (EnemyType::Imp, true, 4) | (EnemyType::Imp, true, 6) => "TROOB4B6",
        (EnemyType::Imp, true, 5) => "TROOB5",

        // Demon walk frames with rotation
        (EnemyType::Demon, false, 1) => "SARGA1",
        (EnemyType::Demon, false, 2) | (EnemyType::Demon, false, 8) => "SARGA2A8",
        (EnemyType::Demon, false, 3) | (EnemyType::Demon, false, 7) => "SARGA3A7",
        (EnemyType::Demon, false, 4) | (EnemyType::Demon, false, 6) => "SARGA4A6",
        (EnemyType::Demon, false, 5) => "SARGA5",
        (EnemyType::Demon, true, 1) => "SARGB1",
        (EnemyType::Demon, true, 2) | (EnemyType::Demon, true, 8) => "SARGB2B8",
        (EnemyType::Demon, true, 3) | (EnemyType::Demon, true, 7) => "SARGB3B7",
        (EnemyType::Demon, true, 4) | (EnemyType::Demon, true, 6) => "SARGB4B6",
        (EnemyType::Demon, true, 5) => "SARGB5",

        // Sergeant walk frames with rotation
        (EnemyType::Sergeant, false, 1) => "SPOSA1",
        (EnemyType::Sergeant, false, 2) | (EnemyType::Sergeant, false, 8) => "SPOSA2A8",
        (EnemyType::Sergeant, false, 3) | (EnemyType::Sergeant, false, 7) => "SPOSA3A7",
        (EnemyType::Sergeant, false, 4) | (EnemyType::Sergeant, false, 6) => "SPOSA4A6",
        (EnemyType::Sergeant, false, 5) => "SPOSA5",
        (EnemyType::Sergeant, true, 1) => "SPOSB1",
        (EnemyType::Sergeant, true, 2) | (EnemyType::Sergeant, true, 8) => "SPOSB2B8",
        (EnemyType::Sergeant, true, 3) | (EnemyType::Sergeant, true, 7) => "SPOSB3B7",
        (EnemyType::Sergeant, true, 4) | (EnemyType::Sergeant, true, 6) => "SPOSB4B6",
        (EnemyType::Sergeant, true, 5) => "SPOSB5",

        // Fallback
        _ => match enemy_type {
            EnemyType::Imp => "TROOA1",
            EnemyType::Demon => "SARGA1",
            EnemyType::Sergeant => "SPOSA1",
        },
    }
}

/// Compute Doom-style sprite rotation index (1-8) based on viewer angle to sprite
/// and the sprite's facing direction.
fn compute_rotation(viewer_angle_to_sprite: i32, sprite_facing: i32) -> u8 {
    // The rotation depends on the angle between the viewer's line of sight to the sprite
    // and the direction the sprite is facing.
    // angle = sprite_facing - viewer_angle_to_sprite + PI (flip because we want viewer's perspective)
    let diff = normalize_angle(sprite_facing - viewer_angle_to_sprite + PI);
    // Map to 1-8: each rotation covers 45° (785 millirad)
    let sector = ((diff + 392) / 785) % 8; // +392 = half of 785 for centering
    (sector as u8) + 1
}

/// Get the sprite name for an item type.
fn item_sprite_name(item_type: ItemType) -> &'static str {
    match item_type {
        ItemType::HealthPack => "STIMA0",
        ItemType::Medikit => "MEDIA0",
        ItemType::AmmoClip => "CLIPA0",
        ItemType::AmmoBox => "AMMOA0",
        ItemType::Armor => "ARM1A0",
        ItemType::KeyRed => "RKEYA0",
        ItemType::KeyBlue => "BKEYA0",
        ItemType::ShellBox => "SHELA0",
        ItemType::Shotgun => "SHOTA0",
        ItemType::Chaingun => "MGUNA0",  // chaingun pickup
        ItemType::RocketLauncher => "LAUNA0", // rocket launcher pickup
        ItemType::RocketBox => "BROKA0",  // box of rockets
    }
}

/// Render all visible sprites (enemies, items, projectiles) with real textures.
fn render_sprites(
    state: &GameState,
    fb: &mut Framebuffer,
    depth_buf: &DepthBuffer,
    player_floor: i32,
) {
    let mut sprite_list: Vec<SpriteRender> = Vec::new();

    let px = state.player.x;
    let py = state.player.y;
    let pa = state.player.angle;

    // Enemies — with 8-directional rotation
    for enemy in &state.enemies {
        let dx = enemy.x - px;
        let dy = enemy.y - py;
        let dist = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i32;
        if dist < 100 { continue; }

        let angle_to = ((dy as f64).atan2(dx as f64) * 1000.0) as i32;
        let angle_diff = normalize_angle(angle_to - pa + PI) - PI;
        if angle_diff.abs() > HALF_FOV + 200 { continue; }

        let screen_x = SCREEN_WIDTH as i32 / 2 + (angle_diff * SCREEN_WIDTH as i32 / FOV);

        // Compute rotation based on enemy's facing direction relative to viewer
        let enemy_facing = enemy.move_dir; // direction enemy is moving
        let rotation = compute_rotation(angle_to, enemy_facing);
        let name = enemy_sprite_name_rotated(enemy.enemy_type, &enemy.ai_state, rotation, state.tick);

        let egx = (enemy.x / FP_SCALE) as u32;
        let egy = (enemy.y / FP_SCALE) as u32;
        let enemy_sector = state.map.get_sector(egx, egy);

        sprite_list.push(SpriteRender {
            screen_x,
            distance: dist,
            sprite_name: name,
            floor_height: enemy_sector.floor_height,
        });
    }

    // Items
    for item in &state.items {
        if item.picked_up { continue; }

        let dx = item.x - px;
        let dy = item.y - py;
        let dist = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i32;
        if dist < 100 { continue; }

        let angle_to = ((dy as f64).atan2(dx as f64) * 1000.0) as i32;
        let angle_diff = normalize_angle(angle_to - pa + PI) - PI;
        if angle_diff.abs() > HALF_FOV + 200 { continue; }

        let screen_x = SCREEN_WIDTH as i32 / 2 + (angle_diff * SCREEN_WIDTH as i32 / FOV);
        let name = item_sprite_name(item.item_type);

        let igx = (item.x / FP_SCALE) as u32;
        let igy = (item.y / FP_SCALE) as u32;
        let item_sector = state.map.get_sector(igx, igy);

        sprite_list.push(SpriteRender {
            screen_x,
            distance: dist,
            sprite_name: name,
            floor_height: item_sector.floor_height,
        });
    }

    // Projectiles (fireballs)
    for proj in &state.projectiles {
        if !proj.alive { continue; }

        let dx = proj.x - px;
        let dy = proj.y - py;
        let dist = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i32;
        if dist < 100 { continue; }

        let angle_to = ((dy as f64).atan2(dx as f64) * 1000.0) as i32;
        let angle_diff = normalize_angle(angle_to - pa + PI) - PI;
        if angle_diff.abs() > HALF_FOV + 200 { continue; }

        let screen_x = SCREEN_WIDTH as i32 / 2 + (angle_diff * SCREEN_WIDTH as i32 / FOV);

        let proj_sprite = if proj.sprite_id == 2 { "MISLA1" } else { "BAL1A0" };
        sprite_list.push(SpriteRender {
            screen_x,
            distance: dist,
            sprite_name: proj_sprite,
            floor_height: 500, // mid-air
        });
    }

    // Decorations
    for deco in &state.decorations {
        let dx = deco.x - px;
        let dy = deco.y - py;
        let dist = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i32;
        if dist < 100 { continue; }

        let angle_to = ((dy as f64).atan2(dx as f64) * 1000.0) as i32;
        let angle_diff = normalize_angle(angle_to - pa + PI) - PI;
        if angle_diff.abs() > HALF_FOV + 200 { continue; }

        let screen_x = SCREEN_WIDTH as i32 / 2 + (angle_diff * SCREEN_WIDTH as i32 / FOV);
        let name = decoration_sprite_name(deco.deco_type, state.tick);

        let dgx = (deco.x / FP_SCALE) as u32;
        let dgy = (deco.y / FP_SCALE) as u32;
        let deco_sector = state.map.get_sector(dgx, dgy);

        sprite_list.push(SpriteRender {
            screen_x,
            distance: dist,
            sprite_name: name,
            floor_height: deco_sector.floor_height,
        });
    }

    // Visual effects (bullet puffs, blood splats)
    for effect in &state.effects {
        let dx = effect.x - px;
        let dy = effect.y - py;
        let dist = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i32;
        if dist < 100 { continue; }

        let angle_to = ((dy as f64).atan2(dx as f64) * 1000.0) as i32;
        let angle_diff = normalize_angle(angle_to - pa + PI) - PI;
        if angle_diff.abs() > HALF_FOV + 200 { continue; }

        let screen_x = SCREEN_WIDTH as i32 / 2 + (angle_diff * SCREEN_WIDTH as i32 / FOV);

        let egx = (effect.x / FP_SCALE) as u32;
        let egy = (effect.y / FP_SCALE) as u32;
        let effect_sector = state.map.get_sector(egx, egy);

        // Use BAL1A0 for puffs (small bright sprite), or render procedurally
        let sprite_name = match effect.effect_type {
            EffectType::BulletPuff => "BAL1A0",
            EffectType::BloodSplat => "BAL1A0",
        };

        sprite_list.push(SpriteRender {
            screen_x,
            distance: dist,
            sprite_name,
            floor_height: effect_sector.floor_height + 500, // mid-height
        });
    }

    // Sort back to front
    sprite_list.sort_by(|a, b| b.distance.cmp(&a.distance));

    let view_mid = VIEW_HEIGHT as i32 / 2;

    for spr in &sprite_list {
        let sprite_data = match find_sprite(spr.sprite_name) {
            Some(s) => s,
            None => continue,
        };

        let bright = distance_brightness(spr.distance);

        let src_h = sprite_data.height as i32;
        let src_w = sprite_data.width as i32;

        if src_h <= 0 || src_w <= 0 {
            continue;
        }

        // Use larger scale divisor for tall torch/pillar sprites so they fit in rooms
        let ppt = if is_tall_decoration(spr.sprite_name) {
            TORCH_PIXELS_PER_TILE as i64
        } else {
            SPRITE_PIXELS_PER_TILE as i64
        };

        let proj_height = (src_h as i64 * VIEW_HEIGHT as i64 * FP_SCALE as i64
            / (ppt * spr.distance.max(1) as i64)) as i32;
        let proj_height = proj_height.min(VIEW_HEIGHT as i32 * 2);

        if proj_height <= 0 {
            continue;
        }

        // Width scales proportionally to maintain sprite aspect ratio
        let proj_width = (src_w as i64 * VIEW_HEIGHT as i64 * FP_SCALE as i64
            / (ppt * spr.distance.max(1) as i64)) as i32;

        // Anchor sprite using top_offset — in Doom, top_offset is how many pixels
        // from the top of the sprite down to the "origin" (feet/ground point).
        // The origin should align with the floor projection at this distance.
        let eye_height = player_floor + 500;
        let floor_offset = eye_height - spr.floor_height;
        let scale = VIEW_HEIGHT as i64 * FP_SCALE as i64 / spr.distance.max(1) as i64;
        let screen_floor_y = view_mid + (floor_offset as i64 * scale / FP_SCALE as i64) as i32;

        // top_offset pixels from sprite top = floor. Project top_offset proportionally.
        let top_off = sprite_data.top_offset as i32;
        let proj_top_offset = top_off * proj_height / src_h;
        let screen_top = screen_floor_y - proj_top_offset;

        let x_start = spr.screen_x - proj_width / 2;

        for sx in 0..proj_width {
            let screen_col = x_start + sx;
            if screen_col < 0 || screen_col >= SCREEN_WIDTH as i32 {
                continue;
            }
            let ucol = screen_col as usize;

            if spr.distance > depth_buf[ucol] {
                continue;
            }

            let src_x = (sx * src_w / proj_width).min(src_w - 1) as usize;

            for sy in 0..proj_height {
                let screen_row = screen_top + sy;
                if screen_row < 0 || screen_row >= VIEW_HEIGHT as i32 {
                    continue;
                }

                let src_y = (sy * src_h / proj_height).min(src_h - 1) as usize;

                let idx = src_y * sprite_data.width as usize + src_x;
                if idx >= sprite_data.data.len() {
                    continue;
                }
                let pal_idx = sprite_data.data[idx];
                if pal_idx == 255 {
                    continue;
                }

                fb.set_pal(ucol, screen_row as usize, pal_idx, bright);
            }
        }
    }
}

/// Render the first-person weapon sprite.
fn render_weapon(state: &GameState, fb: &mut Framebuffer) {
    if !state.player.alive {
        return;
    }

    // Choose weapon frame based on weapon type and cooldown
    let frame_name = match state.player.current_weapon {
        WeaponType::Fist => {
            if state.player.weapon_cooldown >= 3 {
                "PUNGD0" // punch forward
            } else if state.player.weapon_cooldown >= 1 {
                "PUNGC0" // recoil
            } else {
                "PUNGA0" // idle
            }
        }
        WeaponType::Pistol => {
            if state.player.weapon_cooldown >= 2 {
                "PISGE0" // flash
            } else if state.player.weapon_cooldown == 1 {
                "PISGC0" // recoil
            } else {
                "PISGA0" // idle
            }
        }
        WeaponType::Shotgun => {
            if state.player.weapon_cooldown >= 5 {
                "SHTGD0" // pump back
            } else if state.player.weapon_cooldown >= 3 {
                "SHTGC0" // recoil
            } else if state.player.weapon_cooldown >= 1 {
                "SHTGB0" // fire
            } else {
                "SHTGA0" // idle
            }
        }
        WeaponType::Chaingun => {
            // Reuse pistol sprites with faster cycling
            if state.player.weapon_cooldown >= 1 {
                "PISGE0" // flash
            } else {
                "PISGA0" // idle
            }
        }
        WeaponType::RocketLauncher => {
            // Reuse shotgun sprites (visually similar enough)
            if state.player.weapon_cooldown >= 6 {
                "SHTGD0"
            } else if state.player.weapon_cooldown >= 3 {
                "SHTGC0"
            } else if state.player.weapon_cooldown >= 1 {
                "SHTGB0"
            } else {
                "SHTGA0"
            }
        }
    };

    let sprite = match find_sprite(frame_name) {
        Some(s) => s,
        // Fallback to pistol if weapon sprites not found
        None => match find_sprite("PISGA0") {
            Some(s) => s,
            None => return,
        },
    };

    let src_w = sprite.width as usize;
    let src_h = sprite.height as usize;

    // Doom weapon sprites are drawn at 1:1 scale, positioned using their offsets.
    // The offset fields anchor the sprite relative to center-bottom of viewport.
    let dst_w = src_w;
    let dst_h = src_h;

    // Center horizontally, anchor bottom to viewport bottom
    let x_start = (SCREEN_WIDTH / 2).saturating_sub(dst_w / 2);
    let y_start = VIEW_HEIGHT.saturating_sub(dst_h);

    for dy in 0..dst_h {
        let screen_y = y_start + dy;
        if screen_y >= VIEW_HEIGHT { continue; }

        for dx in 0..dst_w {
            let screen_x = x_start + dx;
            if screen_x >= SCREEN_WIDTH { continue; }

            let idx = dy * src_w + dx;
            if idx >= sprite.data.len() { continue; }
            let pal_idx = sprite.data[idx];
            if pal_idx == 255 { continue; }

            fb.set_pal_full(screen_x, screen_y, pal_idx);
        }
    }
}

/// Render the DOOM-style status bar at the bottom of the screen.
fn render_stbar(state: &GameState, fb: &mut Framebuffer) {
    let bar_w = stbar::STBAR_W as usize;
    let bar_h = stbar::STBAR_H as usize;
    let y_start = SCREEN_HEIGHT - bar_h;

    for y in 0..bar_h {
        for x in 0..bar_w.min(SCREEN_WIDTH) {
            let idx = y * bar_w + x;
            if idx >= stbar::STBAR.len() { continue; }
            let pal_idx = stbar::STBAR[idx];
            if pal_idx == 255 { continue; }
            fb.set_pal_full(x, y_start + y, pal_idx);
        }
    }

    // Draw STTNUM digit sprites — Freedoom STBAR box positions.
    // Dividers at x=44, 104, 174, 235.
    // Boxes: AMMO(0-44), HEALTH(44-104), center(104-174), ARMOR(174-235), right(235-320)
    // 3 digits × 14px = 42px. Percent = 13px. Total with % = 55px.
    let num_y = y_start + 3;

    // Ammo — right-aligned in AMMO box (0-44)
    let ammo_display = match state.player.current_weapon {
        WeaponType::Fist => -1,
        WeaponType::Pistol | WeaponType::Chaingun => state.player.ammo,
        WeaponType::Shotgun => state.player.shells,
        WeaponType::RocketLauncher => state.player.rockets,
    };
    if ammo_display >= 0 {
        draw_sttnum(fb, 1, num_y, ammo_display, 3);
    }
    // Health — left edge of HEALTH box (44-104), with % after
    draw_sttnum(fb, 49, num_y, state.player.health, 3);
    draw_sttnum_percent(fb, 49 + 42, num_y);
    // Armor — left edge of ARMOR box (174-235), with % after
    draw_sttnum(fb, 179, num_y, state.player.armor, 3);
    draw_sttnum_percent(fb, 179 + 42, num_y);

    // Ammo inventory — right panel of STBAR
    // Simple thin yellow digits like original Doom (STBAR has / separators baked in)
    let inv_lx = 279; // left numbers (current ammo)
    let inv_rx = 299; // right numbers (max ammo)
    let inv_y = y_start + 5;
    let row_h = 7;
    // Row 1: Bullets — current (left) and max (right of built-in slash)
    draw_tiny_yellow_num(fb, inv_lx, inv_y, state.player.ammo);
    draw_tiny_yellow_num(fb, inv_rx, inv_y, 200);
    // Row 2: Shells — current and max
    draw_tiny_yellow_num(fb, inv_lx, inv_y + row_h, state.player.shells);
    draw_tiny_yellow_num(fb, inv_rx, inv_y + row_h, 50);
    // Row 3: Rockets — current and max
    draw_tiny_yellow_num(fb, inv_lx, inv_y + row_h * 2, state.player.rockets);
    draw_tiny_yellow_num(fb, inv_rx, inv_y + row_h * 2, 50);

    // Status bar face — center panel (x=104..174)
    render_stbar_face(state, fb, y_start);
}

/// Draw a number using STTNUM digit sprites from Freedoom WAD.
/// Each digit is 14px wide (13px sprite + 1px spacing).
fn draw_sttnum(fb: &mut Framebuffer, x: usize, y: usize, value: i32, num_digits: usize) {
    let val = value.max(0);
    // Build digit array right-to-left
    let mut digits = [10u8; 3]; // 10 = blank
    let mut v = val;
    for i in (0..num_digits).rev() {
        digits[i] = (v % 10) as u8;
        v /= 10;
        if v == 0 {
            // Leave remaining positions as blank (10)
            break;
        }
    }

    let mut cx = x;
    for i in 0..num_digits {
        if digits[i] < 10 {
            draw_sttnum_digit(fb, cx, y, digits[i]);
        }
        cx += 14; // 13px digit + 1px spacing
    }
}

/// Draw a single STTNUM digit (0-9) from WAD sprite data.
fn draw_sttnum_digit(fb: &mut Framebuffer, x: usize, y: usize, digit: u8) {
    if digit > 9 { return; }
    let d = &sttnum::DIGITS[digit as usize];
    let w = d.width as usize;
    let h = d.height as usize;

    for dy in 0..h {
        for dx in 0..w {
            let idx = dy * w + dx;
            if idx >= d.data.len() { continue; }
            let pal_idx = d.data[idx];
            if pal_idx == 255 { continue; } // transparent
            fb.set_pal_full(x + dx, y + dy, pal_idx);
        }
    }
}

/// Draw the STTNUM percent sign.
fn draw_sttnum_percent(fb: &mut Framebuffer, x: usize, y: usize) {
    let d = &sttnum::PERCENT;
    let w = d.width as usize;
    let h = d.height as usize;

    for dy in 0..h {
        for dx in 0..w {
            let idx = dy * w + dx;
            if idx >= d.data.len() { continue; }
            let pal_idx = d.data[idx];
            if pal_idx == 255 { continue; }
            fb.set_pal_full(x + dx, y + dy, pal_idx);
        }
    }
}

/// Draw a number in tiny yellow pixel font for the ammo inventory.
/// Right-aligned within 3-digit width, no leading zeros.
fn draw_tiny_yellow_num(fb: &mut Framebuffer, x: usize, y: usize, value: i32) {
    let val = value.max(0).min(999);
    let char_w = 4; // 3px digit + 1px gap

    let d2 = (val % 10) as u8;
    let d1 = ((val / 10) % 10) as u8;
    let d0 = ((val / 100) % 10) as u8;

    // Right-aligned: always draw ones, conditionally tens and hundreds
    if d0 > 0 {
        draw_tiny_digit(fb, x, y, d0);
    }
    if d0 > 0 || d1 > 0 {
        draw_tiny_digit(fb, x + char_w, y, d1);
    }
    draw_tiny_digit(fb, x + char_w * 2, y, d2);
}

/// Draw a single digit in a 3×5 thin yellow pixel font.
fn draw_tiny_digit(fb: &mut Framebuffer, x: usize, y: usize, digit: u8) {
    // 3×5 bitmaps — each row is 3 bits wide (MSB = leftmost pixel)
    let glyph: [u8; 5] = match digit {
        0 => [0b111, 0b101, 0b101, 0b101, 0b111],
        1 => [0b010, 0b110, 0b010, 0b010, 0b111],
        2 => [0b111, 0b001, 0b111, 0b100, 0b111],
        3 => [0b111, 0b001, 0b111, 0b001, 0b111],
        4 => [0b101, 0b101, 0b111, 0b001, 0b001],
        5 => [0b111, 0b100, 0b111, 0b001, 0b111],
        6 => [0b111, 0b100, 0b111, 0b101, 0b111],
        7 => [0b111, 0b001, 0b010, 0b010, 0b010],
        8 => [0b111, 0b101, 0b111, 0b101, 0b111],
        9 => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => return,
    };
    let color: (u8, u8, u8) = (220, 190, 50); // bright gold yellow
    for row in 0..5 {
        for col in 0..3 {
            if glyph[row] & (1 << (2 - col)) != 0 {
                fb.set_rgb(x + col, y + row, color.0, color.1, color.2);
            }
        }
    }
}

/// Render the DOOM-guy face in the center of the status bar using real Freedoom sprites.
/// STFST00/01/02 = normal (forward/left/right), STFST20 = hurt, STFST40 = critical,
/// STFOUCH0 = pain, STFKILL0 = rampage, STFDEAD0 = dead.
fn render_stbar_face(state: &GameState, fb: &mut Framebuffer, bar_y: usize) {
    let recently_hurt = state.tick.saturating_sub(state.player.last_damage_tick) < 10;
    let health = state.player.health;

    // Choose face sprite name based on state
    let face_name = if !state.player.alive {
        "STFDEAD0"
    } else if recently_hurt {
        "STFOUCH0"
    } else if state.player.kills > 0 && state.tick.saturating_sub(state.player.last_damage_tick) < 30 {
        "STFKILL0"
    } else {
        // Health-based face + direction (0=forward, 1=left, 2=right)
        let angle_norm = (state.player.angle as usize) % 6283;
        let dir = if angle_norm < 1047 || angle_norm > 5236 { "0" }
            else if angle_norm < 3142 { "1" }
            else { "2" };
        let level = if health > 60 { "0" }
            else if health > 40 { "2" }
            else { "4" };
        // Build name: STFST + level + dir
        match (level, dir) {
            ("0", "0") => "STFST00",
            ("0", "1") => "STFST01",
            ("0", "2") => "STFST02",
            ("2", "0") => "STFST20",
            ("2", "1") => "STFST21",
            ("2", "2") => "STFST22",
            ("4", "0") => "STFST40",
            ("4", "1") => "STFST41",
            ("4", "2") => "STFST42",
            _ => "STFST00",
        }
    };

    let face = match faces::find_face(face_name) {
        Some(f) => f,
        None => return, // no sprite found, skip
    };

    let w = face.width as usize;
    let h = face.height as usize;

    // Center in the STBAR center panel (x=104..174)
    let center_x: usize = 160;
    let fx = center_x.saturating_sub(w / 2);
    let fy = bar_y + 2;

    for dy in 0..h {
        for dx in 0..w {
            let idx = dy * w + dx;
            if idx >= face.data.len() { continue; }
            let pal_idx = face.data[idx];
            if pal_idx == 255 { continue; } // transparent
            let px = fx + dx;
            let py = fy + dy;
            if px < SCREEN_WIDTH && py < SCREEN_HEIGHT {
                fb.set_pal_full(px, py, pal_idx);
            }
        }
    }
}

/// Render a single sky pixel — gradient from dark blue to light blue, with stars.
/// Scrolls horizontally with player angle for parallax.
#[inline]
fn render_sky_pixel(fb: &mut Framebuffer, col: usize, y: usize, player_angle: i32, view_mid: usize) {
    // Vertical gradient: dark blue at top → lighter blue at horizon
    let t = (y * 255) / view_mid.max(1);
    let r = (t * 40 / 255) as u8;
    let g = (t * 80 / 255) as u8;
    let b = (60 + t * 140 / 255).min(200) as u8;

    // Pseudo-random stars (deterministic based on screen position + angle scroll)
    let scroll = (player_angle as usize / 10) % SCREEN_WIDTH;
    let sx = (col + scroll) % SCREEN_WIDTH;
    let star_hash = (sx * 7919 + y * 6271) % 997;
    if star_hash < 8 && y < view_mid / 2 {
        // Bright star
        fb.set_rgb(col, y, 255, 255, 240);
    } else {
        fb.set_rgb(col, y, r, g, b);
    }
}

/// Render automap overlay — OG DOOM style: black background, colored line walls.
/// Red = impassable walls, yellow = height changes, brown = two-sided lines,
/// green = player arrow, gray = doors.
pub fn render_automap(state: &GameState, fb: &mut Framebuffer) {
    // Black background over viewport
    for y in 0..VIEW_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            fb.set_rgb(x, y, 0, 0, 0);
        }
    }

    let map_w = state.map.width as usize;
    let map_h = state.map.height as usize;

    // Scale to fit viewport with padding
    let cell_w = (SCREEN_WIDTH - 8) / map_w;
    let cell_h = (VIEW_HEIGHT - 8) / map_h;
    let cell = cell_w.min(cell_h).max(2);

    let offset_x = (SCREEN_WIDTH - map_w * cell) / 2;
    let offset_y = (VIEW_HEIGHT - map_h * cell) / 2;

    // Draw wall edges (line-based like OG DOOM automap)
    for gy in 0..map_h {
        for gx in 0..map_w {
            let tile = state.map.get_tile(gx as u32, gy as u32);
            let sx = offset_x + gx * cell;
            let sy = offset_y + gy * cell;

            let is_wall = matches!(tile, TileType::Wall(_));
            let is_door = matches!(tile, TileType::Door(_));
            let is_exit = matches!(tile, TileType::Exit);

            // Check each edge: draw line if this cell and neighbor differ
            // Right edge
            if gx + 1 < map_w {
                let right = state.map.get_tile(gx as u32 + 1, gy as u32);
                let right_wall = matches!(right, TileType::Wall(_));
                if is_wall != right_wall || is_door || matches!(right, TileType::Door(_)) {
                    let (r, g, b) = edge_color(tile, right);
                    for dy in 0..=cell {
                        fb.set_rgb(sx + cell, sy + dy, r, g, b);
                    }
                }
            }
            // Bottom edge
            if gy + 1 < map_h {
                let below = state.map.get_tile(gx as u32, gy as u32 + 1);
                let below_wall = matches!(below, TileType::Wall(_));
                if is_wall != below_wall || is_door || matches!(below, TileType::Door(_)) {
                    let (r, g, b) = edge_color(tile, below);
                    for dx in 0..=cell {
                        fb.set_rgb(sx + dx, sy + cell, r, g, b);
                    }
                }
            }
            // (boundary edges handled by perimeter border below)

            // Exit marker — green filled
            if is_exit {
                for dy in 1..cell {
                    for dx in 1..cell {
                        fb.set_rgb(sx + dx, sy + dy, 0, 160, 0);
                    }
                }
            }
        }
    }

    // Draw clean 1px perimeter border around entire map
    let bx0 = offset_x.saturating_sub(1);
    let by0 = offset_y.saturating_sub(1);
    let bx1 = offset_x + map_w * cell;
    let by1 = offset_y + map_h * cell;
    for x in bx0..=bx1.min(SCREEN_WIDTH - 1) {
        fb.set_rgb(x, by0, 180, 0, 0);
        if by1 < VIEW_HEIGHT { fb.set_rgb(x, by1, 180, 0, 0); }
    }
    for y in by0..=by1.min(VIEW_HEIGHT - 1) {
        fb.set_rgb(bx0, y, 180, 0, 0);
        if bx1 < SCREEN_WIDTH { fb.set_rgb(bx1, y, 180, 0, 0); }
    }

    // Draw things — triangular markers like OG DOOM
    // Items: small yellow triangles
    for item in &state.items {
        if item.picked_up { continue; }
        let sx = offset_x + (item.x as usize / FP_SCALE as usize) * cell + cell / 2;
        let sy = offset_y + (item.y as usize / FP_SCALE as usize) * cell + cell / 2;
        automap_dot(fb, sx, sy, 255, 255, 0);
    }

    // Enemies: red triangles (alive) or dark (dead)
    for enemy in &state.enemies {
        let sx = offset_x + (enemy.x as usize / FP_SCALE as usize) * cell + cell / 2;
        let sy = offset_y + (enemy.y as usize / FP_SCALE as usize) * cell + cell / 2;
        if enemy.is_alive() {
            automap_triangle(fb, sx, sy, 0.0, 255, 60, 0);
        } else {
            automap_dot(fb, sx, sy, 80, 0, 0);
        }
    }

    // Player: green arrow (OG DOOM's signature automap element)
    let player_sx = offset_x + (state.player.x as usize / FP_SCALE as usize) * cell + cell / 2;
    let player_sy = offset_y + (state.player.y as usize / FP_SCALE as usize) * cell + cell / 2;
    let angle = state.player.angle as f64 / 1000.0;
    let arrow_len = cell as f64 * 0.9;

    // Arrow body
    let tip_x = player_sx as f64 + angle.cos() * arrow_len;
    let tip_y = player_sy as f64 + angle.sin() * arrow_len;
    automap_line(fb, player_sx, player_sy, tip_x as usize, tip_y as usize, 0, 255, 0);

    // Arrow wings (±140° from forward)
    let wing_angle_l = angle + 2.44; // ~140°
    let wing_angle_r = angle - 2.44;
    let wing_len = arrow_len * 0.5;
    let wl_x = player_sx as f64 + wing_angle_l.cos() * wing_len;
    let wl_y = player_sy as f64 + wing_angle_l.sin() * wing_len;
    let wr_x = player_sx as f64 + wing_angle_r.cos() * wing_len;
    let wr_y = player_sy as f64 + wing_angle_r.sin() * wing_len;
    automap_line(fb, player_sx, player_sy, wl_x as usize, wl_y as usize, 0, 255, 0);
    automap_line(fb, player_sx, player_sy, wr_x as usize, wr_y as usize, 0, 255, 0);
}

/// OG DOOM automap edge color based on tile types.
fn edge_color(a: TileType, b: TileType) -> (u8, u8, u8) {
    let a_wall = matches!(a, TileType::Wall(_));
    let b_wall = matches!(b, TileType::Wall(_));
    let a_door = matches!(a, TileType::Door(_));
    let b_door = matches!(b, TileType::Door(_));

    if a_door || b_door {
        (220, 170, 50)  // yellow — doors
    } else if a_wall || b_wall {
        (180, 0, 0)     // red — solid walls (OG DOOM red)
    } else {
        (120, 80, 40)   // brown — two-sided lines (height changes)
    }
}

/// Draw a small triangle marker on the automap.
fn automap_triangle(fb: &mut Framebuffer, cx: usize, cy: usize, angle: f64, r: u8, g: u8, b: u8) {
    let size = 2.0;
    for i in 0..3 {
        let a = angle + (i as f64) * 2.094; // 120° apart
        let px = (cx as f64 + a.cos() * size) as usize;
        let py = (cy as f64 + a.sin() * size) as usize;
        if px < SCREEN_WIDTH && py < VIEW_HEIGHT {
            fb.set_rgb(px, py, r, g, b);
        }
    }
    // Center dot
    fb.set_rgb(cx, cy, r, g, b);
}

/// Draw a dot marker on the automap.
fn automap_dot(fb: &mut Framebuffer, x: usize, y: usize, r: u8, g: u8, b: u8) {
    if x < SCREEN_WIDTH && y < VIEW_HEIGHT {
        fb.set_rgb(x, y, r, g, b);
    }
    if x + 1 < SCREEN_WIDTH && y < VIEW_HEIGHT {
        fb.set_rgb(x + 1, y, r, g, b);
    }
}

/// Draw a line on the automap using Bresenham's algorithm.
fn automap_line(fb: &mut Framebuffer, x0: usize, y0: usize, x1: usize, y1: usize, r: u8, g: u8, b: u8) {
    let mut x = x0 as i32;
    let mut y = y0 as i32;
    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx = if x0 < x1 { 1i32 } else { -1 };
    let sy = if y0 < y1 { 1i32 } else { -1 };
    let mut err = dx + dy;

    for _ in 0..200 {
        if x >= 0 && y >= 0 && (x as usize) < SCREEN_WIDTH && (y as usize) < VIEW_HEIGHT {
            fb.set_rgb(x as usize, y as usize, r, g, b);
        }
        if x == x1 as i32 && y == y1 as i32 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Returns true for tall torch/pillar sprites that need reduced scale.
fn is_tall_decoration(name: &str) -> bool {
    matches!(name,
        "TBLUA0" | "TBLUB0" | "TGRNA0" | "TGRNB0" | "TREDA0" | "TREDB0" |
        "SMBTA0" | "SMBTB0" | "SMGTA0" | "SMGTB0" | "SMRTA0" | "SMRTB0" |
        "CBRAA0" | "CBRAB0" | "COLUA0" | "ELECA0" |
        "COL1A0" | "COL2A0" | "COL3A0" | "COL4A0" | "COL5A0" | "COL6A0" |
        "POL3A0" | "POL3B0" | "GOR1A0"
    )
}

/// Render centered overlay text using a chunky 5×7 pixel font.
/// Each character is scaled 3× for visibility. Used for "YOU DIED" etc.
fn render_overlay_text(fb: &mut Framebuffer, text: &str, r: u8, g: u8, b: u8) {
    let scale = 3;
    let char_w = 5 * scale + scale; // 5px + 1px gap, scaled
    let char_h = 7 * scale;
    let total_w = text.len() * char_w;
    let x_start = (SCREEN_WIDTH.saturating_sub(total_w)) / 2;
    let y_start = (VIEW_HEIGHT.saturating_sub(char_h)) / 2;

    // Draw dark background for readability
    for y in y_start.saturating_sub(4)..=(y_start + char_h + 4).min(VIEW_HEIGHT - 1) {
        for x in x_start.saturating_sub(8)..=(x_start + total_w + 8).min(SCREEN_WIDTH - 1) {
            let off = (y * SCREEN_WIDTH + x) * 4;
            if off + 3 < fb.rgba.len() {
                fb.rgba[off] = fb.rgba[off] / 3;
                fb.rgba[off + 1] = fb.rgba[off + 1] / 3;
                fb.rgba[off + 2] = fb.rgba[off + 2] / 3;
            }
        }
    }

    let mut cx = x_start;
    for ch in text.chars() {
        let bitmap = char_bitmap(ch);
        for row in 0..7 {
            for col in 0..5 {
                if bitmap[row] & (1 << (4 - col)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            fb.set_rgb(cx + col * scale + sx, y_start + row * scale + sy, r, g, b);
                        }
                    }
                }
            }
        }
        cx += char_w;
    }
}

/// 5×7 bitmap font for overlay text.
fn char_bitmap(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        ' ' => [0; 7],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111], // box
    }
}

/// Compute brightness (0-255) based on distance fog.
fn distance_brightness(distance: i32) -> u8 {
    let min_dist = 2 * FP_SCALE;
    let max_dist = 12 * FP_SCALE;
    let min_bright: i32 = 76;

    if distance <= min_dist {
        255
    } else if distance >= max_dist {
        min_bright as u8
    } else {
        let range = max_dist - min_dist;
        let fade = distance - min_dist;
        (255 - ((255 - min_bright) * fade / range)) as u8
    }
}

/// Combine distance fog brightness with sector light level.
/// Both are 0-255. Result is the minimum of the two, scaled.
#[inline]
fn combine_brightness(dist_bright: u8, sector_light: u8) -> u8 {
    (dist_bright as u32 * sector_light as u32 / 255) as u8
}

/// Map a palette index to RGB using the Freedoom palette.
pub fn palette_color_rgb(idx: u8) -> (u8, u8, u8) {
    let c = &PALETTE[idx as usize];
    (c[0], c[1], c[2])
}

// ═══════════════════════════════════════════════════════════════
//  TITLE SCREEN
// ═══════════════════════════════════════════════════════════════

/// Render the title screen into the framebuffer.
/// `tick` is used for the blinking "PRESS ANY KEY" text.
pub fn render_title_screen(fb: &mut Framebuffer, tick: u64) {
    // Fill with black
    for i in 0..FRAMEBUFFER_SIZE {
        let off = i * 4;
        fb.rgba[off] = 0;
        fb.rgba[off + 1] = 0;
        fb.rgba[off + 2] = 0;
        fb.rgba[off + 3] = 255;
    }

    // Harsh red band behind logo — hard edges, not smooth
    for y in 58..142 {
        let intensity: u8 = if y < 68 || y > 132 { 15 } else { 30 };
        for x in 20..300 {
            let off = (y * SCREEN_WIDTH + x) * 4;
            fb.rgba[off] = intensity;
        }
    }

    // "CHAIN" — first line
    let letters_chain: [&[u16]; 5] = [
        &TITLE_C, &TITLE_H, &TITLE_A, &TITLE_I, &TITLE_N,
    ];
    // "REACTOR" — second line
    let letters_reactor: [&[u16]; 7] = [
        &TITLE_R, &TITLE_E, &TITLE_A, &TITLE_C, &TITLE_T, &TITLE_O, &TITLE_R,
    ];

    let letter_w = 12;
    let letter_h = 18;
    let scale = 2;
    let rendered_w = letter_w * scale;
    let gap = 2;

    // Line 1: "CHAIN"
    let line1_w = 5 * rendered_w + 4 * gap;
    let line1_x = (SCREEN_WIDTH - line1_w) / 2;
    let line1_y = 60;
    for (i, glyph) in letters_chain.iter().enumerate() {
        draw_title_letter(fb, line1_x + i * (rendered_w + gap), line1_y, glyph, letter_w, letter_h, scale, tick);
    }

    // Line 2: "REACTOR"
    let line2_w = 7 * rendered_w + 6 * gap;
    let line2_x = (SCREEN_WIDTH - line2_w) / 2;
    let line2_y = 100;
    for (i, glyph) in letters_reactor.iter().enumerate() {
        draw_title_letter(fb, line2_x + i * (rendered_w + gap), line2_y, glyph, letter_w, letter_h, scale, tick);
    }

    // "PRESS ANY KEY" — small 1x scale text, blinking, below logo
    if (tick / 8) % 2 == 0 {
        draw_small_text(fb, "PRESS ANY KEY", 160, 155, 160, 160, 160);
    }
}

/// Draw small centered text using the 5×7 bitmap font at 1x scale.
fn draw_small_text(fb: &mut Framebuffer, text: &str, center_x: usize, y: usize, r: u8, g: u8, b: u8) {
    let char_w = 6; // 5px + 1px gap
    let total_w = text.len() * char_w;
    let x_start = center_x.saturating_sub(total_w / 2);

    for (i, ch) in text.chars().enumerate() {
        let bitmap = char_bitmap(ch);
        let cx = x_start + i * char_w;
        for row in 0..7 {
            for col in 0..5 {
                if bitmap[row] & (1 << (4 - col)) != 0 {
                    let px = cx + col;
                    let py = y + row;
                    if px < SCREEN_WIDTH && py < SCREEN_HEIGHT {
                        fb.set_rgb(px, py, r, g, b);
                    }
                }
            }
        }
    }
}

fn draw_title_letter(
    fb: &mut Framebuffer,
    x: usize, y: usize,
    glyph: &[u16],
    w: usize, h: usize,
    scale: usize,
    tick: u64,
) {
    // Heat shimmer: band boundaries shift over time
    // Use a slow sine-like wave via integer LUT (no floats)
    // Wave cycles every 32 ticks, shifts bands by ±2 rows
    let phase = (tick % 32) as i32;
    let wave = match phase {
        0..=3 => 0i32,
        4..=7 => 1,
        8..=11 => 2,
        12..=15 => 1,
        16..=19 => 0,
        20..=23 => -1,
        24..=27 => -2,
        _ => -1,
    };

    for row in 0..h {
        if row >= glyph.len() { break; }
        let bits = glyph[row];
        for col in 0..w {
            if bits & (1 << (w - 1 - col)) != 0 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + col * scale + sx;
                        let py = y + row * scale + sy;
                        if px < SCREEN_WIDTH && py < SCREEN_HEIGHT {
                            // Shifted row for band calculation — heat shimmer effect
                            let shifted = (row as i32 + wave).max(0) as usize;
                            let band = (shifted * 5) / h.max(1);
                            let (r, g, b) = match band {
                                0 => (255u8, 100u8, 0u8),   // bright orange
                                1 => (230, 50, 0),           // red-orange
                                2 => (200, 20, 0),           // red
                                3 => (160, 0, 0),            // dark red
                                _ => (120, 0, 0),            // crimson
                            };
                            fb.set_rgb(px, py, r, g, b);
                        }
                    }
                }
            }
        }
    }
}

// Heavy metal style bitmap font — 12×18 pixels per letter.
// Each row is a u16 with bits 11..0 representing pixels left to right.

const TITLE_C: [u16; 18] = [
    0b011111111110,
    0b111111111111,
    0b111100000011,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000011,
    0b111111111111,
    0b011111111110,
];

const TITLE_H: [u16; 18] = [
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111111111111,
    0b111111111111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111100001111,
    0b011110011110,
];

const TITLE_A: [u16; 18] = [
    0b000001100000,
    0b000011110000,
    0b000111111000,
    0b001111111100,
    0b011110011110,
    0b011100001110,
    0b111000000111,
    0b111000000111,
    0b111111111111,
    0b111111111111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111100001111,
    0b011110011110,
];

const TITLE_I: [u16; 18] = [
    0b011111111110,
    0b001111111100,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b001111111100,
    0b011111111110,
];

const TITLE_N: [u16; 18] = [
    0b111000000111,
    0b111100000111,
    0b111110000111,
    0b111111000111,
    0b111011100111,
    0b111001110111,
    0b111000111111,
    0b111000011111,
    0b111000001111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111100001111,
    0b011110011110,
];

const TITLE_R: [u16; 18] = [
    0b111111111100,
    0b111111111110,
    0b111000001111,
    0b111000000111,
    0b111000000111,
    0b111000001111,
    0b111111111110,
    0b111111111100,
    0b111111100000,
    0b111001110000,
    0b111000111000,
    0b111000011100,
    0b111000001110,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111100001111,
    0b011110011110,
];

const TITLE_E: [u16; 18] = [
    0b111111111111,
    0b111111111111,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111111111100,
    0b111111111100,
    0b111111111100,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111100000000,
    0b111111111111,
    0b111111111111,
];

const TITLE_T: [u16; 18] = [
    0b111111111111,
    0b111111111111,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000011110000,
    0b000111111000,
    0b001111111100,
];

const TITLE_O: [u16; 18] = [
    0b001111111100,
    0b011111111110,
    0b111100001111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111000000111,
    0b111100001111,
    0b011111111110,
    0b001111111100,
];
