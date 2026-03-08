use crate::types::*;
use serde::{Deserialize, Serialize};

/// Grid-based level map. Each cell is 1 unit (1000 in fixed-point).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoomMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileType>,
    pub sectors: Vec<Sector>,  // per-cell sector data (floor/ceiling/light)
    pub player_start: (i32, i32, i32), // x, y (fixed-point), angle (milliradians)
    pub enemy_spawns: Vec<(i32, i32, EnemyType)>,
    pub item_spawns: Vec<(i32, i32, ItemType)>,
    pub decorations: Vec<Decoration>,
}

impl DoomMap {
    /// Get sector for a grid position. Out of bounds returns default.
    pub fn get_sector(&self, gx: u32, gy: u32) -> Sector {
        if gx >= self.width || gy >= self.height {
            return Sector::default();
        }
        self.sectors[(gy * self.width + gx) as usize]
    }

    /// Get tile at grid position. Out of bounds returns Wall.
    pub fn get_tile(&self, gx: u32, gy: u32) -> TileType {
        if gx >= self.width || gy >= self.height {
            return TileType::Wall(0);
        }
        self.tiles[(gy * self.width + gx) as usize]
    }

    /// Check if a grid cell is solid (can't walk through).
    pub fn is_solid(&self, gx: u32, gy: u32) -> bool {
        match self.get_tile(gx, gy) {
            TileType::Empty | TileType::Exit => false,
            // Doors are passable once they start opening (like Doom)
            TileType::Door(DoorState::Opening(_))
            | TileType::Door(DoorState::Open)
            | TileType::Door(DoorState::OpenWait(_))
            | TileType::Door(DoorState::Closing(_)) => false,
            _ => true,
        }
    }

    /// Check if a fixed-point position collides with walls.
    /// Uses a small collision radius around the point.
    pub fn point_collides(&self, x: i32, y: i32) -> bool {
        let radius = 200; // collision radius in fixed-point (0.2 tiles)
        // Check all 4 corners of the bounding box
        let corners = [
            (x - radius, y - radius),
            (x + radius, y - radius),
            (x - radius, y + radius),
            (x + radius, y + radius),
        ];
        for (cx, cy) in corners {
            let gx = (cx / FP_SCALE) as u32;
            let gy = (cy / FP_SCALE) as u32;
            if self.is_solid(gx, gy) {
                return true;
            }
        }
        false
    }

    /// Cast a ray from (x, y) at angle, return distance to first wall hit
    /// and the grid coordinates of the hit cell. Uses DDA algorithm.
    /// All inputs/outputs in fixed-point (×1000).
    pub fn cast_ray(&self, x: i32, y: i32, angle: i32) -> RayHit {
        // Convert to f64 for DDA — this function is used for both
        // combat raycasting and rendering. When we move rendering on-chain,
        // we'll replace with fixed-point DDA.
        let px = x as f64 / FP_SCALE as f64;
        let py = y as f64 / FP_SCALE as f64;
        let a = angle as f64 / 1000.0;

        let dir_x = a.cos();
        let dir_y = a.sin();

        // Current grid cell
        let mut map_x = px as i32;
        let mut map_y = py as i32;

        // Length of ray from one x/y-side to next x/y-side
        let delta_dist_x = if dir_x.abs() < 1e-10 {
            1e30
        } else {
            (1.0 / dir_x).abs()
        };
        let delta_dist_y = if dir_y.abs() < 1e-10 {
            1e30
        } else {
            (1.0 / dir_y).abs()
        };

        // Direction to step in x/y (either +1 or -1)
        let step_x: i32;
        let step_y: i32;

        // Length of ray from current position to next x/y-side
        let mut side_dist_x: f64;
        let mut side_dist_y: f64;

        if dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (px - map_x as f64) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = (map_x as f64 + 1.0 - px) * delta_dist_x;
        }

