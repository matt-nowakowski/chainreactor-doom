#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use crate::fixmath;
use crate::map::DoomMap;
use crate::types::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "substrate")]
use codec::{Decode, Encode};
#[cfg(feature = "substrate")]
use scale_info::TypeInfo;

/// Door auto-close delay in ticks (~4 seconds at 15 tps).
const DOOR_WAIT_TICKS: u8 = 60;

/// Complete game state — everything needed to represent one moment in the game.
/// This is what gets stored on-chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "substrate", derive(Encode, Decode, TypeInfo))]
pub struct GameState {
    pub map: DoomMap,
    pub player: Player,
    pub enemies: Vec<Enemy>,
    pub items: Vec<Item>,
    pub decorations: Vec<Decoration>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<VisualEffect>,
    pub tick: u64,
    pub events: Vec<GameEvent>, // events from the last tick (cleared each tick)
    pub game_over: bool,
    pub level_complete: bool,
    pub rng: DoomRng,
}

impl GameState {
    /// Create a new game from a map.
    pub fn new(map: DoomMap) -> Self {
        let (px, py, pa) = map.player_start;
        let player = Player::new(px, py, pa);

        let enemies = map
            .enemy_spawns
            .iter()
            .map(|(x, y, t)| Enemy::new(*t, *x, *y))
            .collect();

        let items = map
            .item_spawns
            .iter()
            .map(|(x, y, t)| Item::new(*t, *x, *y))
            .collect();

        let decorations = map.decorations.clone();

        Self {
            map,
            player,
            enemies,
            items,
            decorations,
            projectiles: Vec::new(),
            effects: Vec::new(),
            tick: 0,
            events: Vec::new(),
            game_over: false,
            level_complete: false,
            rng: DoomRng::new(),
        }
    }

    /// Process one game tick with the given player inputs.
    /// This is the core function that runs on-chain in `on_initialize`.
    pub fn tick(&mut self, inputs: &[PlayerInput]) {
        self.events.clear();

        if self.game_over || self.level_complete {
            return;
        }

        // 1. Process player inputs
        for input in inputs {
            self.process_input(*input);
        }

        // 2. Update doors (opening, auto-close, closing)
        self.update_doors();

        // 3. Update projectiles (move, collide)
        self.update_projectiles();

        // 4. Run enemy AI
        self.update_enemies();

        // 5. Alert nearby enemies from gunfire (sound propagation)
        if inputs.contains(&PlayerInput::Shoot) && self.player.weapon_cooldown == 0 {
            // The cooldown check is wrong here since we already set it — use a flag
        }
        // Sound alerting is handled inside player_shoot via alert_enemies_from_sound

        // 6. Check item pickups
        self.check_item_pickups();

        // 7. Check exit
        self.check_exit();

        // 8. Decrement cooldowns
        if self.player.weapon_cooldown > 0 {
            self.player.weapon_cooldown -= 1;
        }

        // 9. Update visual effects (decrement timers, remove expired)
        self.effects.retain_mut(|e| {
            if e.timer > 0 {
                e.timer -= 1;
                true
            } else {
                false
            }
        });

        self.tick += 1;
    }

    fn process_input(&mut self, input: PlayerInput) {
        if !self.player.alive {
            return;
        }

        let move_speed: i32 = 100; // fixed-point units per input
        let turn_speed: i32 = 100; // milliradians per input

        match input {
            PlayerInput::Forward => {
                self.move_player(move_speed, 0);
            }
            PlayerInput::Backward => {
                self.move_player(-move_speed, 0);
            }
            PlayerInput::StrafeLeft => {
                self.move_player(0, -move_speed);
            }
            PlayerInput::StrafeRight => {
                self.move_player(0, move_speed);
            }
            PlayerInput::TurnLeft => {
                self.player.angle = normalize_angle(self.player.angle - turn_speed);
            }
            PlayerInput::TurnRight => {
                self.player.angle = normalize_angle(self.player.angle + turn_speed);
            }
            PlayerInput::Shoot => {
                self.player_shoot();
            }
            PlayerInput::Use => {
                self.player_use();
            }
            PlayerInput::WeaponNext => {
                self.cycle_weapon(true);
            }
            PlayerInput::WeaponPrev => {
                self.cycle_weapon(false);
            }
            PlayerInput::Weapon1 => {
                self.player.current_weapon = WeaponType::Fist;
            }
            PlayerInput::Weapon2 => {
                self.player.current_weapon = WeaponType::Pistol;
            }
            PlayerInput::Weapon3 => {
                if self.player.has_shotgun {
                    self.player.current_weapon = WeaponType::Shotgun;
                }
            }
            PlayerInput::Weapon4 => {
                if self.player.has_chaingun {
                    self.player.current_weapon = WeaponType::Chaingun;
                }
            }
            PlayerInput::Weapon5 => {
                if self.player.has_rocket_launcher {
                    self.player.current_weapon = WeaponType::RocketLauncher;
                }
            }
            PlayerInput::ToggleAutomap => {
                self.player.show_automap = !self.player.show_automap;
            }
        }
    }

