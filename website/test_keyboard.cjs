const assert = require("node:assert/strict");
const { test } = require("node:test");

test("browser defaults depend on actual handling, with no duplicate Winit dispatch", async () => {
  const { routeKeyboard } = await import("../web/keyboard.mjs");
  for (const type of ["keydown", "keyup"]) {
    for (const handled of [false, true]) {
      const canvas = new EventTarget();
      const calls = [];
      routeKeyboard(canvas, (event) => { calls.push(event); return handled; });
      canvas.addEventListener(type, () => assert.fail("Winit must not see this key again"));
      const event = new Event(type, { cancelable: true });
      Object.assign(event, { key: "r", code: "KeyR", metaKey: true });
      canvas.dispatchEvent(event);
      assert.deepEqual(calls, [event]);
      assert.equal(event.defaultPrevented, handled);
    }
  }
});

test("repeats, text, dead keys and composition flags reach the dispatcher unchanged", async () => {
  const { routeKeyboard } = await import("../web/keyboard.mjs");
  const canvas = new EventTarget();
  const calls = [];
  routeKeyboard(canvas, (event) => { calls.push(event); return false; });
  for (const props of [
    { key: "é", code: "KeyE", altKey: true },
    { key: "ArrowLeft", code: "ArrowLeft", repeat: true },
    { key: "Dead", code: "Quote" },
    { key: "Process", isComposing: true },
  ]) {
    const event = new Event("keydown", { cancelable: true });
    Object.assign(event, props);
    canvas.dispatchEvent(event);
    assert.equal(calls.at(-1), event);
    assert.equal(event.defaultPrevented, false);
  }
});
