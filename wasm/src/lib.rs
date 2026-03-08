use doom_engine::*;
use wasm_bindgen::prelude::*;

/// WASM-exposed game instance.
#[wasm_bindgen]
pub struct DoomGame {
    state: GameState,
    framebuffer: Framebuffer,
}

#[wasm_bindgen]
impl DoomGame {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let map = DoomMap::e1m1();
        let state = GameState::new(map);
        Self {
            state,
            framebuffer: Framebuffer::new(),
        }
    }

    pub fn tick(&mut self, input_codes: &[u8]) {
        let inputs: Vec<PlayerInput> = input_codes
            .iter()
            .filter_map(|&code| match code {
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
                _ => None,
            })
            .collect();
        self.state.tick(&inputs);
    }

    /// Render the current frame. Read pixels via rgba_ptr()/rgba_len().
    pub fn render(&mut self) {
        render_frame(&self.state, &mut self.framebuffer);
    }

    pub fn rgba_ptr(&self) -> *const u8 {
        self.framebuffer.rgba.as_ptr()
    }

    pub fn rgba_len(&self) -> usize {
        self.framebuffer.rgba.len()
    }

    // --- HUD getters ---
    pub fn player_health(&self) -> i32 { self.state.player.health }
    pub fn player_armor(&self) -> i32 { self.state.player.armor }
    pub fn player_ammo(&self) -> i32 { self.state.player.ammo }
    pub fn player_shells(&self) -> i32 { self.state.player.shells }
    pub fn current_weapon(&self) -> u8 {
        match self.state.player.current_weapon {
            WeaponType::Fist => 0,
            WeaponType::Pistol => 1,
            WeaponType::Shotgun => 2,
        }
    }
    pub fn has_shotgun(&self) -> bool { self.state.player.has_shotgun }
    pub fn projectile_count(&self) -> usize { self.state.projectiles.len() }
    pub fn player_kills(&self) -> u32 { self.state.player.kills }
    pub fn player_alive(&self) -> bool { self.state.player.alive }
    pub fn game_over(&self) -> bool { self.state.game_over }
    pub fn level_complete(&self) -> bool { self.state.level_complete }
    pub fn current_tick(&self) -> u64 { self.state.tick }
    pub fn alive_enemies(&self) -> usize { self.state.alive_enemy_count() }
    pub fn total_enemies(&self) -> usize { self.state.total_enemy_count() }
    pub fn has_red_key(&self) -> bool { self.state.player.has_red_key }
    pub fn has_blue_key(&self) -> bool { self.state.player.has_blue_key }
    pub fn player_x(&self) -> f64 { self.state.player.x_f64() }
    pub fn player_y(&self) -> f64 { self.state.player.y_f64() }
    pub fn player_angle(&self) -> f64 { self.state.player.angle_rad() }
    pub fn screen_width(&self) -> usize { SCREEN_WIDTH }
    pub fn screen_height(&self) -> usize { SCREEN_HEIGHT }

    pub fn reset(&mut self) {
        let map = DoomMap::e1m1();
        self.state = GameState::new(map);
    }
}
