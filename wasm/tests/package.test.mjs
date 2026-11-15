// The built package, as Node loads it: run after build.sh with `node --test wasm/tests/*.test.mjs`.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { contrast, initSync, minContrast, render } from "../pkg/styled_qr_wasm.js";

initSync({ module: await readFile(new URL("../pkg/styled_qr_wasm_bg.wasm", import.meta.url)) });

test("draws an SVG, and a PNG when asked", () => {
    const drawn = render("https://bipa.app", { shape: "rounded", finder: "square", png: true });
    assert.match(drawn.svg, /^<svg/);
    assert.deepEqual([...drawn.png.slice(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
    assert.equal(render("https://bipa.app").png, undefined);
});

test("a refusal carries its code", () => {
    assert.throws(
        () => render("x", { colors: { dark: "#96c8c4", light: "#ffffff" } }),
        (refusal) => refusal.code === "contrast",
    );
    assert.throws(() => render("x", { shape: "stars" }), (refusal) => refusal.code === "options");
    assert.throws(() => render("x", {}, new Uint8Array([1, 2, 3])), (refusal) => refusal.code === "logoImage");
});

test("contrast is the WCAG ratio, and the minimum is 3", () => {
    assert.equal(contrast("#000000", "#ffffff"), 21);
    assert.equal(minContrast(), 3);
});

test("a logo linked by href leaves the SVG lighter and the PNG unchanged", async () => {
    const logo = new Uint8Array(await readFile(new URL("../../assets/mark.png", import.meta.url)));
    const linked = render("https://bipa.app", { logo: { href: "/qr/logo.png" }, png: true }, logo);
    const embedded = render("https://bipa.app", { png: true }, logo);
    assert.match(linked.svg, /xlink:href="\/qr\/logo\.png"/);
    assert.ok(linked.svg.length < embedded.svg.length);
    assert.deepEqual(linked.png, embedded.png);
});
