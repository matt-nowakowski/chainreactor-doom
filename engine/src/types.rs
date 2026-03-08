use serde::{Deserialize, Serialize};

/// Player input actions — submitted as extrinsics on-chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerInput {
    Forward,
    Backward,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Shoot,
    Use,          // open doors, activate switches
    WeaponNext,   // cycle to next weapon
    WeaponPrev,   // cycle to previous weapon
    Weapon1,      // fist
    Weapon2,      // pistol
    Weapon3,      // shotgun
}

/// Tile types in the map grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileType {
    Empty,
    Wall(u8), // texture index (0-15)
    Door(DoorState),
    Exit,
}

/// Light effect for a sector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightEffect {
    None,
    Flicker,    // random flicker between light_level and light_level/2
    Pulse,      // smooth sinusoidal pulse
    Strobe,     // fast on/off
}

/// Sector data for a grid cell — controls floor/ceiling heights, textures, and lighting.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sector {
    pub floor_height: i32,    // fixed-point (0 = ground level, 500 = half tile up)
    pub ceiling_height: i32,  // fixed-point (1000 = standard height)
    pub floor_tex: u8,        // index into FLATS array
    pub ceiling_tex: u8,      // index into FLATS array
    pub light_level: u8,      // 0-255 base brightness
    pub light_effect: LightEffect,
}

impl Default for Sector {
    fn default() -> Self {
        Self {
            floor_height: 0,
            ceiling_height: 1000,
            floor_tex: 3,     // FLOOR0_1 (index 3 in our FLATS array)
            ceiling_tex: 0,   // CEIL3_5
            light_level: 200, // moderately bright
            light_effect: LightEffect::None,
        }
    }
}

impl Sector {
    pub fn new(floor_h: i32, ceil_h: i32, floor_tex: u8, ceil_tex: u8, light: u8) -> Self {
        Self {
            floor_height: floor_h,
            ceiling_height: ceil_h,
            floor_tex,
            ceiling_tex: ceil_tex,
            light_level: light,
            light_effect: LightEffect::None,
        }
    }

    pub fn with_effect(mut self, effect: LightEffect) -> Self {
        self.light_effect = effect;
        self
    }

