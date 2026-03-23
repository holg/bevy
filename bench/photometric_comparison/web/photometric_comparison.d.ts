/* tslint:disable */
/* eslint-disable */

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly main: (a: number, b: number) => number;
    readonly wasm_bindgen_431eeca2fef2c144___closure__destroy___dyn_core_67558b4ca73dc0a8___ops__function__FnMut__core_67558b4ca73dc0a8___option__Option_web_sys_ddddf5dc85b839e3___features__gen_Blob__Blob_____Output_______: (a: number, b: number) => void;
    readonly wasm_bindgen_431eeca2fef2c144___closure__destroy___dyn_core_67558b4ca73dc0a8___ops__function__FnMut__wasm_bindgen_431eeca2fef2c144___JsValue____Output___core_67558b4ca73dc0a8___result__Result_____wasm_bindgen_431eeca2fef2c144___JsError___: (a: number, b: number) => void;
    readonly wasm_bindgen_431eeca2fef2c144___closure__destroy___dyn_core_67558b4ca73dc0a8___ops__function__FnMut_____Output_______: (a: number, b: number) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___js_sys_11b6d20dd46e6579___Array__web_sys_ddddf5dc85b839e3___features__gen_ResizeObserver__ResizeObserver______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue__core_67558b4ca73dc0a8___result__Result_____wasm_bindgen_431eeca2fef2c144___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__2: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__3: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__4: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__5: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__6: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__7: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___wasm_bindgen_431eeca2fef2c144___JsValue______true__8: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke___core_67558b4ca73dc0a8___option__Option_web_sys_ddddf5dc85b839e3___features__gen_Blob__Blob_______true_: (a: number, b: number, c: number) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly wasm_bindgen_431eeca2fef2c144___convert__closures_____invoke_______true__1_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
