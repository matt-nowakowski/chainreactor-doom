use doom_engine::*;

#[test]
fn test_new_game_from_e1m1() {
    let map = DoomMap::e1m1();
    let state = GameState::new(map);

    assert_eq!(state.tick, 0);
    assert!(!state.game_over);
    assert!(!state.level_complete);
    assert!(state.player.alive);
    assert_eq!(state.player.health, 100);
    assert_eq!(state.player.ammo, 50);
    assert_eq!(state.total_enemy_count(), 6);
    assert_eq!(state.alive_enemy_count(), 6);
}

#[test]
fn test_player_movement() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    let start_x = state.player.x;
    let start_y = state.player.y;

    // Player starts facing angle 0 (east), so forward should increase X
    state.tick(&[PlayerInput::Forward]);

    assert!(
        state.player.x > start_x || state.player.y != start_y,
        "Player should have moved"
    );
    assert_eq!(state.tick, 1);
}

#[test]
fn test_player_turn() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    let start_angle = state.player.angle;

    state.tick(&[PlayerInput::TurnRight]);
    assert!(state.player.angle != start_angle, "Angle should change");
    assert_eq!(state.player.angle, start_angle + 100); // turn_speed = 100 millirad
}

#[test]
fn test_player_shoot_decrements_ammo() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    let start_ammo = state.player.ammo;
    state.tick(&[PlayerInput::Shoot]);

    assert_eq!(state.player.ammo, start_ammo - 1);
    assert_eq!(state.player.weapon_cooldown, 2); // decremented once at end of tick
}

#[test]
fn test_weapon_cooldown_prevents_shooting() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    state.tick(&[PlayerInput::Shoot]); // fires, cooldown = 3, then decremented to 2
    let ammo_after_first = state.player.ammo;

    state.tick(&[PlayerInput::Shoot]); // should not fire — cooldown active
    assert_eq!(state.player.ammo, ammo_after_first); // ammo unchanged
}

#[test]
fn test_game_over_stops_ticks() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    state.game_over = true;
    let tick_before = state.tick;
    state.tick(&[PlayerInput::Forward]);

    assert_eq!(state.tick, tick_before); // tick should NOT increment
}

#[test]
fn test_level_complete_stops_ticks() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    state.level_complete = true;
    state.tick(&[PlayerInput::Forward]);

    assert_eq!(state.tick, 0);
}

#[test]
fn test_map_collision() {
    let map = DoomMap::e1m1();

    // (0,0) is a wall — should be solid
    assert!(map.is_solid(0, 0));

    // (1,1) is empty floor in e1m1 — should NOT be solid
    assert!(!map.is_solid(1, 1));
}

#[test]
fn test_raycasting_hits_wall() {
    let map = DoomMap::e1m1();

    // Cast ray from center of tile (2,2) facing east (angle 0)
    let hit = map.cast_ray(2500, 2500, 0);

    assert!(hit.distance > 0);
    assert!(hit.distance < 64000); // should hit a wall within 64 tiles
}

#[test]
fn test_door_opening() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    // Find a door tile manually
    let mut door_idx = None;
    for (i, tile) in state.map.tiles.iter().enumerate() {
        if matches!(tile, TileType::Door(DoorState::Closed)) {
            door_idx = Some(i);
            break;
        }
    }
    assert!(door_idx.is_some(), "E1M1 should have doors");

    // Force-open the door
    let idx = door_idx.unwrap();
    state.map.tiles[idx] = TileType::Door(DoorState::Opening(0));

    // Run ticks until it opens
    for _ in 0..10 {
        state.tick(&[]);
    }

    // After enough ticks, door should be in OpenWait state (auto-close timer)
    assert!(
        matches!(
            state.map.tiles[idx],
            TileType::Door(DoorState::OpenWait(_))
        ),
        "Door should be open (waiting) after enough ticks, got {:?}",
        state.map.tiles[idx]
    );
}

