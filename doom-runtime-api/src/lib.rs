//! Runtime API for on-chain DOOM.
//!
//! Defines the `DoomApi` trait that the runtime implements. Validators use this
//! to read game state and render frames on behalf of connected clients.
//!
//! - `game_state`: Returns the raw SCALE-encoded GameState for a player.
//! - `render_frame`: Reads the player's GameState from storage, runs the
//!   raycaster + sprite compositor natively, and returns a 320×200 RGBA
//!   framebuffer (256,000 bytes).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;

use codec::Codec;
use doom_engine::GameState;

sp_api::decl_runtime_apis! {
    /// Runtime API for the DOOM pallet.
    ///
    /// Called by RPC handlers on the validator node. The `render_frame` method
    /// executes in native context (not WASM), so it has access to `std` and
    /// can use floating-point math for the raycaster without affecting consensus.
    pub trait DoomApi<AccountId> where AccountId: Codec {
        /// Get the current game state for a player. Returns None if no active game.
        fn game_state(player: AccountId) -> Option<GameState>;

        /// Render the current frame for a player. Returns a 320×200×4 RGBA buffer
        /// (256,000 bytes), or an empty Vec if no active game.
        fn render_frame(player: AccountId) -> Vec<u8>;

        /// Check if a player has an active game session.
        fn has_active_game(player: AccountId) -> bool;
    }
}
