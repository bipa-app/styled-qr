# @bipa-app/styled-qr

The [styled-qr](https://github.com/bipa-app/styled-qr) crate built to WebAssembly: styled QR codes with logos, as SVG and PNG, in the browser and in Node.

```js
import init, { render } from "@bipa-app/styled-qr";

await init(); // in Node: initSync({ module: await readFile(wasmPath) })
const { svg } = render("https://bipa.app", { shape: "dots", finder: "rounded" });
const withLogo = render(brcode, { colors: { dark: "#113331", light: "#ffffff", finder: "#0f766e" }, png: true }, logoPng);
```

A page that shows many codes with one logo can serve the logo once and link it: `logo: { href: "/qr/logo.png" }` puts `<image xlink:href>` in the SVG instead of the embedded PNG. Only an SVG inline in HTML loads it, not one in `<img>` or drawn to a canvas; the PNG always embeds the logo. The logo bytes are still required, since the code's readability is checked against them.

A choice that can't make a readable code throws `{ code, message }`. The codes are `contrast`, `logoUnreadable`, `logoImage`, `color`, `data`, `tooLarge` and `options`.

Releases: pushing a `wasm-v<version>` tag that matches `wasm/package.json` attaches the package tarball to a GitHub release; depend on its URL, as in `"@bipa-app/styled-qr": "https://github.com/bipa-app/styled-qr/releases/download/wasm-v0.1.1/bipa-app-styled-qr-0.1.1.tgz"`.

Build with `wasm/build.sh` (needs the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.100), then test with `node --test wasm/tests/*.test.mjs`.
