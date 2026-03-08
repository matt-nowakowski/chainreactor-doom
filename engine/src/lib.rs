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
//! - `fixmath` — Fixed-point trig (sin/cos/atan2/sqrt) — `no_std` via libm
//! - `renderer` — Raycaster producing a 320×200 framebuffer (std-only, used by RPC)
//! - `assets` — Textures, sprites, flats (std-only, used by renderer)

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod fixmath;
pub mod game;
pub mod map;
pub mod types;

// Renderer and assets only available in std (used by native RPC, not on-chain WASM)
#[cfg(feature = "std")]
pub mod assets;
#[cfg(feature = "std")]
pub mod renderer;

// Re-export primary API surface
pub use game::GameState;
pub use map::DoomMap;
pub use types::{
    Decoration, DecorationType, DoorState, DoomRng, Enemy, EnemyAiState, EnemyType, GameEvent,
    Item, ItemType, LightEffect, Player, PlayerInput, Projectile, ProjectileSource, Sector,
    TileType, WeaponType,
};

// Renderer exports only in std
#[cfg(feature = "std")]
pub use renderer::{
    palette_color_rgb, render_automap, render_frame, render_title_screen, Framebuffer,
    FRAMEBUFFER_SIZE, SCREEN_HEIGHT, SCREEN_WIDTH,
};
