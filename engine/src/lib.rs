//! # doom-engine
//!
//! Pure Rust DOOM-style game engine designed for on-chain execution.
//!
//! All game logic uses fixed-point integer math (i32 × 1000) to be
//! deterministic across all validators. The renderer produces a
//! palette-indexed framebuffer (320×200) with no I/O — designed to
//! be movable on-chain inside a Substrate pallet's `on_initialize`.
//!
//! ## Architecture
//!
//! - `types` — Core data types, fixed-point constants, player/enemy/item structs
//! - `map` — Grid-based level with DDA raycasting and collision detection
//! - `game` — Game state and tick loop (inputs → physics → AI → events)
//! - `renderer` — Raycaster producing a 320×200 framebuffer (palette-indexed)

pub mod assets;
pub mod game;
pub mod map;
pub mod renderer;
pub mod types;

// Re-export primary API surface
pub use game::GameState;
pub use map::DoomMap;
pub use renderer::{
    palette_color_rgb, render_automap, render_frame, render_title_screen, Framebuffer,
    FRAMEBUFFER_SIZE, SCREEN_HEIGHT, SCREEN_WIDTH,
};
pub use types::{
    Decoration, DecorationType, DoorState, DoomRng, Enemy, EnemyAiState, EnemyType, GameEvent,
    Item, ItemType, LightEffect, Player, PlayerInput, Projectile, ProjectileSource, Sector,
    TileType, WeaponType,
};
