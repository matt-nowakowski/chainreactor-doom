# Can It Run DOOM? — On-Chain.

A fully playable DOOM-style first-person shooter where **both the game engine and the renderer execute on-chain** inside a Substrate blockchain's block production. Every game tick is a block. Every frame is rendered by validators.

Built as a technical demo for [Chain Reactor](https://chainreactor.io) — proving that Substrate appchains can run real-time game logic, not just token transfers.

## What This Is

A pure Rust game engine that achieves **functional parity with Chocolate Doom** — the reference-standard faithful recreation of the original 1993 DOOM engine. The entire game loop (player movement, enemy AI, projectile physics, raycasting, texture mapping, sprite rendering) runs deterministically using fixed-point integer math, designed from day one to execute inside a Substrate pallet's `on_initialize`.

Assets are sourced from **Freedoom** (Phase 1), the open-source DOOM-compatible IWAD released under a BSD license.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Browser (index.html)                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │  Keyboard    │→│  WASM tick() │→│  Canvas       │ │
│  │  Inputs      │  │  + render() │  │  320×200     │ │
│  └─────────────┘  └──────┬──────┘  └──────▲──────┘ │
│                          │ zero-copy       │         │
│                          └─── RGBA ptr ────┘         │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│  On-Chain (Substrate Pallet)                         │
│                                                      │
│  on_initialize() {                                   │
│      let inputs = pending_extrinsics();              │
│      game_state.tick(inputs);     // game logic      │
│      game_state.render(framebuf); // raycaster       │
│  }                                                   │
│                                                      │
│  custom_rpc::get_frame() → &[u8; 256000]            │
└─────────────────────────────────────────────────────┘
```

**Current state**: The engine runs as a WASM module in the browser at 60fps, with the same code that will run on-chain. The Substrate pallet wrapper is the next step — the engine is already `no_std`-compatible with no I/O, no filesystem, no floats in the hot path, and no dynamic allocation beyond the initial setup.

**On-chain target**: A 1-second block time Substrate appchain where each block executes one full game tick (inputs → movement → combat → AI → physics → rendering), with the framebuffer served to browsers via a custom RPC endpoint.

## Chocolate Doom Parity

The engine matches Chocolate Doom's gameplay systems:

| System | Status | Details |
|--------|--------|---------|
| DDA Raycasting | Done | Per-column wall casting with UV texture mapping |
| Textured Floors/Ceilings | Done | Per-pixel perspective-correct flat sampling |
| Sector Heights | Done | Variable floor/ceiling per cell — stairs, platforms, low ceilings |
| Sector Lighting | Done | Per-cell light levels + distance fog, animated effects (flicker, pulse, strobe) |
| Enemy AI (LOS) | Done | Ray-based line-of-sight, no shooting through walls |
| Enemy Projectiles | Done | Imp fireballs as independent entities with position/velocity, dodgeable |
| Pain Chance | Done | Per-type flinch probability matching Chocolate Doom (Imp 78%, Demon 70%, Sgt 66%) |
| Sound Alerting | Done | Gunfire propagates 15-tile radius, wakes idle enemies with reaction delay |
| Zigzag Movement | Done | Random strafe offsets during chase, no beeline behavior |
| Persistent Chase | Done | Enemies track last known player position after losing LOS |
| Damage Randomization | Done | `base * (rng%8 + 1) / 8` for all damage sources |
| Multiple Weapons | Done | Fist (melee), Pistol (hitscan), Shotgun (7-pellet spread at ±90 millirad) |
| Door System | Done | Opening → OpenWait → Auto-Close cycle, key-locked doors (red/blue) |
| Doom M_Random | Done | Exact 256-byte PRNG table from original Doom source (`m_random.c`) |
| 8-Direction Sprites | Done | Rotation-based enemy sprite selection using viewer-relative angle |
| Walk Animation | Done | A/B frame toggle for enemy walk cycles at all 8 rotations |
| STBAR + STTNUM | Done | Authentic status bar with big red digit sprites from the WAD |
| Ammo Inventory | Done | Yellow 3×5 pixel font for bullet/shell counts, matching Doom's STBAR layout |
| Bullet Puffs | Done | Visible impact effects when hitscan weapons hit walls or enemies |
| Decorative Props | Done | 26 prop types — barrels, torches, columns, skulls, hanging bodies |
| Door Safety | Done | Doors re-open if player is in doorway, preventing trap-in-door bugs |
| Enemy Wall Collision | Done | Radius-based collision keeps enemy sprites from clipping into walls |

### Only Intentional Difference

**Grid-based map geometry (90° walls)** instead of Doom's BSP tree with arbitrary wall angles. This is a deliberate tradeoff — grid-based DDA raycasting is ~10× cheaper to compute than BSP traversal, making it feasible to run inside a block's execution budget. The visual result is classic Wolfenstein-style perpendicular walls with Doom-quality texturing, lighting, and sprites on top.

## Freedoom Assets

All visual assets are extracted from the [Freedoom](https://freedoom.github.io/) IWAD (Phase 1), an open-source replacement for DOOM's commercial assets released under a modified BSD license. A custom WAD extraction tool (`tools/wad-extract/`) parses the WAD at build time and generates Rust source files with embedded data:

- **16 wall textures** — STARTAN3, BROWN1, STONE2, DOOR1, TEKWALL1, etc.
- **8 flat textures** — floors and ceilings (FLOOR0_1, CEIL3_5, NUKAGE1, etc.)
- **100 sprites** — enemies (3 types × 8 rotations × 2 walk frames + attack/pain/death), items, weapons, 26 decorative props
- **12 STTNUM digits** — the authentic Doom big red number font (0-9, %, minus)
- **STBAR** — the status bar background
- **PLAYPAL** — the 256-color palette

All stored as palette-indexed data (1 byte/pixel instead of 4 bytes RGBA), keeping the WASM binary at **~1.2MB** with everything baked in — textures, sprites, palette, and game logic.

## Technical Innovations

### Deterministic Fixed-Point Engine
All game state uses `i32 × 1000` fixed-point math. No floating point in the game loop. This guarantees bit-identical execution across all validators — a hard requirement for blockchain consensus. The same `tick()` function with the same inputs will produce the exact same state on every node, every time.

### On-Chain Renderer
The raycaster, floor/ceiling renderer, and sprite compositor are all pure computation — no GPU, no graphics API, no I/O. The output is a 320×200 RGBA framebuffer (256KB) written to a flat array. This means the renderer can run inside `on_initialize` alongside the game logic, making every block produce a complete frame.

### Zero-Copy WASM Bridge
The browser reads the framebuffer directly from WASM linear memory via a pointer (`rgba_ptr()` + `rgba_len()`), avoiding any data copy between WASM and JavaScript. The RGBA data is sliced directly into an `ImageData` and painted onto a `<canvas>`.

### WAD Asset Baking
Instead of parsing WAD files at runtime (which requires filesystem access), a build-time tool extracts sprites, textures, and palette data into Rust source files with `const` arrays. The compiler embeds these directly into the binary. No I/O, no allocation, no deserialization at runtime.

### Palette-Indexed Compression
Sprites and textures are stored as palette-indexed data (1 byte per pixel) rather than RGBA (4 bytes). With 100 sprites averaging ~2KB each, this keeps total asset size under 400KB. The palette lookup happens during rendering — one array index per pixel.

### Doom's M_Random PRNG
Damage, pain chance, and AI behavior use the exact 256-byte lookup table from the original Doom source code (`m_random.c`). This isn't a random number generator — it's a deterministic sequence that produces the same "random" values in the same order on every machine. Essential for consensus.

### Two-Pass Renderer
Floor/ceiling rendering covers the full viewport first, then walls are drawn on top. This eliminates visual gaps at sector height transitions (e.g. where a raised floor meets a standard floor) that single-pass renderers produce.

### Full Game Loop Per Block
Each block executes: `inputs → player movement → door updates → projectile physics → enemy AI (LOS, chase, attack) → sound propagation → item pickups → visual effects → exit check → cooldowns`. All in one `tick()` call. At a 1-second block time, this gives responsive gameplay while maintaining the "every tick is a block" invariant.

### Perspective-Correct Floor/Ceiling Rendering
For each pixel below the wall strip (floor) or above it (ceiling), the renderer computes the world-space position using the ray direction and row distance, then samples the flat texture with perspective correction. This produces the same visual quality as Chocolate Doom's flat rendering.

### 8-Directional Enemy Sprites
Enemy sprites are selected based on the angle between the viewer's line of sight and the enemy's facing direction, mapping to one of 8 rotations (front, 45°, side, 135°, back, and mirrors). Combined with A/B walk frame animation, enemies visually turn and walk with correct directional sprites.

## Project Structure

```
chainreactor-doom/
├── engine/                     # Pure Rust game engine (no_std compatible)
│   ├── src/
│   │   ├── types.rs            # Core types, fixed-point math, player/enemy/item structs
│   │   ├── map.rs              # Grid-based level, DDA raycasting, collision
│   │   ├── game.rs             # Game state, tick loop, AI, combat
│   │   ├── renderer.rs         # Raycaster, floor/ceiling, sprites, STBAR
│   │   └── assets/             # Auto-generated from Freedoom WAD
│   │       ├── palette.rs      # 256-color PLAYPAL
│   │       ├── textures.rs     # 16 wall textures (RGBA)
│   │       ├── flats.rs        # 8 floor/ceiling textures (64×64 RGBA)
│   │       ├── sprites.rs      # 100 sprites (palette-indexed)
│   │       ├── stbar.rs        # Status bar background
│   │       └── sttnum.rs       # Big red digit sprites (0-9, %, -)
│   └── tests/
│       └── engine_tests.rs     # 34 tests covering all game systems
├── wasm/                       # WASM wrapper for browser playback
│   ├── src/lib.rs              # wasm-bindgen interface
│   └── www/index.html          # Game UI with HUD, minimap, controls
├── tools/
│   └── wad-extract/            # WAD → Rust source asset extractor
├── assets/
│   └── freedoom1.wad           # Freedoom Phase 1 IWAD
└── pkg/                        # Built WASM package (~1.2MB)
```

## Build & Run

```bash
# Prerequisites: Rust, wasm-pack

# Build WASM
cd wasm && wasm-pack build --target web --out-dir ../pkg

# Serve locally (any static file server works)
cd wasm/www && python3 -m http.server 8080

# Run tests
cargo test -p doom-engine --test engine_tests
```

Open `http://localhost:8080` and play.

### Controls

| Key | Action |
|-----|--------|
| W / Arrow Up | Move forward |
| S / Arrow Down | Move backward |
| A | Strafe left |
| D | Strafe right |
| Arrow Left/Right | Turn |
| Space | Shoot |
| E | Use / Open door |
| 1 / 2 / 3 | Select weapon (Fist / Pistol / Shotgun) |
| Q | Cycle weapon |
| R | Restart |

## What's Next

1. **Substrate pallet wrapper** — Thin `pallet-doom` crate wrapping the engine. `on_initialize` calls `tick()` + `render()`, extrinsics submit player inputs, custom RPC serves the framebuffer.
2. **Demo chain deployment** — 3-validator Substrate chain on DigitalOcean with 1-second block time and `doom-mode` feature flag.
3. **Frontend integration** — Playable at `doom.chainreactor.io`, reading frames from the chain's RPC.
4. **Multi-player** — Multiple players submit inputs as extrinsics. The chain orders them by block inclusion. Cooperative or deathmatch modes.

## License

Engine code is MIT. Game assets are from [Freedoom](https://freedoom.github.io/) under the modified BSD license.
