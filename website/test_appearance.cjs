const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

async function page({ stored = null, denied = false } = {}) {
  const helpers = await import("../web/theme.mjs");
  const listeners = {};
  const button = { attributes: {}, setAttribute(name, value) { this.attributes[name] = value; },
    addEventListener(type, callback) { this[type] = callback; } };
  const frames = Array.from({ length: 3 }, () => ({
    messages: [],
    addEventListener(type, callback) { this[type] = callback; },
    set src(_) { assert.fail("A theme change must not reload an editor"); },
  }));
  for (const frame of frames) frame.contentWindow = {
    postMessage(...args) { frame.messages.push(structuredClone(args)); },
  };
  const dataset = {};
  const storage = new Map([["progred-theme", stored]]);
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "public/appearance.js"), "utf8")
    .replace(/^import .*;$/gm, ""), {
    ...helpers,
    location: { origin: "http://localhost" },
    document: { documentElement: { dataset }, querySelector: () => button, querySelectorAll: () => frames },
    window: {
      get localStorage() {
        if (denied) throw new Error("Storage denied");
        return { getItem: (key) => storage.get(key), setItem: (key, value) => storage.set(key, value) };
      },
      addEventListener: (type, callback) => { listeners[type] = callback; },
    },
  });
  return { button, frames, dataset, storage, listeners };
}

test("light is the default; the toggle updates all embeds in place and remembers its choice", async () => {
  const p = await page();
  assert.equal(p.dataset.theme, "light");
  assert.equal(p.button.attributes["aria-checked"], "false");
  assert.equal(p.button.attributes.title, "Switch to dark mode");
  p.button.click();
  assert.equal(p.dataset.theme, "dark");
  assert.equal(p.button.attributes["aria-checked"], "true");
  assert.equal(p.button.attributes.title, "Switch to light mode");
  assert.equal(p.storage.get("progred-theme"), "dark");
  for (const frame of p.frames) {
    assert.deepEqual(frame.messages.at(-1), [{ type: "progred:theme", theme: "dark" }, "http://localhost"]);
  }
  p.button.click();
  assert.equal(p.dataset.theme, "light");
  assert.equal(p.button.attributes["aria-checked"], "false");
  assert.equal(p.button.attributes.title, "Switch to dark mode");
});

test("saved preference, lazy-loaded and reset lessons use the current theme", async () => {
  const p = await page({ stored: "dark" });
  assert.equal(p.dataset.theme, "dark");
  assert.equal(p.button.attributes["aria-checked"], "true");
  p.frames[0].load();
  const event = { origin: "http://localhost", source: p.frames[1].contentWindow,
    data: { type: "progred:ready" } };
  p.listeners.message({ ...event, origin: "http://other" });
  p.listeners.message({ ...event, source: {} });
  assert.equal(p.frames[1].messages.length, 1);
  p.listeners.message(event);
  assert.equal(p.frames[1].messages.length, 2);
  assert.equal(p.frames[0].messages.length, 2);
  p.listeners.storage({ key: "progred-theme", newValue: "light" });
  assert.equal(p.dataset.theme, "light");
  assert.equal(p.button.attributes["aria-checked"], "false");
  p.listeners.storage({ key: "progred-theme", newValue: "dark" });
  p.listeners.storage({ key: null, newValue: null });
  assert.equal(p.dataset.theme, "light");
});

test("disabled local storage doesn't prevent loading or switching themes", async () => {
  const p = await page({ denied: true });
  assert.equal(p.dataset.theme, "light");
  p.button.click();
  assert.equal(p.dataset.theme, "dark");
});
