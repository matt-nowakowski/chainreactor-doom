//! JSON-RPC extension for on-chain DOOM.
//!
//! Exposes three RPC methods:
//! - `doom_renderFrame(account)` → hex-encoded 320×200 RGBA framebuffer
//! - `doom_getState(account)` → hex-encoded SCALE GameState
//! - `doom_hasActiveGame(account)` → bool
//!
//! These are called by the browser thin client. The validator reads game state
//! from on-chain storage and renders frames natively (not in WASM).

use std::sync::Arc;

use codec::{Codec, Encode};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;

use doom_runtime_api::DoomApi;

/// Encode bytes to hex string with 0x prefix.
fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// RPC trait for DOOM game interaction.
#[rpc(server)]
pub trait DoomRpcApi<AccountId> {
    /// Render the current frame for a player.
    /// Returns a hex-encoded 320×200×4 RGBA buffer.
    #[method(name = "doom_renderFrame")]
    fn render_frame(&self, player: AccountId) -> RpcResult<String>;

    /// Get the raw game state for a player (hex-encoded SCALE bytes).
    #[method(name = "doom_getState")]
    fn get_state(&self, player: AccountId) -> RpcResult<Option<String>>;

    /// Check if a player has an active game.
    #[method(name = "doom_hasActiveGame")]
    fn has_active_game(&self, player: AccountId) -> RpcResult<bool>;
}

/// RPC handler implementation.
pub struct DoomRpc<C, Block> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<Block>,
}

impl<C, Block> DoomRpc<C, Block> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

#[async_trait]
impl<C, Block, AccountId> DoomRpcApiServer<AccountId> for DoomRpc<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DoomApi<Block, AccountId>,
    AccountId: Codec + Send + 'static,
{
    fn render_frame(&self, player: AccountId) -> RpcResult<String> {
        let api = self.client.runtime_api();
        let best = self.client.info().best_hash;

        // Get game state from runtime (works in WASM context)
        let state = api.game_state(best, player).map_err(|e| {
            jsonrpsee::core::Error::Custom(format!("Runtime API error: {:?}", e))
        })?;

        match state {
            Some(state) => {
                // Render locally in RPC handler (native context — always has std)
                let mut fb = doom_engine::Framebuffer::new();
                doom_engine::render_frame(&state, &mut fb);
                Ok(to_hex(&fb.rgba))
            }
            None => Err(jsonrpsee::core::Error::Custom(
                "No active game for this player".to_string(),
            )),
        }
    }

    fn get_state(&self, player: AccountId) -> RpcResult<Option<String>> {
        let api = self.client.runtime_api();
        let best = self.client.info().best_hash;

        let state = api.game_state(best, player).map_err(|e| {
            jsonrpsee::core::Error::Custom(format!("Runtime API error: {:?}", e))
        })?;

        Ok(state.map(|s| to_hex(&s.encode())))
    }

    fn has_active_game(&self, player: AccountId) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let best = self.client.info().best_hash;

        api.has_active_game(best, player).map_err(|e| {
            jsonrpsee::core::Error::Custom(format!("Runtime API error: {:?}", e))
        })
    }
}
