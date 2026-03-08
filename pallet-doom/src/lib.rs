//! # pallet-doom
//!
//! Substrate pallet that runs DOOM on-chain. Each player gets an independent
//! game state stored in a `StorageMap`. Inputs are submitted as extrinsics,
//! and all active games are ticked in `on_initialize`.
//!
//! Frame rendering happens off-chain via a custom runtime API — validators
//! render the current state when a client requests it via RPC.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use doom_engine::PlayerInput;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use doom_engine::{DoomMap, GameState};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// Maximum concurrent active game sessions.
    const MAX_ACTIVE_PLAYERS: u32 = 32;

    /// Maximum inputs per extrinsic submission.
    const MAX_INPUTS_PER_CALL: u32 = 16;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    // ═══ STORAGE ═══

    /// Per-player game state. The full GameState is stored on-chain.
    /// ~3-5KB per player, SCALE-encoded.
    #[pallet::storage]
    pub type GameStates<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, GameState>;

    /// Input queue — inputs submitted during the current block, processed in
    /// the next block's `on_initialize`. Bounded to MAX_INPUTS_PER_CALL.
    #[pallet::storage]
    pub type InputQueue<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<u8, ConstU32<MAX_INPUTS_PER_CALL>>>;

    /// Set of accounts with active (non-finished) games. Bounded to cap weight.
    #[pallet::storage]
    pub type ActivePlayers<T: Config> =
        StorageValue<_, BoundedVec<T::AccountId, ConstU32<MAX_ACTIVE_PLAYERS>>, ValueQuery>;

    /// The default map used for new games. Set via `set_map` (sudo).
    #[pallet::storage]
    pub type DefaultMap<T: Config> = StorageValue<_, DoomMap>;

    // ═══ EVENTS ═══

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A new game session started.
        GameStarted { player: T::AccountId },
        /// Player completed the level.
        LevelCompleted { player: T::AccountId, ticks: u64 },
        /// Player died.
        GameOver { player: T::AccountId, kills: u32, ticks: u64 },
        /// Game was reset by the player.
        GameReset { player: T::AccountId },
    }

    // ═══ ERRORS ═══

    #[pallet::error]
    pub enum Error<T> {
        /// No map has been configured. Call `set_map` first.
        NoMapConfigured,
        /// Player already has an active game. Reset first.
        GameAlreadyActive,
        /// No active game for this player.
        NoActiveGame,
        /// Too many concurrent players.
        TooManyPlayers,
        /// Invalid input value.
        InvalidInput,
    }

    // ═══ HOOKS ═══

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
            let active = ActivePlayers::<T>::get();
            let mut total_weight = T::DbWeight::get().reads(1); // read ActivePlayers

            let mut finished: Vec<T::AccountId> = Vec::new();

            for account in active.iter() {
                // Read game state + input queue
                total_weight += T::DbWeight::get().reads(2);

                let inputs_raw = InputQueue::<T>::take(account);
                let inputs: Vec<PlayerInput> = inputs_raw
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|&b| input_from_u8(b))
                    .collect();

                if let Some(mut state) = GameStates::<T>::get(account) {
                    // Tick the game (even with empty inputs — enemies move, doors animate)
                    state.tick(&inputs);

                    // Check for game-ending events
                    if state.level_complete {
                        Self::deposit_event(Event::LevelCompleted {
                            player: account.clone(),
                            ticks: state.tick,
                        });
                        finished.push(account.clone());
                    } else if state.game_over {
                        Self::deposit_event(Event::GameOver {
                            player: account.clone(),
                            kills: state.player.kills,
                            ticks: state.tick,
                        });
                        finished.push(account.clone());
                    }

                    // Write updated state
                    GameStates::<T>::insert(account, state);
                    total_weight += T::DbWeight::get().writes(1);
                }
            }

            // Remove finished players from active list
            if !finished.is_empty() {
                ActivePlayers::<T>::mutate(|players| {
                    players.retain(|p| !finished.contains(p));
                });
                total_weight += T::DbWeight::get().writes(1);
            }

            total_weight
        }
    }

    // ═══ EXTRINSICS ═══

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Start a new game session. Creates a fresh GameState from the default map.
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn new_game(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let map = DefaultMap::<T>::get().ok_or(Error::<T>::NoMapConfigured)?;
            ensure!(!GameStates::<T>::contains_key(&who), Error::<T>::GameAlreadyActive);

            let state = GameState::new(map);
            GameStates::<T>::insert(&who, state);

            // Add to active players
            ActivePlayers::<T>::try_mutate(|players| {
                players.try_push(who.clone()).map_err(|_| Error::<T>::TooManyPlayers)
            })?;

            Self::deposit_event(Event::GameStarted { player: who });
            Ok(())
        }

        /// Submit player inputs for the current tick.
        /// Inputs are queued and processed in the next block's `on_initialize`.
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000_000, 0))]
        pub fn submit_input(
            origin: OriginFor<T>,
            inputs: BoundedVec<u8, ConstU32<MAX_INPUTS_PER_CALL>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(GameStates::<T>::contains_key(&who), Error::<T>::NoActiveGame);

            // Replace any existing queued inputs (latest wins)
            InputQueue::<T>::insert(&who, inputs);
            Ok(())
        }

        /// Reset the current game (restart level).
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::from_parts(50_000_000, 0))]
        pub fn reset_game(origin: OriginFor<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            let map = DefaultMap::<T>::get().ok_or(Error::<T>::NoMapConfigured)?;
            let state = GameState::new(map);
            GameStates::<T>::insert(&who, state);

            // Ensure player is in active list
            ActivePlayers::<T>::try_mutate(|players| -> DispatchResult {
                if !players.contains(&who) {
                    players.try_push(who.clone()).map_err(|_| Error::<T>::TooManyPlayers)?;
                }
                Ok(())
            })?;

            Self::deposit_event(Event::GameReset { player: who });
            Ok(())
        }

        /// Admin: set the default map for new games. Requires sudo.
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::from_parts(100_000_000, 0))]
        pub fn set_map(origin: OriginFor<T>, map: DoomMap) -> DispatchResult {
            ensure_root(origin)?;
            DefaultMap::<T>::put(map);
            Ok(())
        }
    }
}

/// Convert a u8 input byte to a PlayerInput enum.
fn input_from_u8(b: u8) -> Option<PlayerInput> {
    match b {
        0 => Some(PlayerInput::Forward),
        1 => Some(PlayerInput::Backward),
        2 => Some(PlayerInput::TurnLeft),
        3 => Some(PlayerInput::TurnRight),
        4 => Some(PlayerInput::StrafeLeft),
        5 => Some(PlayerInput::StrafeRight),
        6 => Some(PlayerInput::Shoot),
        7 => Some(PlayerInput::Use),
        8 => Some(PlayerInput::WeaponNext),
        9 => Some(PlayerInput::WeaponPrev),
        10 => Some(PlayerInput::Weapon1),
        11 => Some(PlayerInput::Weapon2),
        12 => Some(PlayerInput::Weapon3),
        13 => Some(PlayerInput::Weapon4),
        14 => Some(PlayerInput::Weapon5),
        15 => Some(PlayerInput::ToggleAutomap),
        _ => None,
    }
}
