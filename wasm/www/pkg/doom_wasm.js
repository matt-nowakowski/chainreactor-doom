/* @ts-self-types="./doom_wasm.d.ts" */

/**
 * WASM-exposed game instance.
 */
export class DoomGame {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        DoomGameFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_doomgame_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    alive_enemies() {
        const ret = wasm.doomgame_alive_enemies(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {bigint}
     */
    current_tick() {
        const ret = wasm.doomgame_current_tick(this.__wbg_ptr);
        return BigInt.asUintN(64, ret);
    }
    /**
     * @returns {number}
     */
    current_weapon() {
        const ret = wasm.doomgame_current_weapon(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    game_over() {
        const ret = wasm.doomgame_game_over(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_blue_key() {
        const ret = wasm.doomgame_has_blue_key(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_chaingun() {
        const ret = wasm.doomgame_has_chaingun(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_red_key() {
        const ret = wasm.doomgame_has_red_key(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_rocket_launcher() {
        const ret = wasm.doomgame_has_rocket_launcher(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_shotgun() {
        const ret = wasm.doomgame_has_shotgun(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Returns true if on title screen.
     * @returns {boolean}
     */
    is_title_screen() {
        const ret = wasm.doomgame_is_title_screen(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    level_complete() {
        const ret = wasm.doomgame_level_complete(this.__wbg_ptr);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.doomgame_new();
        this.__wbg_ptr = ret >>> 0;
        DoomGameFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {boolean}
     */
    player_alive() {
        const ret = wasm.doomgame_player_alive(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    player_ammo() {
        const ret = wasm.doomgame_player_ammo(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_angle() {
        const ret = wasm.doomgame_player_angle(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_armor() {
        const ret = wasm.doomgame_player_armor(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_health() {
        const ret = wasm.doomgame_player_health(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_kills() {
        const ret = wasm.doomgame_player_kills(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    player_rockets() {
        const ret = wasm.doomgame_player_rockets(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_shells() {
        const ret = wasm.doomgame_player_shells(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_x() {
        const ret = wasm.doomgame_player_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    player_y() {
        const ret = wasm.doomgame_player_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    projectile_count() {
        const ret = wasm.doomgame_projectile_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Render the current frame. Read pixels via rgba_ptr()/rgba_len().
     */
    render() {
        wasm.doomgame_render(this.__wbg_ptr);
    }
    reset() {
        wasm.doomgame_reset(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    rgba_len() {
        const ret = wasm.doomgame_rgba_len(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    rgba_ptr() {
        const ret = wasm.doomgame_rgba_ptr(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    screen_height() {
        const ret = wasm.doomgame_screen_height(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    screen_width() {
        const ret = wasm.doomgame_screen_width(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Dismiss title screen and start the game.
     */
    start_game() {
        wasm.doomgame_start_game(this.__wbg_ptr);
    }
    /**
     * @param {Uint8Array} input_codes
     */
    tick(input_codes) {
        const ptr0 = passArray8ToWasm0(input_codes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.doomgame_tick(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @returns {number}
     */
    total_enemies() {
        const ret = wasm.doomgame_total_enemies(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) DoomGame.prototype[Symbol.dispose] = DoomGame.prototype.free;

/**
 * Standalone renderer: takes SCALE-encoded GameState bytes from the chain,
 * decodes them, renders a frame, and returns raw RGBA pixels.
 * @param {Uint8Array} state_bytes
 * @returns {Uint8Array}
 */
export function render_from_scale(state_bytes) {
    const ptr0 = passArray8ToWasm0(state_bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.render_from_scale(ptr0, len0);
    var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v2;
}

function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_6ddd609b62940d55: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./doom_wasm_bg.js": import0,
    };
}

const DoomGameFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_doomgame_free(ptr >>> 0, 1));

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasm;
function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    wasmModule = module;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('doom_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
