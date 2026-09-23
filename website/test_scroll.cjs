const assert = require("node:assert/strict");
const { test } = require("node:test");

test("auto wheel routing is binary and uses canvas coordinates", async () => {
  const { routeWheel } = await import("../web/scroll.mjs");
  const canvas = new EventTarget();
  canvas.width = 600;
  canvas.height = 400;
  canvas.getBoundingClientRect = () => ({ left: 20, top: 30, width: 300, height: 200 });
  let accepts = false;
  const probes = [];
  routeWheel(canvas, (...input) => { probes.push(input); return accepts; });
  let delivered = 0;
  canvas.addEventListener("wheel", (event) => { delivered++; event.preventDefault(); });
  const send = (cancelable = true) => {
    const event = new Event("wheel", { cancelable });
    Object.assign(event, { clientX: 50, clientY: 70, deltaX: 2, deltaY: 5, deltaMode: 0 });
    canvas.dispatchEvent(event);
    return event;
  };
  assert.equal(send().defaultPrevented, false);
  assert.equal(delivered, 0);
  assert.deepEqual(probes, [[60, 80, 2, 5, 0]]);
  accepts = true;
  assert.equal(send().defaultPrevented, true);
  assert.equal(delivered, 1);
  // Never move both the page and editor for an event that cannot be cancelled.
  assert.equal(send(false).defaultPrevented, false);
  assert.equal(delivered, 1);
  accepts = false;
  assert.equal(send().defaultPrevented, false);
  assert.equal(delivered, 1);
});
