# @bipa-app/styled-qr

The [styled-qr](https://github.com/bipa-app/styled-qr) crate built to WebAssembly: styled QR codes with logos, as SVG and PNG, in the browser and in Node.

```js
import init, { render } from "@bipa-app/styled-qr";

await init(); // in Node: initSync({ module: await readFile(wasmPath) })
const { svg } = render("https://bipa.app", { shape: "dots", finder: "rounded" });
const withLogo = render(brcode, { colors: { dark: "#113331", light: "#ffffff", finder: "#0f766e" }, png: true }, logoPng);
```

A choice that can't make a readable code throws `{ code, message }`. The codes are `contrast`, `logoUnreadable`, `logoImage`, `color`, `data`, `tooLarge` and `options`.

Releases: pushing a `wasm-v<version>` tag that matches `wasm/package.json` attaches the package tarball to a GitHub release; depend on its URL, as in `"@bipa-app/styled-qr": "https://github.com/bipa-app/styled-qr/releases/download/wasm-v0.1.0/bipa-app-styled-qr-0.1.0.tgz"`.

Build with `wasm/build.sh` (needs the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.100), then test with `node --test wasm/tests/*.test.mjs`.