    /// Get the effective light level for a given tick, accounting for light effects.
    pub fn effective_light(&self, tick: u64) -> u8 {
        match self.light_effect {
            LightEffect::None => self.light_level,
            LightEffect::Flicker => {
                // Deterministic flicker using tick-based hash
                let hash = tick.wrapping_mul(2654435761) >> 24;
                if hash & 3 == 0 {
                    self.light_level / 2
                } else {
                    self.light_level
                }
            }
            LightEffect::Pulse => {
                // Smooth sinusoidal pulse (period ~120 ticks = 8 seconds)
                let phase = (tick % 120) as f64 * 6.283 / 120.0;
                let factor = (phase.sin() + 1.0) / 2.0; // 0.0 - 1.0
                let min_light = self.light_level as f64 * 0.4;
                let range = self.light_level as f64 - min_light;
                (min_light + range * factor) as u8
            }
            LightEffect::Strobe => {
                // Fast on/off every 4 ticks
                if (tick / 4) % 2 == 0 {
                    self.light_level
                } else {
                    self.light_level / 3
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorState {
    Closed,
    Opening(u8),    // progress 0-100
    Open,
    OpenWait(u8),   // open, waiting N ticks before auto-close
    Closing(u8),    // progress 100-0
    LockedRed,
    LockedBlue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyType {
    Imp,      // fireball attack, medium health
    Demon,    // melee only, fast, tanky
    Sergeant, // hitscan attack, low health
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnemyAiState {
    Idle,
    Alerted(u8),   // reaction delay ticks before chasing
    Chasing,
    Attacking,
    Pain,          // pain state — duration based on pain_chance roll
    Dead,
}

/// Weapon types available to the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponType {
    Fist,
    Pistol,
    Shotgun,
}

/// Visual effect type for bullet puffs and blood splats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectType {
    BulletPuff,  // yellow-white flash when bullet hits a wall
    BloodSplat,  // red splash when bullet hits an enemy
}

/// A temporary visual effect in the world (bullet puff, blood, etc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEffect {
    pub x: i32,        // fixed-point world position
    pub y: i32,
    pub effect_type: EffectType,
    pub timer: u8,     // ticks remaining (counts down to 0)
}

/// A projectile traveling through the world (Imp fireballs, etc).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub x: i32,        // fixed-point
    pub y: i32,        // fixed-point
    pub vx: i32,       // velocity x per tick (fixed-point)
    pub vy: i32,       // velocity y per tick (fixed-point)
    pub damage: i32,
    pub source: ProjectileSource,
    pub alive: bool,
    pub sprite_id: u8, // for rendering
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectileSource {
    Enemy(EnemyType),
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemType {
    HealthPack,  // +25 health
    Medikit,     // +50 health
    AmmoClip,    // +10 ammo (pistol)
    AmmoBox,     // +25 ammo (pistol)
    ShellBox,    // +4 shells
    Shotgun,     // gives shotgun + 8 shells
    Armor,       // +50 armor
    KeyRed,
    KeyBlue,
}

/// Decorative props — non-interactive scenery objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecorationType {
    Barrel,         // BAR1 — explodable barrel
    Column,         // COLU — tech column
    Candelabra,     // CBRA — tall candelabra
    Candlestick,    // CAND — small candlestick
    TallBlueTorch,  // TBLU — tall blue fire torch
    TallGreenTorch, // TGRN — tall green fire torch
    TallRedTorch,   // TRED — tall red fire torch
    ShortBlueTorch, // SMBT — short blue torch
    ShortGreenTorch,// SMGT — short green torch
    ShortRedTorch,  // SMRT — short red torch
    EvilEye,        // CEYE — floating evil eye
    FloatingSkull,  // FSKU — floating skull rock
    TechPillar,     // ELEC — tall tech pillar
    TallGreenPillar,// COL1 — tall green pillar
    ShortGreenPillar,// COL2 — short green pillar
    TallRedPillar,  // COL3 — tall red pillar
    ShortRedPillar, // COL4 — short red pillar
    HeartColumn,    // COL5 — heart column
    SkullColumn,    // COL6 — skull column
    SkullPile,      // POL2 — pile of skulls
    SkullsAndCandles,// POL1 — skulls and candles pile
    SkullColumnTall,// POL3 — tall skull column
    SkullOnStick,   // POL4 — skull impaled on stick
    HangingTwitching,// POL6 — hanging twitching body
    HangingBody,    // GOR1 — hanging body
    DeadPlayer,     // SMIT — bloody mess on ground
}

impl DecorationType {
    /// Whether this decoration blocks player movement (solid).
    pub fn is_solid(&self) -> bool {
        matches!(
            self,
            DecorationType::Barrel
                | DecorationType::Column
                | DecorationType::TechPillar
                | DecorationType::TallGreenPillar
                | DecorationType::ShortGreenPillar
                | DecorationType::TallRedPillar
                | DecorationType::ShortRedPillar
                | DecorationType::HeartColumn
                | DecorationType::SkullColumn
                | DecorationType::SkullColumnTall
        )
    }

    /// Whether this decoration is animated (has 2 frames).
    pub fn is_animated(&self) -> bool {
        matches!(
            self,
            DecorationType::Barrel
                | DecorationType::TallBlueTorch
                | DecorationType::TallGreenTorch
                | DecorationType::TallRedTorch
                | DecorationType::ShortBlueTorch
                | DecorationType::ShortGreenTorch
                | DecorationType::ShortRedTorch
                | DecorationType::EvilEye
                | DecorationType::FloatingSkull
                | DecorationType::SkullColumnTall
                | DecorationType::HangingTwitching
        )
    }
}

/// A decoration instance placed in the world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decoration {
    pub x: i32,       // fixed-point position
    pub y: i32,
    pub deco_type: DecorationType,
}

impl Decoration {
    pub fn new(deco_type: DecorationType, x: i32, y: i32) -> Self {
        Self { x, y, deco_type }
    }
}

/// Player state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub x: i32,       // fixed-point position (× 1000)
    pub y: i32,       // fixed-point position (× 1000)
    pub angle: i32,   // fixed-point angle in milliradians (0 - 6283)
    pub health: i32,
    pub armor: i32,
    pub ammo: i32,
    pub shells: i32,  // shotgun ammo
    pub kills: u32,
    pub has_red_key: bool,
    pub has_blue_key: bool,
    pub alive: bool,
    pub weapon_cooldown: u8,
    pub current_weapon: WeaponType,
    pub has_shotgun: bool,
}

impl Player {
    pub fn new(x: i32, y: i32, angle: i32) -> Self {
        Self {
            x,
            y,
            angle,
            health: 100,
            armor: 0,
            ammo: 50,
            shells: 0,
            kills: 0,
            has_red_key: false,
            has_blue_key: false,
            alive: true,
            weapon_cooldown: 0,
            current_weapon: WeaponType::Pistol,
            has_shotgun: false,
        }
    }

    /// Angle in fixed-point milliradians to f64 radians (for rendering).
    pub fn angle_rad(&self) -> f64 {
        self.angle as f64 / 1000.0
    }

    pub fn x_f64(&self) -> f64 {
        self.x as f64 / 1000.0
    }

    pub fn y_f64(&self) -> f64 {
        self.y as f64 / 1000.0
    }
}

/// Enemy instance state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enemy {
    pub x: i32,          // fixed-point × 1000
    pub y: i32,          // fixed-point × 1000
    pub enemy_type: EnemyType,
    pub health: i32,
    pub ai_state: EnemyAiState,
    pub attack_cooldown: u8,
    pub move_dir: i32,   // current movement angle (for zigzag)
    pub strafe_timer: u8, // ticks until next direction change
    pub last_known_px: i32, // last known player position (for chasing after LOS lost)
    pub last_known_py: i32,
}

impl Enemy {
    pub fn new(enemy_type: EnemyType, x: i32, y: i32) -> Self {
        let health = match enemy_type {
            EnemyType::Imp => 60,
            EnemyType::Demon => 150,
            EnemyType::Sergeant => 30,
        };
        Self {
            x,
            y,
            enemy_type,
            health,
            ai_state: EnemyAiState::Idle,
            attack_cooldown: 0,
            move_dir: 0,
            strafe_timer: 0,
            last_known_px: 0,
            last_known_py: 0,
        }
    }