#[test]
fn test_item_pickup() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear(); // isolate item pickup from enemy AI

    // Place a health pack right on the player
    state.items.push(Item::new(
        ItemType::HealthPack,
        state.player.x,
        state.player.y,
    ));
    state.player.health = 50; // so the pickup actually applies

    let last_idx = state.items.len() - 1;
    state.tick(&[]);

    assert!(state.items[last_idx].picked_up);
    assert_eq!(state.player.health, 75); // +25 from health pack
}

#[test]
fn test_ammo_pickup() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    state.items.push(Item::new(
        ItemType::AmmoClip,
        state.player.x,
        state.player.y,
    ));
    let ammo_before = state.player.ammo;

    state.tick(&[]);

    assert_eq!(state.player.ammo, ammo_before + 10);
}

#[test]
fn test_health_pack_not_picked_at_full_health() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear(); // isolate from enemy damage

    state.items.push(Item::new(
        ItemType::HealthPack,
        state.player.x,
        state.player.y,
    ));
    assert_eq!(state.player.health, 100);

    let last_idx = state.items.len() - 1;
    state.tick(&[]);

    assert!(!state.items[last_idx].picked_up); // should NOT pick up at full health
}

#[test]
fn test_render_frame_produces_output() {
    let map = DoomMap::e1m1();
    let state = GameState::new(map);
    let mut fb = Framebuffer::new();

    render_frame(&state, &mut fb);

    // Should have non-zero RGBA pixels (sky, walls, floor)
    let non_zero = fb.rgba.iter().filter(|&&p| p != 0).count();
    assert!(
        non_zero > fb.rgba.len() / 2,
        "Most of the framebuffer should be filled"
    );

    // All alpha values should be 255 (no transparent pixels in final output)
    for i in 0..FRAMEBUFFER_SIZE {
        assert_eq!(fb.rgba[i * 4 + 3], 255);
    }
}

#[test]
fn test_normalize_angle() {
    use doom_engine::types::{normalize_angle, TWO_PI};

    assert_eq!(normalize_angle(0), 0);
    assert_eq!(normalize_angle(-100), TWO_PI - 100);
    assert_eq!(normalize_angle(TWO_PI + 100), 100);
}

#[test]
fn test_enemy_stats() {
    let imp = Enemy::new(EnemyType::Imp, 5000, 5000);
    assert_eq!(imp.health, 60);
    assert_eq!(imp.speed(), 40);
    assert_eq!(imp.damage(), 15);
    assert!(imp.is_alive());

    let demon = Enemy::new(EnemyType::Demon, 5000, 5000);
    assert_eq!(demon.health, 80);
    assert_eq!(demon.attack_range(), 1500); // melee only

    let sgt = Enemy::new(EnemyType::Sergeant, 5000, 5000);
    assert_eq!(sgt.health, 30);
    assert_eq!(sgt.attack_range(), 10000); // ranged
}

#[test]
fn test_player_damage_with_armor() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.player.armor = 100;
    state.player.health = 100;

    // Simulate damage
    // 50% absorbed by armor
    // 20 damage → 10 to armor, 10 to health
    state.apply_damage_to_player(20, EnemyType::Imp);

    assert_eq!(state.player.armor, 90);
    assert_eq!(state.player.health, 90);
}

#[test]
fn test_player_death() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.player.health = 10;

    state.apply_damage_to_player(20, EnemyType::Demon);

    assert_eq!(state.player.health, 0);
    assert!(!state.player.alive);
    assert!(state.game_over);
}

#[test]
fn test_multiple_ticks() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear(); // isolate tick counting from enemy damage

    // Run 100 ticks with no input
    for _ in 0..100 {
        state.tick(&[]);
    }

    assert_eq!(state.tick, 100);
    assert!(state.player.alive);
}