    /// Cycle through available weapons.
    fn cycle_weapon(&mut self, forward: bool) {
        let weapons: Vec<WeaponType> = {
            let mut w = vec![WeaponType::Fist, WeaponType::Pistol];
            if self.player.has_shotgun {
                w.push(WeaponType::Shotgun);
            }
            if self.player.has_chaingun {
                w.push(WeaponType::Chaingun);
            }
            if self.player.has_rocket_launcher {
                w.push(WeaponType::RocketLauncher);
            }
            w
        };
        let current_idx = weapons
            .iter()
            .position(|w| *w == self.player.current_weapon)
            .unwrap_or(0);
        let next_idx = if forward {
            (current_idx + 1) % weapons.len()
        } else {
            (current_idx + weapons.len() - 1) % weapons.len()
        };
        self.player.current_weapon = weapons[next_idx];
    }

    /// Move player forward/backward and strafe, with wall collision.
    fn move_player(&mut self, forward: i32, strafe: i32) {
        let cos_a = fixmath::fp_cos(self.player.angle);
        let sin_a = fixmath::fp_sin(self.player.angle);

        // Forward component
        let dx = (cos_a * forward) / FP_SCALE + (-sin_a * strafe) / FP_SCALE;
        let dy = (sin_a * forward) / FP_SCALE + (cos_a * strafe) / FP_SCALE;

        let new_x = self.player.x + dx;
        let new_y = self.player.y + dy;

        // Try X and Y independently for wall sliding
        if !self.map.point_collides(new_x, self.player.y) {
            self.player.x = new_x;
        }
        if !self.map.point_collides(self.player.x, new_y) {
            self.player.y = new_y;
        }
    }

    /// Shoot — weapon-dependent behavior with damage randomization.
    fn player_shoot(&mut self) {
        if self.player.weapon_cooldown > 0 {
            return;
        }

        match self.player.current_weapon {
            WeaponType::Fist => {
                self.player.weapon_cooldown = 5;
                let damage = self.randomize_damage(10);
                let (px, py, pa) = (self.player.x, self.player.y, self.player.angle);
                self.hitscan_attack(px, py, pa, 1500, damage, 87);
            }
            WeaponType::Pistol => {
                if self.player.ammo <= 0 {
                    return;
                }
                self.player.ammo -= 1;
                self.player.weapon_cooldown = 3;
                let damage = self.randomize_damage(15);
                let (px, py, pa) = (self.player.x, self.player.y, self.player.angle);
                self.hitscan_attack(px, py, pa, 64 * FP_SCALE, damage, 87);
                self.alert_enemies_from_sound();
            }
            WeaponType::Shotgun => {
                if self.player.shells <= 0 {
                    return;
                }
                self.player.shells -= 1;
                self.player.weapon_cooldown = 7;
                // Shotgun fires 7 pellets in a spread (like Doom)
                let (px, py, pa) = (self.player.x, self.player.y, self.player.angle);
                for i in 0..7 {
                    let spread = (i as i32 - 3) * 30; // ±90 millirad spread
                    let angle = normalize_angle(pa + spread);
                    let damage = self.randomize_damage(8);
                    self.hitscan_attack(px, py, angle, 64 * FP_SCALE, damage, 50);
                }
                self.alert_enemies_from_sound();
            }
            WeaponType::Chaingun => {
                if self.player.ammo <= 0 {
                    return;
                }
                self.player.ammo -= 1;
                self.player.weapon_cooldown = 2; // faster than pistol
                let damage = self.randomize_damage(15);
                let (px, py, pa) = (self.player.x, self.player.y, self.player.angle);
                self.hitscan_attack(px, py, pa, 64 * FP_SCALE, damage, 100);
                self.alert_enemies_from_sound();
            }
            WeaponType::RocketLauncher => {
                if self.player.rockets <= 0 {
                    return;
                }
                self.player.rockets -= 1;
                self.player.weapon_cooldown = 8;
                let damage = self.randomize_damage(80);
                // Fire a rocket projectile (like Imp fireball but player-owned, higher damage)
                let cos_a = fixmath::fp_cos(self.player.angle);
                let sin_a = fixmath::fp_sin(self.player.angle);
                let speed = 400; // faster than imp fireball
                self.projectiles.push(Projectile {
                    x: self.player.x + cos_a * 500 / FP_SCALE,
                    y: self.player.y + sin_a * 500 / FP_SCALE,
                    vx: cos_a * speed / FP_SCALE,
                    vy: sin_a * speed / FP_SCALE,
                    damage,
                    source: ProjectileSource::Player,
                    alive: true,
                    sprite_id: 2, // rocket sprite
                });
                self.alert_enemies_from_sound();
            }
        }
    }

