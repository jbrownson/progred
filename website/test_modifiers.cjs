const assert = require("node:assert/strict");
const { test } = require("node:test");

function send(target, type, keys = {}) {
  const event = new Event(type, { cancelable: true });
  Object.assign(event, keys);
  target.dispatchEvent(event);
  assert.equal(event.defaultPrevented, false);
}

test("pointer modifiers are forwarded before any focus or click without swallowing motion", async () => {
  const { forwardModifiers } = await import("../web/modifiers.mjs");
  const canvas = new EventTarget();
  const host = new EventTarget();
  canvas.focus = () => assert.fail("hover must not take keyboard focus");
  const changes = [];
  forwardModifiers(canvas, host, (...state) => changes.push(state));
  let motions = 0;
  canvas.addEventListener("pointermove", () => motions++);
  send(canvas, "pointerover", { metaKey: true });
  for (let i = 0; i < 10; i++) send(canvas, "pointermove", { metaKey: true });
  assert.equal(motions, 10);
  assert.deepEqual(changes, [[false, false, false, true]]);
  send(canvas, "pointermove");
  send(canvas, "pointermove", { ctrlKey: true, shiftKey: true, altKey: true });
  assert.deepEqual(changes.slice(1), [[false, false, false, false], [true, true, true, false]]);
});

test("focus loss and regain re-observe even identical pointer flags", async () => {
  const { forwardModifiers } = await import("../web/modifiers.mjs");
  for (const boundary of ["focus", "blur"]) {
    const canvas = new EventTarget();
    const host = new EventTarget();
    const changes = [];
    forwardModifiers(canvas, host, (...state) => changes.push(state));
    send(canvas, "pointermove", { metaKey: true });
    for (const target of [canvas, host]) {
      send(target, boundary);
      send(canvas, "pointermove", { metaKey: true });
    }
    assert.equal(changes.length, 3);
  }
});

test("keyboard changes keep the pointer observation in sync", async () => {
  const { forwardModifiers } = await import("../web/modifiers.mjs");
  const canvas = new EventTarget();
  const changes = [];
  forwardModifiers(canvas, new EventTarget(), (...state) => changes.push(state));
  send(canvas, "pointermove", { metaKey: true });
  send(canvas, "keyup");
  send(canvas, "pointermove");
  send(canvas, "keydown", { ctrlKey: true });
  send(canvas, "wheel", { ctrlKey: true });
  assert.deepEqual(changes, [
    [false, false, false, true], [false, false, false, false], [false, true, false, false],
  ]);
});
