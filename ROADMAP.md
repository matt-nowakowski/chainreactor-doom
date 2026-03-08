# Chainreactor DOOM — Parity Roadmap

Target: full Chocolate Doom (vanilla DOOM engine) parity, running deterministically on-chain.

## Legend
- **Done** — implemented and working
- **In Progress** — currently being built
- **Planned** — on the roadmap, not started

---

## Rendering Engine

| Feature | Status | Notes |
|---|---|---|
| DDA raycasting (320×200) | Done | Standard algorithm, 60° FOV |
| Textured walls (16 textures) | Done | UV mapping, side shading |
| Textured floors/ceilings | Done | 64×64 flats, per-sector |
| Sprite rendering (8-dir, depth sorted) | Done | ~100 Freedoom sprites |
| Per-sector lighting (flicker/pulse/strobe) | Done | 4 light effect types |
| Tall sprite scaling (torches/pillars) | Done | Per-sprite scale factor |
| Bullet puffs / blood splats | Done | VisualEffect system |
| Status bar (STBAR + digit sprites) | Done | Ammo, health, armor, keys |
| Status bar face (DOOM guy) | In Progress | Reacts to damage/health/kills |
| Sky texture | In Progress | Outdoor sectors render sky instead of ceiling |
| Automap overlay | In Progress | Tab-toggle wireframe map |
| Height-variable sectors (layered floors) | In Progress | Raised/lowered floor sections with step-up |
| Transparent sprites (partial alpha) | Planned | Spectres, invisibility effect |
| Light diminishing (distance fog) | Planned | Vanilla DOOM's distance-based light falloff |
| Colormap / invulnerability palette | Planned | Full-screen palette shifts |
| Sky scrolling | Planned | Parallax sky movement with player angle |
| Sprite clipping to floor/ceiling | Planned | Tall sprites clip at sector boundaries |
| Fuzz effect (Spectre rendering) | Planned | Column-based pixel scramble |

## Weapons

| Weapon | Status | Notes |
|---|---|---|
| Fist | Done | 10 dmg, melee range |
| Pistol | Done | 15 dmg, hitscan |
| Shotgun | Done | 7×8 dmg, spread |
| Chainsaw | Planned | Continuous melee, auto-aim lock |
| Chaingun | In Progress | Rapid-fire hitscan, 2× pistol rate |
| Rocket Launcher | In Progress | Projectile + splash damage radius |
| Plasma Rifle | Planned | Fast projectile, blue sprite |
| BFG 9000 | Planned | 40 tracers + 100-800 direct hit |
| Super Shotgun | Planned | 20 pellets, 2-shell cost (DOOM II) |

## Enemies

| Enemy | Status | Notes |
|---|---|---|
| Zombieman (Sergeant) | Done | Hitscan, 30 HP |
| Imp | Done | Fireball projectile, 60 HP |
| Demon (Pinky) | Done | Melee, 150 HP |
| Cacodemon | Planned | Flying, fireball, 400 HP |
| Baron of Hell | Planned | Fireball, melee, 1000 HP |
| Lost Soul | Planned | Flying charge attack, 100 HP |
| Pain Elemental | Planned | Spawns Lost Souls |
| Revenant | Planned | Homing missile + melee, 300 HP |
| Mancubus | Planned | Dual fireball spread, 600 HP |
| Arachnotron | Planned | Rapid plasma, 500 HP |
| Arch-Vile | Planned | Resurrects enemies, fire attack |
| Cyberdemon | Planned | Boss — rockets, 4000 HP |
| Spider Mastermind | Planned | Boss — chaingun, 3000 HP |
| Spectre | Planned | Invisible Demon (fuzz effect) |
| Chaingunner | Planned | Hitscan burst, 70 HP |

## Level Geometry & Interactivity

| Feature | Status | Notes |
|---|---|---|
| Doors (keyed + auto-close) | Done | Full state machine |
| Variable floor/ceiling heights | In Progress | Step-up traversal, sector transitions |
| Elevators / Lifts | Planned | Moving floor sectors (trigger-activated) |
| Crushers | Planned | Moving ceilings that damage player |
| Switches / Buttons | Planned | Line-based triggers for remote actions |
| Teleporters | Planned | Sector-to-sector instant travel |
| Secret areas | Planned | Hidden walls that open on Use |
| Exploding barrels | Planned | Chain-reaction splash damage |
| Moving platforms | Planned | Perpetual movers (rising/falling) |
| Scrolling walls/floors | Planned | Animated texture movement |
| Line-of-sight triggers | Planned | Walk-over lines that activate events |
| Damaging floors (lava/nukage) | Planned | Periodic damage in certain sectors |

## Game Systems

| Feature | Status | Notes |
|---|---|---|
| Deterministic RNG (Doom table) | Done | Bit-identical 256-entry table |
| Fixed-point math (i32×1000) | Done | On-chain safe, no floats |
| Item pickups | Done | Health, ammo, armor, keys, weapons |
| Title screen | Done | Animated logo + press any key |
| Death/victory overlays | Done | Red/green tint + restart |
| Powerups (Invulnerability) | Planned | Timed invincibility + palette shift |
| Powerups (Berserk) | Planned | 10× fist damage + full health |
| Powerups (Invisibility) | Planned | Partial invisibility, enemies miss more |
| Powerups (Light Amp) | Planned | Full brightness for duration |
| Powerups (Radiation Suit) | Planned | Immunity to floor damage |
| Difficulty settings | Planned | Enemy count/damage/ammo scaling |
| Multiple levels / map progression | Planned | E1M1–E1M9 style episode structure |
| Intermission screen | Planned | Kill/item/secret percentages |
| Save / Load | Planned | Serialize GameState to storage |
| Demo recording / playback | Planned | Input sequence capture and replay |
| Sound effects | Planned | Web Audio API for browser build |
| Music (MIDI playback) | Planned | OPL2/OPL3 emulation or Web MIDI |

## On-Chain Specific

| Feature | Status | Notes |
|---|---|---|
| Deterministic tick execution | Done | Same inputs → same state, always |
| Compact state serialization | Done | Serde for full GameState |
| WASM compilation | Done | wasm-pack, runs in browser |
| Block-per-tick mapping | Done | Each game tick = one block |
| Substrate pallet integration | Planned | Game state in on-chain storage |
| Extrinsic-based input | Planned | Player inputs as signed transactions |
| State proof verification | Planned | Merkle proof of game state per block |
| Multi-player consensus | Planned | Multiple players, same deterministic world |
| Replay verification | Planned | Anyone can re-execute and verify outcomes |
| Gas metering | Planned | Bounded execution per tick for block weight |

---

## Priority Order (recommended implementation sequence)

1. **Chaingun + Rocket Launcher** — biggest gameplay variety boost
2. **Exploding barrels** — already have barrel props, just need damage logic
3. **Secret walls** — simple Use-key check on marked wall tiles
4. **Cacodemon + Baron** — flying enemy + tank enemy for difficulty curve
5. **Elevators / Lifts** — core DOOM level design mechanic
6. **Damaging floors** — nukage/lava sectors
7. **Switches** — remote door/lift triggers
8. **Teleporters** — sector links
9. **Plasma Rifle + BFG** — endgame weapons
10. **Additional enemies** — Revenant, Mancubus, Arch-Vile
11. **Sound system** — Web Audio API integration
12. **Multiple levels** — E1M1–M3 at minimum
13. **Substrate pallet** — move game loop on-chain for real