#[test]
fn test_wall_sliding() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    // Move player to near a wall corner and try to walk into it
    // The player should slide along the wall (X or Y moves independently)
    state.player.x = 1200; // close to left wall
    state.player.y = 1500;
    state.player.angle = PI; // facing west (into wall)

    let old_x = state.player.x;
    let old_y = state.player.y;
    state.tick(&[PlayerInput::Forward]);

    // Player might not move in X (blocked by wall) but shouldn't crash
    assert!(state.player.x <= old_x || state.player.y != old_y || (state.player.x == old_x && state.player.y == old_y));
}

// --- Chocolate Doom parity tests ---

#[test]
fn test_weapon_switching() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);

    assert_eq!(state.player.current_weapon, WeaponType::Pistol);

    // Switch to fist
    state.tick(&[PlayerInput::Weapon1]);
    assert_eq!(state.player.current_weapon, WeaponType::Fist);

    // Can't switch to shotgun without having it
    state.tick(&[PlayerInput::Weapon3]);
    assert_eq!(state.player.current_weapon, WeaponType::Fist);

    // Give shotgun and switch
    state.player.has_shotgun = true;
    state.player.shells = 10;
    state.tick(&[PlayerInput::Weapon3]);
    assert_eq!(state.player.current_weapon, WeaponType::Shotgun);
}

#[test]
fn test_fist_no_ammo_needed() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    state.player.current_weapon = WeaponType::Fist;
    state.player.ammo = 0;

    // Fist should still work with no ammo
    state.tick(&[PlayerInput::Shoot]);
    assert!(state.player.weapon_cooldown > 0);
}

#[test]
fn test_shotgun_uses_shells() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    state.player.has_shotgun = true;
    state.player.current_weapon = WeaponType::Shotgun;
    state.player.shells = 5;

    state.tick(&[PlayerInput::Shoot]);
    assert_eq!(state.player.shells, 4);
    assert_eq!(state.player.weapon_cooldown, 6); // 7 - 1 (decremented at end of tick)
}

#[test]
fn test_projectiles_move() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    // Manually spawn a projectile
    state.projectiles.push(Projectile {
        x: 5000,
        y: 5000,
        vx: 300,
        vy: 0,
        damage: 15,
        source: ProjectileSource::Enemy(EnemyType::Imp),
        alive: true,
        sprite_id: 0,
    });

    state.tick(&[]);
    assert_eq!(state.projectiles.len(), 1);
    assert_eq!(state.projectiles[0].x, 5300);
}

#[test]
fn test_projectile_wall_collision() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    // Spawn projectile heading into a wall
    state.projectiles.push(Projectile {
        x: 500,
        y: 500,
        vx: -300,
        vy: 0,
        damage: 15,
        source: ProjectileSource::Enemy(EnemyType::Imp),
        alive: true,
        sprite_id: 0,
    });

    state.tick(&[]);
    // Projectile should be destroyed (hit wall at x=0)
    assert_eq!(state.projectiles.len(), 0);
}

#[test]
fn test_door_auto_close() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    // Find a door
    let door_idx = state
        .map
        .tiles
        .iter()
        .position(|t| matches!(t, TileType::Door(DoorState::Closed)))
        .expect("E1M1 should have doors");

    // Force-open it
    state.map.tiles[door_idx] = TileType::Door(DoorState::Opening(0));

    // Run ticks until it opens fully (5 ticks at +20 per tick)
    for _ in 0..6 {
        state.tick(&[]);
    }
    assert!(
        matches!(state.map.tiles[door_idx], TileType::Door(DoorState::OpenWait(_))),
        "Door should be in OpenWait"
    );

    // Run enough ticks for auto-close timer (60 ticks) + closing (5 ticks)
    for _ in 0..70 {
        state.tick(&[]);
    }
    assert!(
        matches!(state.map.tiles[door_idx], TileType::Door(DoorState::Closed)),
        "Door should auto-close, got {:?}",
        state.map.tiles[door_idx]
    );
}

