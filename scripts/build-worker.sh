#!/usr/bin/env bash
set -euo pipefail

# 1. Build wasm32-unknown-unknown release binary
if command -v cargo >/dev/null 2>&1; then
  cargo build --lib --target wasm32-unknown-unknown --release
elif [ -f "/mnt/c/Users/danny/.cargo/bin/cargo.exe" ]; then
  /mnt/c/Users/danny/.cargo/bin/cargo.exe build --lib --target wasm32-unknown-unknown --release
else
  echo "Error: cargo not found in PATH." >&2
  exit 1
fi

# 2. Ensure wasm-bindgen 0.2.129 (matching Cargo.lock) is cached
WASM_BINDGEN_VER="0.2.129"
CACHE_DIR="${HOME}/.cache/wasm-bindgen-${WASM_BINDGEN_VER}"
if [ ! -x "${CACHE_DIR}/wasm-bindgen" ]; then
  mkdir -p "${CACHE_DIR}"
  ARCH="$(uname -m)"
  if [ "${ARCH}" = "aarch64" ] || [ "${ARCH}" = "arm64" ]; then
    TARGET="aarch64-unknown-linux-musl"
  else
    TARGET="x86_64-unknown-linux-musl"
  fi
  curl -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${WASM_BINDGEN_VER}/wasm-bindgen-${WASM_BINDGEN_VER}-${TARGET}.tar.gz" \
    | tar -xz -C "${CACHE_DIR}" --strip-components=1
fi

# 3. Generate JS/Wasm bindings
rm -rf build
mkdir -p build/worker
"${CACHE_DIR}/wasm-bindgen" \
  target/wasm32-unknown-unknown/release/wayfit_backend.wasm \
  --out-dir build \
  --out-name index \
  --target bundler \
  --no-typescript

mv build/index_bg.js build/worker/index_bg.js
mv build/index_bg.wasm build/worker/index.wasm
if [ -d build/snippets ]; then
  mv build/snippets build/worker/snippets
fi

# 4. Write Cloudflare Workers Entrypoint shim
cat > build/worker/shim.js << 'EOF'
import * as imports from "./index_bg.js";
export * from "./index_bg.js";
import wasmModule from "./index.wasm";
import { WorkerEntrypoint } from "cloudflare:workers";

const instance = new WebAssembly.Instance(wasmModule, {
	"./index_bg.js": imports,
});

imports.__wbg_set_wasm(instance.exports);

// Run the worker's initialization function.
instance.exports.__wbindgen_start?.();

export { wasmModule };

class Entrypoint extends WorkerEntrypoint {
	async fetch(request) {
		let response = imports.fetch(request, this.env, this.ctx);
		return await response;
	}
}

const EXCLUDE_EXPORT = [
	"IntoUnderlyingByteSource",
	"IntoUnderlyingSink",
	"IntoUnderlyingSource",
	"MinifyConfig",
	"PolishConfig",
	"R2Range",
	"RequestRedirect",
	"fetch",
	"queue",
	"scheduled",
	"getMemory",
];

Object.keys(imports).map((k) => {
	if (!(EXCLUDE_EXPORT.includes(k) | k.startsWith("__"))) {
		Entrypoint.prototype[k] = imports[k];
	}
});

export default Entrypoint;
EOF

# 5. Bundle with esbuild
ESBUILD_BIN="$(find "${HOME}/.npm/_npx" -name esbuild -type f -executable 2>/dev/null | head -n 1 || true)"
if [ -n "${ESBUILD_BIN}" ]; then
  (cd build/worker && "${ESBUILD_BIN}" \
    --external:./index.wasm \
    --external:cloudflare:email \
    --external:cloudflare:sockets \
    --external:cloudflare:workers \
    --format=esm \
    --bundle \
    ./shim.js \
    --outfile=shim.mjs \
    --minify)
else
  (cd build/worker && npx --yes esbuild \
    --external:./index.wasm \
    --external:cloudflare:email \
    --external:cloudflare:sockets \
    --external:cloudflare:workers \
    --format=esm \
    --bundle \
    ./shim.js \
    --outfile=shim.mjs \
    --minify)
fi

rm -rf build/worker/snippets build/worker/index_bg.js build/worker/shim.js build/index.js
echo "Built build/worker/shim.mjs and build/worker/index.wasm successfully."