    pub fn x_f64(&self) -> f64 {
        self.x as f64 / 1000.0
    }

    pub fn y_f64(&self) -> f64 {
        self.y as f64 / 1000.0
    }

    pub fn is_alive(&self) -> bool {
        self.ai_state != EnemyAiState::Dead
    }

    pub fn speed(&self) -> i32 {
        match self.enemy_type {
            EnemyType::Imp => 40,
            EnemyType::Demon => 70,
            EnemyType::Sergeant => 30,
        }
    }

    pub fn damage(&self) -> i32 {
        match self.enemy_type {
            EnemyType::Imp => 15,
            EnemyType::Demon => 25,
            EnemyType::Sergeant => 10,
        }
    }

    pub fn attack_range(&self) -> i32 {
        match self.enemy_type {
            EnemyType::Demon => 1500, // melee only — 1.5 tiles
            _ => 10000,               // ranged — 10 tiles
        }
    }

    /// Pain chance out of 256 (Chocolate Doom values).
    /// Higher = more likely to flinch when hit.
    pub fn pain_chance(&self) -> u8 {
        match self.enemy_type {
            EnemyType::Imp => 200,      // ~78%
            EnemyType::Demon => 180,    // ~70%
            EnemyType::Sergeant => 170, // ~66%
        }
    }

    /// Reaction delay in ticks before an alerted enemy starts chasing.
    pub fn reaction_ticks(&self) -> u8 {
        match self.enemy_type {
            EnemyType::Imp => 3,
            EnemyType::Demon => 2,    // demons are quick to react
            EnemyType::Sergeant => 4, // slower to react
        }
    }

    /// Whether this enemy fires projectiles (vs hitscan or melee).
    pub fn fires_projectile(&self) -> bool {
        matches!(self.enemy_type, EnemyType::Imp)
    }

