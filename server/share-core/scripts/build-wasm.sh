#!/usr/bin/env sh
# The only way the Worker gets its share-core package: a release build WITHOUT test features,
# then the export guard. Wrangler runs this before every deploy and `wrangler dev`
# (server/worker/wrangler.toml, wrangler.staging.toml: [build] command).
set -eu
cd "$(dirname "$0")/.."
wasm-pack build --release --target web
# `reinit()`: a fresh instance (new linear memory) of the same compiled module. A trap poisons an
# instance (round 14, critic26b: after ~219 trapped calls every call failed, argon2id included),
# and wasm-bindgen's `initSync` never instantiates twice. The Worker calls it after any exception
# from any export, then retries nothing (the request that trapped is answered with an error).
cat >> pkg/share_core.js <<'JS'

/** A fresh instance of the same module, after any exception from an export (see `health`). */
export function reinit() {
    const m = wasmModule;
    if (m === undefined) throw new Error("share-core: not initialised");
    // (The old instance's memory zeroed first: it held served orders and keys, round 16.)
    try { if (wasm !== undefined) new Uint8Array(wasm.memory.buffer).fill(0); } catch (_) {}
    wasm = undefined;
    wasmInstance = undefined;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    return initSync({ module: m });
}
JS
cat >> pkg/share_core.d.ts <<'TS'
/** A fresh instance of the same module, after any exception from an export (see `health`). */
export function reinit(): InitOutput;
TS
sh scripts/check-wasm-exports.sh pkg
