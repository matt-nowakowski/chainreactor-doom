/* tslint:disable */
/* eslint-disable */

/**
 * WASM-exposed game instance.
 */
export class DoomGame {
    free(): void;
    [Symbol.dispose](): void;
    alive_enemies(): number;
    current_tick(): bigint;
    current_weapon(): number;
    game_over(): boolean;
    has_blue_key(): boolean;
    has_chaingun(): boolean;
    has_red_key(): boolean;
    has_rocket_launcher(): boolean;
    has_shotgun(): boolean;
    /**
     * Returns true if on title screen.
     */
    is_title_screen(): boolean;
    level_complete(): boolean;
    constructor();
    player_alive(): boolean;
    player_ammo(): number;
    player_angle(): number;
    player_armor(): number;
    player_health(): number;
    player_kills(): number;
    player_rockets(): number;
    player_shells(): number;
    player_x(): number;
    player_y(): number;
    projectile_count(): number;
    /**
     * Render the current frame. Read pixels via rgba_ptr()/rgba_len().
     */
    render(): void;
    reset(): void;
    rgba_len(): number;
    rgba_ptr(): number;
    screen_height(): number;
    screen_width(): number;
    /**
     * Dismiss title screen and start the game.
     */
    start_game(): void;
    tick(input_codes: Uint8Array): void;
    total_enemies(): number;
}

/**
 * Standalone renderer: takes SCALE-encoded GameState bytes from the chain,
 * decodes them, renders a frame, and returns raw RGBA pixels.
 */
export function render_from_scale(state_bytes: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_doomgame_free: (a: number, b: number) => void;
    readonly doomgame_alive_enemies: (a: number) => number;
    readonly doomgame_current_tick: (a: number) => bigint;
    readonly doomgame_current_weapon: (a: number) => number;
    readonly doomgame_game_over: (a: number) => number;
    readonly doomgame_has_blue_key: (a: number) => number;
    readonly doomgame_has_chaingun: (a: number) => number;
    readonly doomgame_has_red_key: (a: number) => number;
    readonly doomgame_has_rocket_launcher: (a: number) => number;
    readonly doomgame_has_shotgun: (a: number) => number;
    readonly doomgame_is_title_screen: (a: number) => number;
    readonly doomgame_level_complete: (a: number) => number;
    readonly doomgame_new: () => number;
    readonly doomgame_player_alive: (a: number) => number;
    readonly doomgame_player_ammo: (a: number) => number;
    readonly doomgame_player_angle: (a: number) => number;
    readonly doomgame_player_armor: (a: number) => number;
    readonly doomgame_player_health: (a: number) => number;
    readonly doomgame_player_kills: (a: number) => number;
    readonly doomgame_player_rockets: (a: number) => number;
    readonly doomgame_player_shells: (a: number) => number;
    readonly doomgame_player_x: (a: number) => number;
    readonly doomgame_player_y: (a: number) => number;
    readonly doomgame_projectile_count: (a: number) => number;
    readonly doomgame_render: (a: number) => void;
    readonly doomgame_reset: (a: number) => void;
    readonly doomgame_rgba_len: (a: number) => number;
    readonly doomgame_rgba_ptr: (a: number) => number;
    readonly doomgame_screen_height: (a: number) => number;
    readonly doomgame_screen_width: (a: number) => number;
    readonly doomgame_start_game: (a: number) => void;
    readonly doomgame_tick: (a: number, b: number, c: number) => void;
    readonly doomgame_total_enemies: (a: number) => number;
    readonly render_from_scale: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