    /// Projectile speed in fixed-point units per tick.
    pub fn projectile_speed(&self) -> i32 {
        match self.enemy_type {
            EnemyType::Imp => 300, // fireball speed
            _ => 0,
        }
    }
}

/// Item instance state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub x: i32, // fixed-point × 1000
    pub y: i32,
    pub item_type: ItemType,
    pub picked_up: bool,
}

impl Item {
    pub fn new(item_type: ItemType, x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            item_type,
            picked_up: false,
        }
    }
}

/// Game events emitted during a tick (become on-chain events).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    EnemyKilled { enemy_type: EnemyType, x: i32, y: i32 },
    ItemPickedUp { item_type: ItemType },
    PlayerDamaged { amount: i32, source: EnemyType },
    PlayerDied { kills: u32 },
    DoorOpened { x: u32, y: u32 },
    LevelComplete,
}

// Fixed-point math constants (milliradians / 1000ths)
pub const FP_SCALE: i32 = 1000;
pub const TWO_PI: i32 = 6283; // 2π × 1000
pub const PI: i32 = 3141;
pub const HALF_PI: i32 = 1570;

/// Deterministic PRNG for on-chain compatibility.
/// Uses the same table approach as Doom's M_Random (256-entry table).
/// This ensures bit-identical results across all validators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoomRng {
    index: u8,
}

impl DoomRng {
    // Doom's actual random number table (from m_random.c)
    const TABLE: [u8; 256] = [
          0,   8, 109, 220, 222, 241, 149, 107,  75, 248, 254, 140,  16,  66,
         74,  21, 211,  47,  80, 242, 154,  27, 205, 128, 161,  89,  77,  36,
         95, 110,  85,  48, 212, 140, 211, 249,  22,  79, 200,  50,  28, 188,
         52, 140, 202, 120,  68, 145,  62,  70, 184, 190,  91, 197, 152, 224,
        149, 104,  25, 178, 252, 182, 202, 182, 141, 197,   4,  81, 181, 242,
        145,  42,  39, 227, 156, 198, 225, 193, 219,  93, 122, 175, 249,   0,
        175, 143,  70, 239,  46, 246, 163,  53, 163, 109, 168, 135,   2, 235,
         25,  92,  20, 145, 138,  77,  69, 166,  78, 176, 173, 212, 166, 113,
         94, 161,  41,  50, 239,  49, 111, 164,  70,  60,   2,  37, 171,  75,
        136, 156,  11,  56,  42, 146, 138, 229,  73, 146,  77,  61,  98, 196,
        135, 106,  63, 197, 195,  86,  96, 203, 113, 101, 170, 247, 181, 113,
         80, 250, 108,   7, 255, 237, 129, 226,  79, 107, 112, 166, 103, 241,
         24, 223, 239, 120, 198,  58,  60,  82, 128,   3, 184,  66, 143, 224,
        145, 224,  81, 206, 163,  45,  63,  90, 168, 114,  59,  33, 159,  95,
         28, 139, 123,  98, 125, 196,  15,  70, 194, 253,  54,  14, 109, 226,
         71,  17, 161,  93, 186,  87, 244, 138,  20,  52, 123, 133,  67,  39,
         45, 176, 167, 200,  93, 106,  60, 224,  72,  44, 178, 107, 163,  52,
         80,  93, 161, 169,  26,  53, 106, 157, 180,  61,  10,  28, 247, 242,
         66, 232,  58,  25,
    ];

    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// Get next random byte (0-255), deterministic sequence.
    pub fn next(&mut self) -> u8 {
        self.index = self.index.wrapping_add(1);
        Self::TABLE[self.index as usize]
    }

    /// Random value in range [0, max) using rejection-free modulo.
    pub fn range(&mut self, max: i32) -> i32 {
        (self.next() as i32 * max) / 256
    }

    /// Roll a check: returns true if random byte < threshold.
    pub fn check(&mut self, threshold: u8) -> bool {
        self.next() < threshold
    }
}

/// Normalize angle to 0..TWO_PI range.
pub fn normalize_angle(mut angle: i32) -> i32 {
    while angle < 0 {
        angle += TWO_PI;
    }
    while angle >= TWO_PI {
        angle -= TWO_PI;
    }
    angle
}