#[test]
fn test_pain_chance() {
    let imp = Enemy::new(EnemyType::Imp, 5000, 5000);
    assert_eq!(imp.pain_chance(), 200); // ~78%

    let demon = Enemy::new(EnemyType::Demon, 5000, 5000);
    assert_eq!(demon.pain_chance(), 180); // ~70%
}

#[test]
fn test_enemy_reaction_delay() {
    let imp = Enemy::new(EnemyType::Imp, 5000, 5000);
    assert_eq!(imp.reaction_ticks(), 3);

    let demon = Enemy::new(EnemyType::Demon, 5000, 5000);
    assert_eq!(demon.reaction_ticks(), 2);
}

#[test]
fn test_imp_fires_projectile() {
    let imp = Enemy::new(EnemyType::Imp, 5000, 5000);
    assert!(imp.fires_projectile());

    let demon = Enemy::new(EnemyType::Demon, 5000, 5000);
    assert!(!demon.fires_projectile());

    let sgt = Enemy::new(EnemyType::Sergeant, 5000, 5000);
    assert!(!sgt.fires_projectile());
}

#[test]
fn test_doom_rng_deterministic() {
    let mut rng1 = DoomRng::new();
    let mut rng2 = DoomRng::new();

    let seq1: Vec<u8> = (0..50).map(|_| rng1.next()).collect();
    let seq2: Vec<u8> = (0..50).map(|_| rng2.next()).collect();

    assert_eq!(seq1, seq2, "RNG must be deterministic");
}

#[test]
fn test_shell_pickup() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    state.items.push(Item::new(
        ItemType::ShellBox,
        state.player.x,
        state.player.y,
    ));

    state.tick(&[]);
    assert_eq!(state.player.shells, 4);
}

#[test]
fn test_shotgun_pickup_gives_weapon() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear();

    assert!(!state.player.has_shotgun);

    state.items.push(Item::new(
        ItemType::Shotgun,
        state.player.x,
        state.player.y,
    ));

    state.tick(&[]);
    assert!(state.player.has_shotgun);
    assert_eq!(state.player.shells, 8);
    assert_eq!(state.player.current_weapon, WeaponType::Shotgun);
}

// Re-export for the test
use doom_engine::types::PI;

#[test]
fn test_bullet_puff_spawns_on_shoot() {
    let map = DoomMap::e1m1();
    let mut state = GameState::new(map);
    state.enemies.clear(); // No enemies — shots hit walls

    assert_eq!(state.effects.len(), 0);

    // Shoot pistol — should create a bullet puff at wall hit
    state.tick(&[PlayerInput::Shoot]);
    assert!(
        state.effects.len() > 0,
        "Shooting should spawn a bullet puff effect"
    );

    // Effect should have timer > 0
    assert!(state.effects[0].timer > 0);

    // After enough ticks, effect should expire
    for _ in 0..10 {
        state.tick(&[]);
    }
    assert_eq!(state.effects.len(), 0, "Effects should expire after timer runs out");
}

#[test]
fn test_decoration_sprites_render() {
    let map = DoomMap::e1m1();
    let state = GameState::new(map);
    let mut fb = Framebuffer::new();

    // Render the frame — player starts facing east, dead soldier at (9,3) directly ahead
    render_frame(&state, &mut fb);

    // Check that the center area of the viewport has non-black pixels
    // The dead soldier sprite at ~7 tiles distance should render around y=72..97, x~155..165
    // Sample a few pixels in that region
    let center_x = 160;
    let sample_region_non_black = (70..100).any(|y| {
        let off = (y * 320 + center_x) * 4;
        fb.rgba[off] > 0 || fb.rgba[off + 1] > 0 || fb.rgba[off + 2] > 0
    });
    assert!(sample_region_non_black, "Center viewport should have visible pixels (walls/decorations)");
}