    /// Randomize damage Doom-style: base * (rng(0-255) % 8 + 1) / 8
    /// Gives range from base/8 to base.
    fn randomize_damage(&mut self, base: i32) -> i32 {
        let roll = self.rng.next() as i32;
        let multiplier = (roll % 8) + 1; // 1-8
        (base * multiplier) / 8
    }

    /// Generic hitscan attack — used by pistol, shotgun, and sergeant enemies.
    fn hitscan_attack(
        &mut self,
        from_x: i32,
        from_y: i32,
        angle: i32,
        max_range: i32,
        damage: i32,
        cone: i32,
    ) {
        let hit = self.map.cast_ray(from_x, from_y, angle);
        let wall_dist = hit.distance.min(max_range);

        let mut closest_enemy: Option<(usize, i32)> = None;

        for (i, enemy) in self.enemies.iter().enumerate() {
            if !enemy.is_alive() {
                continue;
            }

            let dx = enemy.x - from_x;
            let dy = enemy.y - from_y;
            let dist = fixmath::fp_dist(dx, dy);

            if dist > wall_dist || dist < 100 {
                continue;
            }

            let angle_to_enemy = fixmath::fp_atan2(dy, dx) as i32;
            let angle_diff = normalize_angle(angle_to_enemy - angle + PI) - PI;

            if angle_diff.abs() < cone {
                match closest_enemy {
                    Some((_, d)) if dist < d => {
                        closest_enemy = Some((i, dist));
                    }
                    None => {
                        closest_enemy = Some((i, dist));
                    }
                    _ => {}
                }
            }
        }

        if let Some((idx, _)) = closest_enemy {
            // Blood splat at enemy position
            let ex = self.enemies[idx].x;
            let ey = self.enemies[idx].y;
            self.effects.push(VisualEffect {
                x: ex,
                y: ey,
                effect_type: EffectType::BloodSplat,
                timer: 4,
            });
            self.damage_enemy(idx, damage);
        } else {
            // Bullet puff at wall hit point
            let puff_dist = (wall_dist - 50).max(0);
            let puff_x = from_x + fixmath::fp_cos(angle) * puff_dist / FP_SCALE;
            let puff_y = from_y + fixmath::fp_sin(angle) * puff_dist / FP_SCALE;
            self.effects.push(VisualEffect {
                x: puff_x,
                y: puff_y,
                effect_type: EffectType::BulletPuff,
                timer: 3,
            });
        }
    }

    /// Apply damage to an enemy, handling pain chance and death.
    fn damage_enemy(&mut self, idx: usize, damage: i32) {
        self.enemies[idx].health -= damage;
        if self.enemies[idx].health <= 0 {
            self.enemies[idx].health = 0;
            self.enemies[idx].ai_state = EnemyAiState::Dead;
            self.player.kills += 1;
            self.events.push(GameEvent::EnemyKilled {
                enemy_type: self.enemies[idx].enemy_type,
                x: self.enemies[idx].x,
                y: self.enemies[idx].y,
            });
        } else {
            // Pain chance roll — Chocolate Doom style
            let pain_chance = self.enemies[idx].pain_chance();
            if self.rng.check(pain_chance) {
                self.enemies[idx].ai_state = EnemyAiState::Pain;
            } else {
                // Hit but no flinch — start chasing if idle
                if self.enemies[idx].ai_state == EnemyAiState::Idle {
                    self.enemies[idx].ai_state =
                        EnemyAiState::Alerted(self.enemies[idx].reaction_ticks());
                }
            }
        }
    }