        if dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (py - map_y as f64) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = (map_y as f64 + 1.0 - py) * delta_dist_y;
        }

        // DDA loop
        let mut side: u8 = 0; // 0 = x-side hit, 1 = y-side hit
        let max_steps = 64;

        for _ in 0..max_steps {
            if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x;
                side = 0;
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y;
                side = 1;
            }

            if map_x < 0 || map_y < 0 {
                break;
            }

            let tile = self.get_tile(map_x as u32, map_y as u32);
            match tile {
                TileType::Wall(tex) => {
                    let dist = if side == 0 {
                        side_dist_x - delta_dist_x
                    } else {
                        side_dist_y - delta_dist_y
                    };

                    // Calculate wall hit position (0.0 - 1.0) for texture mapping
                    let wall_x = if side == 0 {
                        py + dist * dir_y
                    } else {
                        px + dist * dir_x
                    };
                    let wall_x = wall_x - wall_x.floor();

                    return RayHit {
                        distance: (dist * FP_SCALE as f64) as i32,
                        grid_x: map_x as u32,
                        grid_y: map_y as u32,
                        side,
                        texture: tex,
                        wall_x_frac: (wall_x * FP_SCALE as f64) as i32,
                        hit_type: RayHitType::Wall,
                    };
                }
                TileType::Door(DoorState::Closed)
                | TileType::Door(DoorState::Closing(_))
                | TileType::Door(DoorState::LockedRed)
                | TileType::Door(DoorState::LockedBlue) => {
                    let dist = if side == 0 {
                        side_dist_x - delta_dist_x
                    } else {
                        side_dist_y - delta_dist_y
                    };

                    let wall_x = if side == 0 {
                        py + dist * dir_y
                    } else {
                        px + dist * dir_x
                    };
                    let wall_x = wall_x - wall_x.floor();

                    return RayHit {
                        distance: (dist * FP_SCALE as f64) as i32,
                        grid_x: map_x as u32,
                        grid_y: map_y as u32,
                        side,
                        texture: 15,
                        wall_x_frac: (wall_x * FP_SCALE as f64) as i32,
                        hit_type: RayHitType::Door,
                    };
                }
                _ => {} // continue through empty/open doors
            }
        }

        // No hit — return max distance
        RayHit {
            distance: 64 * FP_SCALE,
            grid_x: 0,
            grid_y: 0,
            side: 0,
            texture: 0,
            wall_x_frac: 0,
            hit_type: RayHitType::None,
        }
    }

    /// E1M1-inspired test level — military base theme.
    /// Designed with classic Doom pacing: safe start → exploration → escalation → exit.
    pub fn e1m1() -> Self {
        // 24×24 grid — themed rooms connected by corridors
        // W = Wall, . = empty, D = door, X = exit
        #[rustfmt::skip]
        let layout: Vec<&str> = vec![
            //0123456789012345678901234
            "WWWWWWWWWWWWWWWWWWWWWWWW", // 0
            "W..........W...........W", // 1  Start hall (bright, safe)
            "W..........W...........W", // 2
            "W..........W...........W", // 3
            "W..........W...........W", // 4
            "WWWWW.WWWWWWWWWW.WWWWWWW", // 5  Corridor with two branches
            "W..........W..........WW", // 6  West armory (storage)
            "W..........D..........WW", // 7
            "W..........W..........WW", // 8  East corridor
            "WWWWWDWWWWWWWWWWWDWWWWWW", // 9  Doors to deeper areas
            "W..........W..........WW", // 10 Ritual hall (west)
            "W..........W..........WW", // 11
            "W..........W..........WW", // 12
            "W..........W..........WW", // 13
            "WWWWWW.WWWWWWWWWWWWWWWWW", // 14
            "W..........W..........WW", // 15 Dark corridor
            "W..........D..........WW", // 16
            "W..........W..........WW", // 17
            "WWWWWDWWWWWWWWWWW.WWWWWW", // 18 Final wing doors
            "W..........W..........WW", // 19 Command center
            "W..........W..........WW", // 20
            "W..........W..........WW", // 21
            "W..........W.........XWW", // 22 Exit
            "WWWWWWWWWWWWWWWWWWWWWWWW", // 23
        ];

        // Sector map — defines theme per room
        // S = Start (bright), T = Start center (tech floor), A = Armory, C = Corridor,
        // H = Ritual Hall, D = Dark corridor, K = Command center, E = Exit area
        #[rustfmt::skip]
        let sector_map: Vec<&str> = vec![
            "WWWWWWWWWWWWWWWWWWWWWWWW", // 0
            "WSSSSSSSSSSWCCCCCCCCCCCW", // 1  Start(10) | Corridor(11)
            "WSSTTTTTTSSWCCCCCCCCCCCW", // 2  Tech floor center path
            "WSSTTTTTTSSWCCCCCCCCCCCW", // 3
            "WSSSSSSSSSSWCCCCCCCCCCCW", // 4
            "WWWWWSWWWWWWWWWWWCWWWWWW", // 5
            "WAAAAAAAAAAWCCCCCCCCCCWW", // 6  Armory(10) | Corridor(10)
            "WAAAAAAAAAADCCCCCCCCCCWW", // 7
            "WAAAAAAAAAAWCCCCCCCCCCWW", // 8
            "WWWWWDWWWWWWWWWWWDWWWWWW", // 9
            "WHHHHHHHHHHWDDDDDDDDDDWW", // 10 Ritual(10) | Dark(10)
            "WHHHHHHHHHHWDDDDDDDDDDWW", // 11
            "WHHHHHHHHHHWDDDDDDDDDDWW", // 12
            "WHHHHHHHHHHWDDDDDDDDDDWW", // 13
            "WWWWWWHWWWWWWWWWWWWWWWWW", // 14
            "WDDDDDDDDDDWKKKKKKKKKKWW", // 15 Dark(10) | Command(10)
            "WDDDDDDDDDDWKKKKKKKKKKWW", // 16
            "WDDDDDDDDDDWKKKKKKKKKKWW", // 17
            "WWWWWDWWWWWWWWWWWKWWWWWW", // 18
            "WKKKKKKKKKKWEEEEEEEEEEWW", // 19 Command(10) | Exit(10)
            "WKKKKKKKKKKWEEEEEEEEEEWW", // 20
            "WKKKKKKKKKKWEEEEEEEEEEWW", // 21
            "WKKKKKKKKKKWEEEEEEEEEEWW", // 22
            "WWWWWWWWWWWWWWWWWWWWWWWW", // 23
        ];

        let height = layout.len() as u32;
        let width = layout[0].len() as u32;
        let mut tiles = Vec::with_capacity((width * height) as usize);
        let mut sectors = Vec::with_capacity((width * height) as usize);

        for (row_idx, row) in layout.iter().enumerate() {
            let sector_row = sector_map[row_idx];
            for (col_idx, ch) in row.chars().enumerate() {
                tiles.push(match ch {
                    'W' => TileType::Wall(0),
                    'D' => TileType::Door(DoorState::Closed),
                    'X' => TileType::Exit,
                    _ => TileType::Empty,
                });

                let sector_ch = sector_row.as_bytes().get(col_idx).copied().unwrap_or(b'C');
                sectors.push(match sector_ch {
                    b'S' => Sector::new(0, 1000, 3, 0, 180),      // Start hall: moderate
                    b'T' => Sector::new(0, 1000, 5, 0, 190),      // Start center: tech floor, slightly brighter
                    b'A' => Sector::new(0, 1000, 5, 0, 140),      // Armory: dim industrial
                    b'H' => Sector::new(0, 1100, 7, 2, 120)       // Ritual hall: tall ceiling, NUKAGE floor, dim
                        .with_effect(LightEffect::Flicker),
                    b'D' => Sector::new(0, 900, 4, 1, 100)        // Dark corridor: low ceiling, dark
                        .with_effect(LightEffect::Flicker),
                    b'K' => Sector::new(100, 1000, 6, 0, 160),    // Command center: slightly raised, moderate
                    b'E' => Sector::new(200, 900, 7, 2, 60)       // Exit: raised, very dark, ominous
                        .with_effect(LightEffect::Pulse),
                    _     => Sector::new(0, 1000, 3, 0, 160),     // Corridors: moderate
                });
            }
        }

        // Helper: convert grid position to fixed-point center of cell
        let c = |gx: i32, gy: i32| -> (i32, i32) {
            (gx * FP_SCALE + 500, gy * FP_SCALE + 500)
        };

        DoomMap {
            width,
            height,
            tiles,
            sectors,
            // Player starts in the safe hall, facing east
            player_start: (2 * FP_SCALE + 500, 3 * FP_SCALE + 500, 0),
            enemy_spawns: vec![
                // ── Start hall + east corridor: EMPTY — explore freely ──

                // ── Armory (behind door): 1 enemy guards the shotgun ──
                (c(5, 7).0, c(5, 7).1, EnemyType::Imp),

                // ── Ritual hall: 1 enemy in the dark ──
                (c(5, 12).0, c(5, 12).1, EnemyType::Imp),

                // ── Dark corridor (east): 1 lurking sergeant ──
                (c(17, 12).0, c(17, 12).1, EnemyType::Sergeant),

                // ── Dark lower corridor: 1 demon ambush (scary moment) ──
                (c(5, 16).0, c(5, 16).1, EnemyType::Demon),

                // ── Command center: 1 guard ──
                (c(5, 20).0, c(5, 20).1, EnemyType::Sergeant),

                // ── Exit area: 1 final challenge ──
                (c(17, 21).0, c(17, 21).1, EnemyType::Imp),
            ],
            item_spawns: vec![
                // ── Start hall: ammo clip to get going ──
                (c(5, 1).0, c(5, 1).1, ItemType::AmmoClip),

                // ── East corridor: health ──
                (c(19, 1).0, c(19, 1).1, ItemType::HealthPack),

                // ── Armory (reward for exploring): shotgun + shells ──
                (c(5, 8).0, c(5, 8).1, ItemType::Shotgun),
                (c(2, 6).0, c(2, 6).1, ItemType::ShellBox),
                (c(9, 6).0, c(9, 6).1, ItemType::AmmoClip),

                // ── Ritual hall: medikit (reward for clearing) ──
                (c(5, 12).0, c(5, 12).1, ItemType::Medikit),

                // ── Dark corridor: armor (risky reward) ──
                (c(18, 10).0, c(18, 10).1, ItemType::Armor),

                // ── Before lower areas: ammo ──
                (c(3, 15).0, c(3, 15).1, ItemType::AmmoClip),
                (c(15, 16).0, c(15, 16).1, ItemType::ShellBox),

                // ── Command center: health + ammo for final push ──
                (c(5, 19).0, c(5, 19).1, ItemType::HealthPack),
                (c(9, 20).0, c(9, 20).1, ItemType::AmmoBox),

                // ── Exit area: medikit for survivors ──
                (c(18, 22).0, c(18, 22).1, ItemType::Medikit),
            ],
            decorations: vec![
                // ══ START HALL — clean military base ══
                // Torches flanking the south exit passage (functional — they light the way out)
                Decoration::new(DecorationType::TallGreenTorch, c(4, 4).0, c(4, 4).1),
                Decoration::new(DecorationType::TallGreenTorch, c(6, 4).0, c(6, 4).1),
                // Dead soldier near the wall — something happened before you arrived
                Decoration::new(DecorationType::DeadPlayer, c(5, 2).0, c(5, 2).1),

                // ══ EAST CORRIDOR ══
                Decoration::new(DecorationType::Barrel, c(20, 2).0, c(20, 2).1),

                // ══ ARMORY — barrels along walls, storage room feel ══
                Decoration::new(DecorationType::Barrel, c(1, 6).0, c(1, 6).1),
                Decoration::new(DecorationType::Barrel, c(1, 8).0, c(1, 8).1),
                Decoration::new(DecorationType::Barrel, c(10, 8).0, c(10, 8).1),

                // ══ RITUAL HALL — candelabra centerpiece, red torches at corners ══
                Decoration::new(DecorationType::Candelabra, c(6, 12).0, c(6, 12).1),
                Decoration::new(DecorationType::TallRedTorch, c(1, 10).0, c(1, 10).1),
                Decoration::new(DecorationType::TallRedTorch, c(10, 13).0, c(10, 13).1),
                Decoration::new(DecorationType::SkullsAndCandles, c(5, 10).0, c(5, 10).1),

                // ══ DARK CORRIDORS (east) — sparse, unsettling ══
                Decoration::new(DecorationType::SkullOnStick, c(20, 11).0, c(20, 11).1),
                Decoration::new(DecorationType::DeadPlayer, c(17, 12).0, c(17, 12).1),

                // ══ DARK LOWER CORRIDOR ══
                Decoration::new(DecorationType::HangingBody, c(5, 15).0, c(5, 15).1),

                // ══ COMMAND CENTER — columns framing the entrance ══
                Decoration::new(DecorationType::Column, c(1, 19).0, c(1, 19).1),
                Decoration::new(DecorationType::Column, c(10, 19).0, c(10, 19).1),

                // ══ EXIT AREA — ominous, you know you're at the end ══
                Decoration::new(DecorationType::EvilEye, c(14, 20).0, c(14, 20).1),
                Decoration::new(DecorationType::TallRedPillar, c(14, 22).0, c(14, 22).1),
                Decoration::new(DecorationType::TallRedPillar, c(20, 22).0, c(20, 22).1),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RayHitType {
    Wall,
    Door,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct RayHit {
    pub distance: i32,      // fixed-point distance
    pub grid_x: u32,
    pub grid_y: u32,
    pub side: u8,           // 0 = x-side, 1 = y-side (for shading)
    pub texture: u8,        // texture index
    pub wall_x_frac: i32,  // where on the wall was hit (0-1000) for texture U coord
    pub hit_type: RayHitType,
}
