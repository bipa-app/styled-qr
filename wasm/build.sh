#!/usr/bin/env bash
# Builds the npm package in wasm/pkg: the module, its types and the .wasm file.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --locked --release --target wasm32-unknown-unknown -p styled-qr-wasm
rm -rf pkg
wasm-bindgen --target web --out-dir pkg ../target/wasm32-unknown-unknown/release/styled_qr_wasm.wasm
cp package.json options.d.ts README.md pkg/
# render's options and refusal are typed, not `any`.
sed -i.bak \
    -e 's/^export function render(data: string, options: any, logo_png?: Uint8Array | null): Rendered;/export function render(data: string, options?: import(".\/options").QrOptions, logo_png?: Uint8Array | null): Rendered;/' \
    pkg/styled_qr_wasm.d.ts
rm pkg/*.bak
printf 'export type { QrOptions, QrRefusal } from "./options";\n' >> pkg/styled_qr_wasm.d.ts