    /// Alert nearby enemies when the player fires a gun.
    /// Sound propagates through connected open areas (simplified: radius-based
    /// with LOS check through doors/open spaces).
    fn alert_enemies_from_sound(&mut self) {
        let alert_range = 15 * FP_SCALE; // gunfire heard within 15 tiles
        let px = self.player.x;
        let py = self.player.y;

        for enemy in self.enemies.iter_mut() {
            if !enemy.is_alive() || enemy.ai_state != EnemyAiState::Idle {
                continue;
            }

            let dx = px - enemy.x;
            let dy = py - enemy.y;
            let dist = fixmath::fp_dist(dx, dy);

            if dist < alert_range {
                // Sound-based alert — enemy knows general direction but has reaction delay
                enemy.ai_state = EnemyAiState::Alerted(enemy.reaction_ticks());
                enemy.last_known_px = px;
                enemy.last_known_py = py;
            }
        }
    }

    /// Use key — open doors in front of player, respecting key locks.
    fn player_use(&mut self) {
        let check_dist = FP_SCALE; // check 1 tile ahead
        let check_x = self.player.x + fixmath::fp_cos(self.player.angle) * check_dist / FP_SCALE;
        let check_y = self.player.y + fixmath::fp_sin(self.player.angle) * check_dist / FP_SCALE;

        let gx = (check_x / FP_SCALE) as u32;
        let gy = (check_y / FP_SCALE) as u32;

        let tile = self.map.get_tile(gx, gy);
        let idx = (gy * self.map.width + gx) as usize;

        match tile {
            TileType::Door(DoorState::Closed) => {
                self.map.tiles[idx] = TileType::Door(DoorState::Opening(0));
                self.events.push(GameEvent::DoorOpened { x: gx, y: gy });
            }
            TileType::Door(DoorState::LockedRed) => {
                if self.player.has_red_key {
                    self.map.tiles[idx] = TileType::Door(DoorState::Opening(0));
                    self.events.push(GameEvent::DoorOpened { x: gx, y: gy });
                }
            }
            TileType::Door(DoorState::LockedBlue) => {
                if self.player.has_blue_key {
                    self.map.tiles[idx] = TileType::Door(DoorState::Opening(0));
                    self.events.push(GameEvent::DoorOpened { x: gx, y: gy });
                }
            }
            _ => {}
        }
    }

    /// Update door animations — opening, auto-close timer, closing.
    fn update_doors(&mut self) {
        let player_gx = (self.player.x / FP_SCALE) as u32;
        let player_gy = (self.player.y / FP_SCALE) as u32;
        let w = self.map.width;

        for (idx, tile) in self.map.tiles.iter_mut().enumerate() {
            match tile {
                TileType::Door(DoorState::Opening(progress)) => {
                    if *progress >= 100 {
                        *tile = TileType::Door(DoorState::OpenWait(DOOR_WAIT_TICKS));
                    } else {
                        *progress += 20;
                    }
                }
                TileType::Door(DoorState::OpenWait(wait)) => {
                    if *wait == 0 {
                        *tile = TileType::Door(DoorState::Closing(100));
                    } else {
                        *wait -= 1;
                    }
                }
                TileType::Door(DoorState::Closing(progress)) => {
                    let door_gx = (idx as u32) % w;
                    let door_gy = (idx as u32) / w;
                    let player_in_door = door_gx == player_gx && door_gy == player_gy;

                    if *progress <= 20 {
                        if player_in_door {
                            // Re-open — don't trap the player
                            *tile = TileType::Door(DoorState::Opening(0));
                        } else {
                            *tile = TileType::Door(DoorState::Closed);
                        }
                    } else {
                        if player_in_door {
                            // Player walked in during closing — re-open
                            *tile = TileType::Door(DoorState::Opening(*progress));
                        } else {
                            *progress -= 20;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Update all projectiles — move them, check wall/player collisions.
    fn update_projectiles(&mut self) {
        let mut damage_to_player: Vec<(i32, EnemyType)> = Vec::new();

        for proj in self.projectiles.iter_mut() {
            if !proj.alive {
                continue;
            }

            // Move projectile
            proj.x += proj.vx;
            proj.y += proj.vy;

            // Wall collision
            let gx = (proj.x / FP_SCALE) as u32;
            let gy = (proj.y / FP_SCALE) as u32;
            if self.map.is_solid(gx, gy) {
                proj.alive = false;
                continue;
            }

            // Player projectile hitting enemies (rockets)
            if matches!(proj.source, ProjectileSource::Player) {
                let mut hit_enemy = false;
                for enemy in self.enemies.iter_mut() {
                    if !enemy.is_alive() { continue; }
                    let dx = (proj.x - enemy.x) as i64;
                    let dy = (proj.y - enemy.y) as i64;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < 400 * 400 {
                        // Direct hit
                        enemy.health -= proj.damage;
                        if enemy.health <= 0 {
                            enemy.ai_state = EnemyAiState::Dead;
                            self.player.kills += 1;
                        } else {
                            let pain_chance = match enemy.enemy_type {
                                EnemyType::Imp => 200,
                                EnemyType::Demon => 180,
                                EnemyType::Sergeant => 170,
                            };
                            if (self.rng.next() as i32) < pain_chance {
                                enemy.ai_state = EnemyAiState::Pain;
                            }
                        }
                        // Blood effect
                        self.effects.push(VisualEffect {
                            x: enemy.x, y: enemy.y,
                            effect_type: EffectType::BloodSplat, timer: 4,
                        });
                        hit_enemy = true;
                        break;
                    }
                }
                if hit_enemy {
                    // Spawn puff at impact
                    self.effects.push(VisualEffect {
                        x: proj.x, y: proj.y,
                        effect_type: EffectType::BulletPuff, timer: 4,
                    });
                    proj.alive = false;
                    continue;
                }
            }

            // Enemy projectile hitting player
            if let ProjectileSource::Enemy(etype) = proj.source {
                let dx = (proj.x - self.player.x) as i64;
                let dy = (proj.y - self.player.y) as i64;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < 300 * 300 {
                    // hit radius 0.3 tiles
                    proj.alive = false;
                    damage_to_player.push((proj.damage, etype));
                }
            }

            // Out of bounds check
            if proj.x < 0 || proj.y < 0 || gx >= self.map.width || gy >= self.map.height {
                proj.alive = false;
            }
        }

        // Apply projectile damage to player
        for (amount, source) in damage_to_player {
            let randomized = {
                let roll = self.rng.next() as i32;
                let mult = (roll % 8) + 1;
                (amount * mult) / 8
            };
            self.apply_damage_to_player(randomized, source);
        }

        // Clean up dead projectiles
        self.projectiles.retain(|p| p.alive);
    }

    /// Run AI for all enemies — Chocolate Doom-style behavior.
    fn update_enemies(&mut self) {
        let px = self.player.x;
        let py = self.player.y;
        let player_alive = self.player.alive;

        let mut damage_to_player: Vec<(i32, EnemyType)> = Vec::new();
        let mut new_projectiles: Vec<Projectile> = Vec::new();

        for enemy in self.enemies.iter_mut() {
            if !enemy.is_alive() {
                continue;
            }

            if enemy.attack_cooldown > 0 {
                enemy.attack_cooldown -= 1;
            }

            // Pain state — lasts one tick then resume chasing
            if enemy.ai_state == EnemyAiState::Pain {
                enemy.ai_state = EnemyAiState::Chasing;
                continue; // skip this tick (pain flinch)
            }

            // Alerted state — count down reaction time
            if let EnemyAiState::Alerted(ticks) = enemy.ai_state {
                if ticks <= 1 {
                    enemy.ai_state = EnemyAiState::Chasing;
                } else {
                    enemy.ai_state = EnemyAiState::Alerted(ticks - 1);
                }
                continue; // still reacting
            }

            if !player_alive {
                enemy.ai_state = EnemyAiState::Idle;
                continue;
            }

            // Distance to player
            let dx = px - enemy.x;
            let dy = py - enemy.y;
            let dist = fixmath::fp_dist(dx, dy);

            // Line-of-sight check
            let angle_to_player = fixmath::fp_atan2(dy, dx) as i32;
            let los_hit = self.map.cast_ray(enemy.x, enemy.y, angle_to_player);
            let has_los = los_hit.distance > dist;

            // Detection range: 10 tiles
            let detection_range = 10 * FP_SCALE;

            // Idle → Alerted (with reaction delay)
            if enemy.ai_state == EnemyAiState::Idle && dist < detection_range && has_los {
                enemy.ai_state = EnemyAiState::Alerted(enemy.reaction_ticks());
                enemy.last_known_px = px;
                enemy.last_known_py = py;
            }

            match enemy.ai_state {
                EnemyAiState::Chasing => {
                    // Update last known position when we have LOS
                    if has_los {
                        enemy.last_known_px = px;
                        enemy.last_known_py = py;
                    }

                    // Choose movement target — player if LOS, else last known position
                    let (target_x, target_y) = if has_los {
                        (px, py)
                    } else {
                        (enemy.last_known_px, enemy.last_known_py)
                    };

                    let target_dx = target_x - enemy.x;
                    let target_dy = target_y - enemy.y;
                    let target_dist = fixmath::fp_dist(target_dx, target_dy);

                    // If we reached last known position without LOS, go idle
                    if !has_los && target_dist < 500 {
                        enemy.ai_state = EnemyAiState::Idle;
                        continue;
                    }

                    // Move toward target with zigzag behavior
                    if target_dist > 0 {
                        let speed = enemy.speed();
                        let base_angle = fixmath::fp_atan2(target_dy, target_dx) as i32;

                        // Zigzag: periodically add random strafe angle
                        if enemy.strafe_timer == 0 {
                            // Random strafe offset: -500 to +500 millirad (~±30°)
                            let r = enemy.x.wrapping_mul(31).wrapping_add(enemy.y.wrapping_mul(17));
                            enemy.move_dir = (r % 1000) - 500;
                            enemy.strafe_timer = 8 + ((r.unsigned_abs() % 8) as u8);
                        } else {
                            enemy.strafe_timer -= 1;
                        }

                        let move_angle = base_angle + enemy.move_dir;
                        let move_x = fixmath::fp_cos(move_angle) * speed / FP_SCALE;
                        let move_y = fixmath::fp_sin(move_angle) * speed / FP_SCALE;

                        let new_x = enemy.x + move_x;
                        let new_y = enemy.y + move_y;

                        // Collision check with radius (prevents sprite clipping into walls)
                        if !self.map.point_collides(new_x, new_y) {
                            enemy.x = new_x;
                            enemy.y = new_y;
                        } else if !self.map.point_collides(new_x, enemy.y) {
                            // Wall slide — try X only
                            enemy.x = new_x;
                        } else if !self.map.point_collides(enemy.x, new_y) {
                            // Wall slide — try Y only
                            enemy.y = new_y;
                        } else {
                            // Fully blocked, reset strafe
                            enemy.strafe_timer = 0;
                        }
                    }

                    // Attack if in range, cooldown ready, AND line of sight
                    if dist < enemy.attack_range() && enemy.attack_cooldown == 0 && has_los {
                        enemy.ai_state = EnemyAiState::Attacking;
                        enemy.attack_cooldown = 10 + (enemy.x.unsigned_abs() % 5) as u8; // slight randomization

                        if enemy.fires_projectile() {
                            // Imp fireball — spawn projectile entity
                            let proj_speed = enemy.projectile_speed();
                            let proj_angle = fixmath::fp_atan2(dy, dx);
                            let vx = fixmath::fp_cos(proj_angle) * proj_speed / FP_SCALE;
                            let vy = fixmath::fp_sin(proj_angle) * proj_speed / FP_SCALE;
                            new_projectiles.push(Projectile {
                                x: enemy.x,
                                y: enemy.y,
                                vx,
                                vy,
                                damage: enemy.damage(),
                                source: ProjectileSource::Enemy(enemy.enemy_type),
                                alive: true,
                                sprite_id: 0, // fireball sprite
                            });
                        } else if matches!(enemy.enemy_type, EnemyType::Demon) {
                            // Melee attack — direct damage
                            damage_to_player.push((enemy.damage(), enemy.enemy_type));
                        } else {
                            // Sergeant — hitscan attack (will be resolved after loop)
                            damage_to_player.push((enemy.damage(), enemy.enemy_type));
                        }
                    }
                }
                EnemyAiState::Attacking => {
                    // Return to chasing after attack frame
                    enemy.ai_state = EnemyAiState::Chasing;
                }
                _ => {}
            }
        }

        // Spawn projectiles
        self.projectiles.extend(new_projectiles);

        // Apply melee/hitscan damage to player (randomized)
        for (amount, source) in damage_to_player {
            let randomized = {
                let roll = self.rng.next() as i32;
                let mult = (roll % 8) + 1;
                (amount * mult) / 8
            };
            self.apply_damage_to_player(randomized, source);
        }
    }

    pub fn apply_damage_to_player(&mut self, amount: i32, source: EnemyType) {
        // Armor absorbs 50% of damage
        let armor_absorb = if self.player.armor > 0 {
            let absorb = amount / 2;
            let actual = absorb.min(self.player.armor);
            self.player.armor -= actual;
            actual
        } else {
            0
        };

        let health_damage = amount - armor_absorb;
        self.player.health -= health_damage;
        self.player.last_damage_tick = self.tick;

        self.events.push(GameEvent::PlayerDamaged {
            amount: health_damage,
            source,
        });

        if self.player.health <= 0 {
            self.player.health = 0;
            self.player.alive = false;
            self.game_over = true;
            self.events.push(GameEvent::PlayerDied {
                kills: self.player.kills,
            });
        }
    }

    /// Check if player is standing on any items.
    fn check_item_pickups(&mut self) {
        let pickup_range_sq: i64 = 400 * 400; // 0.4 tiles

        for item in self.items.iter_mut() {
            if item.picked_up {
                continue;
            }

            let dx = (self.player.x - item.x) as i64;
            let dy = (self.player.y - item.y) as i64;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq > pickup_range_sq {
                continue;
            }

            let picked = match item.item_type {
                ItemType::HealthPack if self.player.health < 100 => {
                    self.player.health = (self.player.health + 25).min(100);
                    true
                }
                ItemType::Medikit if self.player.health < 100 => {
                    self.player.health = (self.player.health + 50).min(100);
                    true
                }
                ItemType::AmmoClip => {
                    self.player.ammo += 10;
                    true
                }
                ItemType::AmmoBox => {
                    self.player.ammo += 25;
                    true
                }
                ItemType::ShellBox => {
                    self.player.shells += 4;
                    true
                }
                ItemType::Shotgun => {
                    self.player.has_shotgun = true;
                    self.player.shells += 8;
                    self.player.current_weapon = WeaponType::Shotgun;
                    true
                }
                ItemType::Chaingun => {
                    self.player.has_chaingun = true;
                    self.player.ammo += 20;
                    self.player.current_weapon = WeaponType::Chaingun;
                    true
                }
                ItemType::RocketLauncher => {
                    self.player.has_rocket_launcher = true;
                    self.player.rockets += 2;
                    self.player.current_weapon = WeaponType::RocketLauncher;
                    true
                }
                ItemType::RocketBox => {
                    self.player.rockets += 5;
                    true
                }
                ItemType::Armor if self.player.armor < 100 => {
                    self.player.armor = (self.player.armor + 50).min(100);
                    true
                }
                ItemType::KeyRed => {
                    self.player.has_red_key = true;
                    true
                }
                ItemType::KeyBlue => {
                    self.player.has_blue_key = true;
                    true
                }
                _ => false,
            };

            if picked {
                item.picked_up = true;
                self.events.push(GameEvent::ItemPickedUp {
                    item_type: item.item_type,
                });
            }
        }
    }

    /// Check if player reached the exit.
    fn check_exit(&mut self) {
        let gx = (self.player.x / FP_SCALE) as u32;
        let gy = (self.player.y / FP_SCALE) as u32;

        if let TileType::Exit = self.map.get_tile(gx, gy) {
            self.level_complete = true;
            self.events.push(GameEvent::LevelComplete);
        }
    }

    /// Count alive enemies.
    pub fn alive_enemy_count(&self) -> usize {
        self.enemies.iter().filter(|e| e.is_alive()).count()
    }

    /// Total enemies (including dead).
    pub fn total_enemy_count(&self) -> usize {
        self.enemies.len()
    }
}
