const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

const root = path.resolve(__dirname, "..");
const bootstrap = fs.readFileSync(path.join(root, "web/index.html"), "utf8")
  .match(/<script type="module">([\s\S]*?)<\/script>/)[1]
  .replace(/^\s*import .*;$/gm, "");

async function start(search, { ok = true, parseError = false, platform = "Linux x86_64", storedTheme, storageDenied = false, duringInit, capturesScroll = () => false } = {}) {
  const { commandIsMeta } = await import("../web/platform.mjs");
  const { isTheme, savedTheme } = await import("../web/theme.mjs");
  const { forwardModifiers } = await import("../web/modifiers.mjs");
  const { routeWheel } = await import("../web/scroll.mjs");
  const { routeKeyboard } = await import("../web/keyboard.mjs");
  const calls = [];
  const messages = [];
  const focusEvents = [];
  const listeners = {};
  const themeChanges = [];
  const modifierChanges = [];
  const keyboardEvents = [];
  const parent = { postMessage: (...args) => messages.push(structuredClone(args)) };
  const host = {
    parent,
    get localStorage() {
      if (storageDenied) throw new Error("Storage denied");
      return { getItem: () => storedTheme };
    },
    addEventListener: (event, callback) => { listeners[event] = callback; },
  };
  let onChange;
  const loading = { style: {} };
  const canvas = new EventTarget();
  canvas.width = 400;
  canvas.height = 200;
  canvas.getBoundingClientRect = () => ({ left: 0, top: 0, width: 200, height: 100 });
  const canvasListeners = [];
  const addCanvasListener = canvas.addEventListener.bind(canvas);
  canvas.addEventListener = (type, listener, options) => {
    canvasListeners.push({ type, options: structuredClone(options) });
    addCanvasListener(type, listener, options);
  };
  const documentElement = { style: {}, dataset: {} };
  const body = { style: {} };
  await vm.runInNewContext(`(async () => { ${bootstrap} })()`, {
    URL, URLSearchParams, Error, crossOriginIsolated: true,
    JSON, commandIsMeta, isTheme, savedTheme, forwardModifiers, routeWheel, routeKeyboard, navigator: { platform },
    location: { search, href: `http://localhost/editor/${search}`, origin: "http://localhost" },
    window: host,
    document: {
      documentElement, body,
      querySelector: (selector) => selector === "#loading" ? loading : canvas,
    },
    console: { info() {}, error() {} },
    fetch: async (url) => {
      calls.push(["fetch", url.href]);
      return { ok, status: ok ? 200 : 404, text: async () => "example source" };
    },
    init: async () => { calls.push(["init"]); duringInit?.(listeners, parent); },
    startWorker: async (...args) => { calls.push(["workers", args[4]]); },
    wasm: {
      browser_focus_changed: () => focusEvents.push("changed"),
      browser_modifiers_changed: (...state) => modifierChanges.push(state),
      browser_captures_scroll: capturesScroll,
      browser_keyboard: (event) => { keyboardEvents.push(event); return false; },
      set_theme: (theme) => themeChanges.push(theme),
      computation_finished() {},
      worker_threads: () => 1,
      prepare_renderer: async (target) => { assert.equal(target, canvas); return "test"; },
      start_editor: (...args) => {
        if (parseError) throw "invalid document";
        calls.push(["editor", ...args]);
        onChange = args[2];
      },
    },
  });
  return { calls, loading, messages, onChange, focusEvents, listeners, parent, themeChanges, modifierChanges, keyboardEvents,
    canvas, canvasListeners, documentElement, body };
}

test("browser window focus is forwarded without page click handlers", async () => {
  const { focusEvents, listeners } = await start("?menu=hidden");
  assert.deepEqual(focusEvents, ["changed"]);
  assert.deepEqual(Object.keys(listeners).sort(), ["blur", "focus", "message"]);
  listeners.blur();
  listeners.focus();
  assert.deepEqual(focusEvents, ["changed", "changed", "changed"]);
});

test("standalone startup remains blank with its full menu and default workers", async () => {
  const { calls, loading } = await start("");
  assert.deepEqual(calls, [["init"], ["workers", undefined], ["editor", undefined, true, undefined, undefined, undefined, false, "light"]]);
  assert.equal(loading.style.display, "none");
});

test("embedded pointer modifiers reach WASM before the first click", async () => {
  const { canvas, modifierChanges } = await start("?menu=hidden&wheel=page");
  const move = new Event("pointermove", { cancelable: true });
  Object.assign(move, { metaKey: true });
  canvas.dispatchEvent(move);
  assert.deepEqual(modifierChanges, [[false, false, false, true]]);
  assert.equal(move.defaultPrevented, false);
});

test("editor and lesson labels use the browser host's command modifier", async () => {
  for (const [platform, meta] of [["MacIntel", true], ["iPad", true], ["Win32", false], ["Linux x86_64", false]]) {
    const { calls } = await start("", { platform });
    assert.equal(calls.find(([name]) => name === "editor")[6], meta);
    assert.equal((await lessonPage(platform)).commandLabel.textContent, meta ? "Cmd" : "Ctrl");
  }
});

test("wheel handling defaults to the editor, including embeds without an explicit choice", async () => {
  for (const search of ["", "?wheel=editor", "?menu=hidden&document=../lessons/values.gid"]) {
    const { canvas, canvasListeners, documentElement, body } = await start(search);
    assert.deepEqual(canvasListeners.filter(({ type }) => type === "wheel"),
      [{ type: "wheel", options: { capture: true, passive: true } }]);
    assert.equal(documentElement.style.overscrollBehavior, undefined);
    assert.equal(body.style.overscrollBehavior, undefined);
    let received = 0;
    canvas.addEventListener("wheel", (event) => { received++; event.preventDefault(); });
    const event = new Event("wheel", { cancelable: true });
    canvas.dispatchEvent(event);
    assert.equal(received, 1);
    assert.equal(event.defaultPrevented, true);
  }
});

test("page wheel handling leaves the browser default intact and excludes editor wheel listeners", async () => {
  const { canvas, canvasListeners, documentElement, body, keyboardEvents } = await start("?wheel=page");
  assert.deepEqual(canvasListeners.filter(({ type }) => type === "wheel"),
    Array.from({ length: 2 }, () => ({ type: "wheel", options: { capture: true, passive: true } })));
  assert.equal(documentElement.style.overscrollBehavior, "auto");
  assert.equal(body.style.overscrollBehavior, "auto");
  let received = 0;
  canvas.addEventListener("wheel", (event) => { received++; event.preventDefault(); });
  for (const cancelable of [true, false]) {
    const event = new Event("wheel", { cancelable });
    canvas.dispatchEvent(event);
    assert.equal(event.defaultPrevented, false);
  }
  assert.equal(received, 0);
  for (const type of ["pointerdown", "pointermove", "pointerup", "keydown"]) {
    canvas.addEventListener(type, () => { received++; });
    canvas.dispatchEvent(new Event(type));
  }
  assert.equal(received, 3);
  assert.equal(keyboardEvents.length, 1); // Keys use the synchronous route, independently of wheel policy.
});

test("invalid wheel configuration fails before starting workers or the editor", async () => {
  for (const search of ["?wheel=", "?wheel=unknown"]) {
    const { calls, loading } = await start(search);
    assert.deepEqual(calls, []);
    assert.equal(loading.style.display, "grid");
    assert.match(loading.textContent, /wheel must be 'editor', 'auto', or 'page'/);
  }
});

test("auto wheel mode consults the installed WASM probe before allowing editor dispatch", async () => {
  let accepts = false;
  const probes = [];
  const { canvas, documentElement, body } = await start("?wheel=auto", {
    capturesScroll: (...args) => { probes.push(args); return accepts; },
  });
  assert.equal(documentElement.style.overscrollBehavior, "auto");
  assert.equal(body.style.overscrollBehavior, "auto");
  let received = 0;
  canvas.addEventListener("wheel", (event) => { received++; event.preventDefault(); });
  for (const take of [false, true, false]) {
    accepts = take;
    const event = new Event("wheel", { cancelable: true });
    Object.assign(event, { clientX: 20, clientY: 30, deltaX: 0, deltaY: 10, deltaMode: 0 });
    canvas.dispatchEvent(event);
    assert.equal(event.defaultPrevented, take);
  }
  assert.equal(received, 1);
  assert.deepEqual(probes, Array.from({ length: 3 }, () => [40, 60, 0, 10, 0]));
});

test("embed fetches its document and supplies ordinary startup options", async () => {
  const { calls } = await start("?document=../lessons/values.gid&menu=hidden&threads=1");
  assert.deepEqual(calls, [
    ["fetch", "http://localhost/lessons/values.gid"], ["init"],
    ["workers", 1], ["editor", "example source", false, undefined, undefined, undefined, false, "light"],
  ]);
});

test("explicit library lists, including an empty list, reach editor startup unchanged", async () => {
  for (const libraries of ["3209ad5d23a0c8513f6bd76324a5cf60,eaaf309c36a65d2811083944da29aec9", ""]) {
    const { calls } = await start(`?libraries=${libraries}`);
    assert.equal(calls.find(([name]) => name === "editor")[4], libraries);
  }
});

test("tutorial slot configuration reaches the host only when requested", async () => {
  const slots = "9940ece27410c72a5308a544890ccc71,f717b766d250a7b86c5eb842885c4417";
  const { calls } = await start(`?tutorial-slots=${slots}`);
  assert.equal(calls.find(([name]) => name === "editor")[5], slots);
});

test("missing or malformed documents report errors instead of starting empty", async () => {
  for (const options of [{ ok: false }, { parseError: true }]) {
    const { calls, loading } = await start("?document=../lessons/values.gid", options);
    assert.equal(calls.some(([name]) => name === "editor"), false);
    assert.equal(loading.style.display, "grid");
    assert.match(loading.textContent, /404|invalid document/);
  }
});

test("observation is opt-in and forwards ordinary state only to the same-origin parent", async () => {
  const { onChange, messages } = await start("?observe=values-0");
  assert.deepEqual(messages.shift(), [{ type: "progred:ready" }, "http://localhost"]);
  const state = { document: { root: { list: [] }, cells: {} }, selection: null };
  onChange(JSON.stringify(state));
  assert.equal(messages.length, 1);
  assert.deepEqual(messages[0], [{ type: "progred:change", channel: "values-0", state }, "http://localhost"]);
  assert.equal((await start("")).onChange, undefined);
});

test("theme startup defaults to light and tolerates unavailable preferences", async () => {
  for (const [options, expected] of [
    [{}, "light"], [{ storedTheme: "dark" }, "dark"],
    [{ storedTheme: "nonsense" }, "light"], [{ storageDenied: true }, "light"],
  ]) {
    const page = await start("", options);
    assert.equal(page.documentElement.dataset.theme, expected);
    assert.equal(page.calls.find(([name]) => name === "editor")[7], expected);
  }
  const explicit = await start("?theme=light", { storedTheme: "dark" });
  assert.equal(explicit.calls.find(([name]) => name === "editor")[7], "light");
  const invalid = await start("?theme=unknown");
  assert.deepEqual(invalid.calls, []);
  assert.match(invalid.loading.textContent, /theme must be/);
});

test("same-origin parent can change appearance without restarting the editor", async () => {
  const page = await start("");
  const event = { origin: "http://localhost", source: page.parent,
    data: { type: "progred:theme", theme: "dark" } };
  page.listeners.message({ ...event, origin: "http://other" });
  page.listeners.message({ ...event, source: {} });
  page.listeners.message({ ...event, data: { type: "progred:theme", theme: "bad" } });
  assert.deepEqual(page.themeChanges, []);
  page.listeners.message(event);
  assert.deepEqual(page.themeChanges, ["dark"]);
  assert.equal(page.documentElement.dataset.theme, "dark");
  assert.equal(page.calls.filter(([name]) => name === "editor").length, 1);
});

test("a theme arriving while WASM loads becomes the startup theme", async () => {
  const page = await start("", { duringInit(listeners, parent) {
    listeners.message({ origin: "http://localhost", source: parent,
      data: { type: "progred:theme", theme: "dark" } });
  } });
  assert.equal(page.calls.find(([name]) => name === "editor")[7], "dark");
  assert.deepEqual(page.themeChanges, []);
});

async function lessonPage(platform = "Linux x86_64") {
  const { commandIsMeta } = await import("../web/platform.mjs");
  const commandLabel = { textContent: "Ctrl" };
  const { lessonProgress } = await import("./public/lesson-progress.mjs");
  const messageListeners = [];
  const tasksByLesson = {
    values: ["rename", "moons", "add", "remove", "restore"],
    cells: ["shared-edit", "create", "link", "linked-edit"],
    grap: ["argument", "shared-edit", "equal", "hundred"],
    functions: ["argument", "body", "rename", "nest"],
    drawing: ["argument", "body", "add", "source"],
    forest: ["height", "color", "plant"],
    "growing-forest": [],
    projections: ["full", "plain", "raw"],
    model: ["rename", "add", "key"],
  };
  const exercises = Object.keys(tasksByLesson).map((name) => {
    const listeners = {};
    const status = { textContent: "" };
    const tasks = tasksByLesson[name].map((task) => {
      const marker = { textContent: "" };
      const status = { textContent: "" };
      const classes = new Set();
      return {
        dataset: { task }, marker, status, classes,
        classList: { toggle: (name, on) => on ? classes.add(name) : classes.delete(name) },
        querySelector: (selector) => ({ ".task-marker": marker, ".task-status": status })[selector],
      };
    });
    const frame = {
      src: `./editor/?document=../lessons/${name}.gid&menu=hidden&wheel=auto&threads=1${name === "growing-forest" ? "" : `&observe=${name}-0`}`,
      contentWindow: {},
      getAttribute() { return this.src; },
      addEventListener: (event, callback) => { listeners[event] = callback; },
    };
    const button = { addEventListener: (event, callback) => { listeners[event] = callback; } };
    return {
      id: name, frame, status, tasks, listeners,
      querySelectorAll: () => tasks,
      querySelector: (selector) => ({ iframe: frame, ".reset-status": status, ".reset": button })[selector],
    };
  });
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "public/lessons.js"), "utf8").replace(/^import .*;$/gm, ""), {
    URL, lessonProgress, commandIsMeta, navigator: { platform },
    location: { href: "http://localhost/", origin: "http://localhost" },
    window: { addEventListener: (_, callback) => messageListeners.push(callback) },
    document: { querySelectorAll: (selector) => selector === "[data-command-key]" ? [commandLabel] : exercises },
  });
  return {
    commandLabel,
    exercises,
    send(index, state, overrides = {}) {
      const exercise = exercises[index];
      const event = {
        origin: "http://localhost", source: exercise.frame.contentWindow,
        data: { type: "progred:change", channel: new URL(exercise.frame.src, "http://localhost").searchParams.get("observe"), state },
        ...overrides,
      };
      messageListeners.forEach((listener) => listener(event));
    },
  };
}

test("reset reloads only its own iframe", async () => {
  const { exercises } = await lessonPage();
  const resetCounts = exercises.map(() => 0);
  for (const [index, exercise] of exercises.entries()) {
    const src = exercise.frame.src;
    Object.defineProperty(exercise.frame, "src", {
      get: () => src,
      set: (value) => {
        const url = new URL(value);
        assert.equal(url.searchParams.get("document"), `../lessons/${exercise.id}.gid`);
        assert.equal(url.searchParams.get("observe"), exercise.id === "growing-forest" ? null : `${exercise.id}-1`);
        assert.equal(url.searchParams.get("wheel"), "auto");
        resetCounts[index]++;
      },
    });
  }
  for (const [index, exercise] of exercises.entries()) {
    exercise.listeners.click();
    assert.deepEqual(resetCounts, exercises.map((_, i) => i <= index ? 1 : 0));
    exercise.listeners.load();
    assert.equal(exercise.status.textContent, "Example reset.");
    if (index + 1 < exercises.length) assert.equal(exercises[index + 1].status.textContent, "");
  }
});

const record = (...entries) => ({ record: entries });
const text = (value) => record(["332529b8ea83a7ba10fd7f6d942e5016", { blob: Buffer.from(value).toString("hex") }]);
const number = (value) => {
  const bytes = Buffer.alloc(8);
  bytes.writeDoubleLE(value);
  return record(["ed11fde03b7c2c1ba2fccc3cdba5d561", { blob: bytes.toString("hex") }]);
};
const planet = (name = text("Mars"), moons = number(2), colors = ["red", "orange"]) => ({ document: { root: record(
  ["6655e4ba9e8ae76706056e0b8edf94f0", name],
  ["6122302fdbe3db3624731717321b94a1", moons],
  ["c785948e50c854a7c89bf6c4a4bdf4fc", { list: colors.map(text) }],
) }, selection: null });
const palette = (...colors) => planet(text("Mars"), number(2), colors);
const sharedCell = "f56d42a97558ccc8205fb4019b878945";
const cell = (id) => ({ cell: id });
const cells = (list, definitions) => ({ document: { root: { list }, cells: definitions }, selection: null });
const sharedPair = [cell(sharedCell), cell(sharedCell)];

test("planet quests need changed, valid values rather than selections or missing fields", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("values", planet()), []);
  assert.deepEqual(completedSteps("values", planet(text("Venus"), number(5))), ["rename", "moons"]);
  assert.deepEqual(completedSteps("values", planet(text(""), number(0))), ["rename", "moons"]);
  assert.deepEqual(completedSteps("values", planet(record(), number(NaN))), []);
  assert.deepEqual(completedSteps("values", planet(record(), number(Infinity))), []);
  assert.deepEqual(completedSteps("values", palette("red", "blue", "orange")), ["add"]);
  assert.deepEqual(completedSteps("values", { document: { root: null } }), []);
});

test("achievements latch through undo, stay independent, and ignore stale or unrelated messages", async () => {
  const { exercises, send } = await lessonPage();
  const success = planet(text("Venus"), number(5), ["red", "blue", "orange"]);
  const page = exercises[0];
  const done = () => page.tasks.filter(t => t.classes.has("completed")).length;
  send(0, success, { origin: "https://elsewhere.example" });
  send(0, success, { source: {} });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].marker.textContent, "✓");
  assert.equal(done(), 3);
  assert.equal(exercises[1].tasks[0].classes.has("completed"), false);
  send(0, planet(text("Mars"), number(2), ["red", "blue", "orange"]));
  assert.equal(done(), 3);
  page.listeners.click();
  assert.equal(done(), 0);
  send(0, success, { data: { type: "progred:change", channel: "values-0", state: success } });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].classes.has("completed"), true);
});

test("removal observes a shrinking color list, not empty text or a missing root", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const original = palette("red", "blue", "orange");
  assert.deepEqual(completedSteps("values", original), ["add"]);
  assert.deepEqual(completedSteps("values", palette("red", "", "orange"), original), ["add"]);
  assert.deepEqual(completedSteps("values", palette("red", "orange"), original), ["remove"]);
  assert.deepEqual(completedSteps("values", { document: { root: null } }, original), []);
  assert.deepEqual(completedSteps("values", original, { document: { root: null } }), ["add"]);
  assert.deepEqual(completedSteps("values", original, palette("red", "orange")), ["add"]);
});

test("removal and restoration have separate checkmarks and both reset", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[0];
  const removal = page.tasks.find((task) => task.dataset.task === "remove");
  const restoration = page.tasks.find((task) => task.dataset.task === "restore");
  send(0, palette("red", "orange"));
  send(0, palette("red", "blue", "orange"));
  send(0, palette("red", "blu", "orange"));
  send(0, palette("red", "", "orange"));
  send(0, palette("red", "orange"));
  assert.equal(removal.marker.textContent, "✓");
  assert.equal(restoration.classes.has("completed"), false);
  send(0, palette("red", "blue", "orange"));
  assert.equal(restoration.marker.textContent, "✓");
  send(0, palette("red", "orange"));
  assert.equal(restoration.marker.textContent, "✓");
  page.listeners.click();
  send(0, palette("red", "orange"));
  assert.equal(removal.marker.textContent, 4);
  assert.equal(removal.classes.has("completed"), false);
  assert.equal(restoration.classes.has("completed"), false);
  send(0, palette("red", "blue", "orange"));
  assert.equal(restoration.classes.has("completed"), false);
});

test("restoration needs a previously seen list after removal, not an unrelated edit or insertion", async () => {
  const { lessonProgress } = await import("./public/lesson-progress.mjs");
  const observe = lessonProgress("values");
  const original = palette("red", "blue", "orange");
  observe(original);
  observe(palette("red", "green", "orange"));
  assert.equal(observe(original).has("restore"), false);
  observe(palette("red", "orange"));
  assert.equal(observe(palette("red", "violet", "orange")).has("restore"), false);
  observe({ document: { root: null } });
  assert.equal(observe(original).has("restore"), false);
  observe(palette("red", "orange"));
  assert.equal(observe(original).has("restore"), true);
});

test("cell quests observe shared definitions, not equal numbers or unresolved references", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("cells", cells(sharedPair, { [sharedCell]: number(7) })), []);
  assert.deepEqual(completedSteps("cells", cells(sharedPair, { [sharedCell]: number(8) })), ["shared-edit"]);
  assert.deepEqual(completedSteps("cells", cells([cell(sharedCell)], { [sharedCell]: number(8) })), []);
  assert.deepEqual(completedSteps("cells", cells([number(11), number(11)], {})), []);
  assert.deepEqual(completedSteps("cells", cells([cell("new"), cell("new")], {})), []);
  assert.deepEqual(completedSteps("cells", cells([cell("first"), cell("second")], {
    first: number(11), second: number(11),
  })), ["create"]);
  for (const value of [record(), text("11"), number(NaN), number(Infinity)]) {
    assert.deepEqual(completedSteps("cells", cells(sharedPair, { [sharedCell]: value })), []);
    assert.deepEqual(completedSteps("cells", cells([cell("new"), cell("new")], { new: value })), []);
  }
  assert.deepEqual(completedSteps("cells", { document: { root: null } }), []);
});

test("creating and linking use a different cell from the supplied shared pair", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const definitions = { [sharedCell]: number(7), new: number(11) };
  assert.deepEqual(completedSteps("cells", cells(sharedPair, definitions)), []);
  assert.deepEqual(completedSteps("cells", cells([...sharedPair, cell("new")], definitions)), ["create"]);
  assert.deepEqual(completedSteps("cells", cells([...sharedPair, cell("new"), cell("new")], definitions)), ["create", "link"]);
  assert.deepEqual(completedSteps("cells", cells([...sharedPair, cell(sharedCell)], { [sharedCell]: number(11) })), ["shared-edit"]);
});

test("editing the new pair counts only after both references already exist", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const single = cells([...sharedPair, cell("new")], { [sharedCell]: number(7), new: number(11) });
  const paired = cells([...single.document.root.list, cell("new")], single.document.cells);
  const edited = cells(paired.document.root.list, { [sharedCell]: number(7), new: number(12) });
  assert.deepEqual(completedSteps("cells", paired, single), ["create", "link"]);
  assert.deepEqual(completedSteps("cells", edited, single), ["link"]);
  assert.deepEqual(completedSteps("cells", edited, paired), ["link", "linked-edit"]);
  assert.deepEqual(completedSteps("cells", edited, edited), ["link"]);
  assert.deepEqual(completedSteps("cells", edited), ["link"]);
});

test("the cells checklist completes through sharing, latches, and resets independently", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[1];
  const original = cells(sharedPair, { [sharedCell]: number(7) });
  send(1, original);
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  send(1, cells(sharedPair, { [sharedCell]: number(8) }));
  const definitions = { [sharedCell]: number(8), new: number(11) };
  send(1, cells([...sharedPair, cell("new")], definitions));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 2);
  const paired = [...sharedPair, cell("new"), cell("new")];
  send(1, cells(paired, definitions));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 3);
  send(1, cells(paired, { ...definitions, new: number(12) }));
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  assert.equal(exercises[0].tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[2].tasks.filter(t => t.classes.has("completed")).length, 0);
  send(1, original);
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(0, planet(text("Venus")));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[0].tasks.filter(t => t.classes.has("completed")).length, 1);
});

const grapInput = "4df73aa7950eafc65d50d9c6ceca0721";
const slots = [
  "9940ece27410c72a5308a544890ccc71",
  "f717b766d250a7b86c5eb842885c4417",
  "5e716c07490849f072b4e9017dd6230d",
];
const stacked = (items, definitions) => ({
  document: {
    root: record(...slots.map((slot, index) => [slot, items[index]])),
    cells: definitions,
  },
});
const items = (state) => slots.map((slot) => state.document.root.record.find(([id]) => id === slot)?.[1]);
const removeSlot = (state, index) => {
  state.document.root.record = state.document.root.record.filter(([id]) => id !== slots[index]);
};
const grapState = (argument = 2, input = 3, multiplier = 2) => stacked(
  [cell(grapInput), ...["201af445eb7e2c270bb5ead10b781fc1", "d6f384c439d9d69996d545df422efd79"].map((fn, index) => record(
    ["acfc5e50881292518dab3cec77cf43ee", record(
      ["751fca4373debdd0b7e6eb73e08d684b", cell(fn)],
      ["764f6afe17ba14e81f5ab61204be0bec", cell(grapInput)],
      ["4f53ff25390f58472d31a6142644dec2", number(index === 0 ? argument : multiplier)],
    )],
  ))],
  { [grapInput]: number(input) },
);

test("Grap quests distinguish literal arguments, the shared input, and the puzzles", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("grap", grapState()), []);
  assert.deepEqual(completedSteps("grap", grapState(4)), ["argument"]);
  assert.deepEqual(completedSteps("grap", grapState(2, 5)), ["shared-edit"]);
  assert.deepEqual(completedSteps("grap", grapState(4, 5)), ["argument", "shared-edit"]);
  // 2 + 2 equals 2 × 2; 50 × 2 is 100.
  assert.deepEqual(completedSteps("grap", grapState(2, 2)), ["shared-edit", "equal"]);
  assert.deepEqual(completedSteps("grap", grapState(2, 50)), ["shared-edit", "hundred"]);
  assert.deepEqual(completedSteps("grap", grapState(2, 3, 34)), []);
  assert.deepEqual(completedSteps("grap", grapState(NaN, 3)), []);
  assert.deepEqual(completedSteps("grap", grapState(2, Infinity)), []);
  assert.deepEqual(completedSteps("grap", grapState(4, NaN)), []);
  assert.deepEqual(completedSteps("grap", { document: { root: null } }), []);
  const missing = grapState(4, 5);
  removeSlot(missing, 2);
  assert.deepEqual(completedSteps("grap", missing), []);
  const unrelated = grapState();
  unrelated.document.root.record.push(["extra", number(4)]);
  assert.deepEqual(completedSteps("grap", unrelated), []);
});

test("Grap sharing requires references to the same resolved numeric cell", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  for (const definition of [undefined, record(), text("5"), number(NaN), number(Infinity)]) {
    const state = grapState(4, 5);
    state.document.cells[grapInput] = definition;
    assert.deepEqual(completedSteps("grap", state), []);
  }
  for (const replacement of [number(5), cell("separate")]) {
    const state = grapState(2, 5);
    items(state)[2].record[0][1].record[1][1] = replacement;
    state.document.cells.separate = number(5);
    assert.deepEqual(completedSteps("grap", state), []);
  }
  const missingReference = grapState(2, 5);
  removeSlot(missingReference, 0);
  assert.deepEqual(completedSteps("grap", missingReference), []);
});

test("Grap progress latches through undo and resets without touching earlier lessons", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[2];
  send(2, grapState());
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  send(2, grapState(4));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 1);
  send(2, grapState(4, 5));
  send(2, grapState(2, 2));
  send(2, grapState(2, 50));
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  send(2, grapState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(0, planet(text("Venus")));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[0].tasks.filter(t => t.classes.has("completed")).length, 1);
});

const scaleFunction = "27b02645cf74e4db930f7ace768a0aaf";
const scaleParameter = "a6ea8f498dc19d41294f9610ac9e8eed";
const scaleCall = (argument) => record(
  ["751fca4373debdd0b7e6eb73e08d684b", cell(scaleFunction)],
  [scaleParameter, argument],
);
const functionsState = ({ argument = 3, factor = 2, name = "x", nested = false } = {}) => stacked(
  [cell(scaleFunction), ...[number(argument), nested ? scaleCall(number(5)) : number(5)].map((value) =>
    record(["acfc5e50881292518dab3cec77cf43ee", scaleCall(value)]))],
  {
    [scaleFunction]: record(
      ["02e562654d6d0828d3a7559e6f75fffe", text("scale")],
      ["195b378d0d31d90ab0d7366c15346b70", { list: [cell(scaleParameter)] }],
      ["986143866eda2e2fbf9ab8484357a0c9", record(
        ["751fca4373debdd0b7e6eb73e08d684b", cell("d6f384c439d9d69996d545df422efd79")],
        ["764f6afe17ba14e81f5ab61204be0bec", cell(scaleParameter)],
        ["4f53ff25390f58472d31a6142644dec2", number(factor)],
      )],
    ),
    [scaleParameter]: record(["02e562654d6d0828d3a7559e6f75fffe", text(name)]),
  },
);

test("function quests distinguish arguments, the body, a parameter rename, and nesting", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("functions", functionsState()), []);
  assert.deepEqual(completedSteps("functions", functionsState({ argument: 4 })), ["argument"]);
  assert.deepEqual(completedSteps("functions", functionsState({ factor: 3 })), ["body"]);
  assert.deepEqual(completedSteps("functions", functionsState({ name: "amount" })), ["rename"]);
  assert.deepEqual(completedSteps("functions", functionsState({ nested: true })), ["nest"]);
  assert.deepEqual(completedSteps("functions", functionsState({ argument: 4, factor: 3, name: "amount", nested: true })),
    ["argument", "body", "rename", "nest"]);
  for (const invalid of [NaN, Infinity]) {
    assert.deepEqual(completedSteps("functions", functionsState({ argument: invalid, factor: 3 })), ["body"]);
    assert.deepEqual(completedSteps("functions", functionsState({ factor: invalid, name: "amount" })), []);
  }
  assert.deepEqual(completedSteps("functions", functionsState({ name: "" })), []);
  assert.deepEqual(completedSteps("functions", { document: { root: null } }), []);
});

test("function achievements require the shown definition and its connected calls", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  for (const disconnect of [
    (state) => { delete state.document.cells[scaleFunction]; },
    (state) => { removeSlot(state, 0); },
    (state) => { state.document.cells[scaleFunction].record[1][1].list.push(cell("other")); },
    (state) => { state.document.cells[scaleFunction].record[2][1].record[1][1] = cell("other"); },
  ]) {
    const state = functionsState({ argument: 4, factor: 3, name: "amount", nested: true });
    disconnect(state);
    assert.deepEqual(completedSteps("functions", state), []);
  }
  const otherFunction = functionsState({ argument: 4 });
  items(otherFunction)[1].record[0][1].record[0][1] = cell("other");
  assert.deepEqual(completedSteps("functions", otherFunction), []);
  const renamedFunction = functionsState();
  renamedFunction.document.cells[scaleFunction].record[0][1] = text("amount");
  assert.deepEqual(completedSteps("functions", renamedFunction), []);
});

test("function progress latches through undo and resets only its own lesson", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[3];
  send(3, functionsState());
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  send(3, functionsState({ argument: 4 }));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 1);
  send(3, functionsState({ argument: 4, factor: 3 }));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 2);
  send(3, functionsState({ argument: 4, factor: 3, name: "amount", nested: true }));
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  send(3, functionsState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(2, grapState(4, 5));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[2].tasks.filter(t => t.classes.has("completed")).length, 2);
});

const drawingIds = {
  fn: "c1ae4281e6899b6a268bc154fe127b7b",
  program: "d4ebfe96309382811fa21c889be44f53",
  function: "751fca4373debdd0b7e6eb73e08d684b",
  params: "195b378d0d31d90ab0d7366c15346b70",
  body: "986143866eda2e2fbf9ab8484357a0c9",
  drawing: "6889fa235b002be4c8b106d5f31dafbf",
  programField: "bdf607810b274ad5b02bd409aef34b6a",
  fill: "1624dc973ec7790203eb8fd22c9d6d05",
  shape: "fcaaadef14980397cce95363fc5a54f9",
  circle: "e06a6d094c4f75cd6c1d59ed6ed64e05",
  x: "415def0fa0a9ac40dfba5fca4d0f8876",
  y: "4e2dcde5b1ab1480a2f126176dd148c7",
  radius: "6423c35e07d7a4ff536127d1f1d8eb53",
  do: "b1fc4cb45c58b1a662c431feef5bd140",
  expressions: "5fab151c006ae1487c28837f2003f43c",
  quote: "7f81d4812ceb33d4222e9e5cb9c82497",
  expression: "ccc55b0eb63b9f564ea74436094d4014",
  unquote: "da48703c290e3b35d7353c38110bc953",
  key: "f12dea12c741fe36312750a264f3a235",
  follow: "33c6fb363ddc6f13fd054c68f7a37a98",
  document: "ef62fa62f008c1eca70389571933aba0",
};
const drawingState = ({ x = 60, radius = 24, dots = [160] } = {}) => {
  const d = drawingIds;
  return stacked([
    cell(d.fn), cell(d.program), record([d.drawing, record([d.programField, cell(d.program)])]),
  ], {
    [d.fn]: record(
      [d.params, { list: [cell(d.x)] }],
      [d.body, record([d.function, cell(d.fill)], [d.shape, record(
        [d.function, cell(d.quote)],
        [d.expression, record([d.circle, record(
          [d.x, record([d.unquote, cell(d.x)])], [d.y, number(50)], [d.radius, number(radius)],
        )])],
      )])],
    ),
    [d.program]: record([d.function, cell(d.do)], [d.expressions, { list: [x, ...dots].map((value) =>
      record([d.function, cell(d.fn)], [d.x, number(value)])) }]),
  });
};
const drawingSource = () => ({
  view: "document", stage: "value", source_path: { list: [
    record([drawingIds.key, cell(slots[0])]),
    record([drawingIds.follow, cell(drawingIds.document)]),
    record([drawingIds.key, cell(drawingIds.body)]),
  ] },
});

test("finale checks one original tree's height, shared paint, and a planted tree", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const d = drawingIds;
  const height = "cc32dd050a9351804e7a704b4d59e7ac";
  const paint = "cb04728f5e6a1d93a6790238b5f1ce4d";
  const rgb = "6c8a17cbe463186cc8b07e536ccffa6b";
  const trees = [[40, 60], [128, 90], [216, 72]];
  const scene = (list = trees, color = "548b64") => stacked([
    cell(d.program), cell(d.fn), record([d.drawing, record([d.programField, cell(d.program)])]),
  ], {
    [d.program]: record([d.expressions, { list: list.map(([x, h]) =>
      record([d.function, cell(d.fn)], [d.x, number(x)], [height, number(h)])) }]),
    [d.fn]: record([d.body, record([d.expressions, { list: [record(), record([d.function, cell(d.fill)], [paint, record([rgb, { blob: color }])])] }])]),
  });
  assert.deepEqual(completedSteps("forest", scene()), []);
  assert.deepEqual(completedSteps("forest", scene([[40, 100], [128, 90], [216, 72]], "cc7733")), ["height", "color"]);
  // Planting between trees shifts positions without counting as a height change.
  const planted = [[40, 60], [80, 50], [128, 90], [216, 72]];
  assert.deepEqual(completedSteps("forest", scene(planted)), ["plant"]);
  assert.deepEqual(completedSteps("forest", scene([[40, 60], [80, NaN], [128, 90], [216, 72]])), []);
  assert.deepEqual(completedSteps("forest", scene([[40, NaN], [128, 90], [216, 72]], "bad")), []);
  const state = scene(planted);
  removeSlot(state, 2);
  assert.deepEqual(completedSteps("forest", state), []);
});

test("drawing quests distinguish one call's argument, shared radius, a new dot, and source selection", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("drawing", drawingState()), []);
  assert.deepEqual(completedSteps("drawing", drawingState({ x: 80 })), ["argument"]);
  assert.deepEqual(completedSteps("drawing", drawingState({ radius: 36 })), ["body"]);
  assert.deepEqual(completedSteps("drawing", drawingState({ dots: [200, 160] })), ["add"]);
  assert.deepEqual(completedSteps("drawing", drawingState({ dots: [NaN, 160] })), []);
  assert.deepEqual(completedSteps("drawing", { ...drawingState(), selection: drawingSource() }), ["source"]);
  assert.deepEqual(completedSteps("drawing", { document: { root: null } }), []);
  for (const invalid of [NaN, Infinity]) {
    assert.deepEqual(completedSteps("drawing", drawingState({ x: invalid, radius: 36 })), ["body"]);
    assert.deepEqual(completedSteps("drawing", drawingState({ x: 80, radius: invalid })), []);
  }
  assert.deepEqual(completedSteps("drawing", drawingState({ x: 80, radius: 0 })), []);
});

test("drawing achievements require the visible function, calls, and drawing to remain connected", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  for (const disconnect of [
    (state) => { delete state.document.cells[drawingIds.fn]; },
    (state) => { delete state.document.cells[drawingIds.program]; },
    ...[0, 1, 2].map((index) => (state) => removeSlot(state, index)),
    (state) => { items(state)[2].record[0][1].record[0][1] = cell("other"); },
    (state) => { state.document.cells[drawingIds.program].record[1][1].list[0].record[0][1] = cell("other"); },
  ]) {
    const state = { ...drawingState({ x: 80, radius: 36 }), selection: drawingSource() };
    disconnect(state);
    assert.deepEqual(completedSteps("drawing", state), []);
  }
});

test("source achievement requires the fill call, not its parent, argument, or another view", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  for (const change of [
    (selection) => { selection.view = { pane: {} }; },
    (selection) => { selection.stage = "pending"; },
    (selection) => { selection.source_path = null; },
    (selection) => { selection.source_path.list.pop(); },
    (selection) => { selection.source_path.list.push(record([drawingIds.key, cell(drawingIds.shape)])); },
    (selection) => { selection.source_path.list[1] = record([drawingIds.follow, cell("library")]); },
  ]) {
    const selection = drawingSource();
    change(selection);
    assert.deepEqual(completedSteps("drawing", { ...drawingState(), selection }), []);
  }
});

test("drawing progress latches through undo and resets independently", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[4];
  send(4, drawingState());
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  send(4, drawingState({ x: 80 }));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 1);
  send(4, drawingState({ x: 80, radius: 36 }));
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 2);
  send(4, { ...drawingState({ x: 80, radius: 36, dots: [200, 160] }), selection: drawingSource() });
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  send(4, drawingState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(2, grapState(4, 5));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[2].tasks.filter(t => t.classes.has("completed")).length, 2);
});

const calculation = "623ada768cddc2cd6ea3d7870ca13c2c";
const views = (right, index, rest = []) => ({
  document: {
    root: record(...slots.map((slot) => [slot, cell(calculation)])),
    cells: { [calculation]: record(["acfc5e50881292518dab3cec77cf43ee", record(
      ["751fca4373debdd0b7e6eb73e08d684b", cell("201af445eb7e2c270bb5ead10b781fc1")],
      ["764f6afe17ba14e81f5ab61204be0bec", number(3)],
      ["4f53ff25390f58472d31a6142644dec2", number(right)],
    )]) },
  },
  selection: index === undefined ? null : { view: "document", stage: "value", path: { list: [
    record([drawingIds.key, cell(slots[index])]),
    record([drawingIds.follow, cell(drawingIds.document)]),
    record([drawingIds.key, cell("acfc5e50881292518dab3cec77cf43ee")]),
    record([drawingIds.key, cell("4f53ff25390f58472d31a6142644dec2")]),
    ...rest,
  ] } },
});

test("projection quests credit an edit to the view that made it, and finding the number in raw", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("projections", views(2)), []);
  assert.deepEqual(completedSteps("projections", views(4, 0), views(2, 0)), ["full"]);
  assert.deepEqual(completedSteps("projections", views(4, 1), views(2, 1)), ["plain"]);
  // Selecting without editing is not an edit.
  assert.deepEqual(completedSteps("projections", views(4, 0), views(4, 0)), []);
  assert.deepEqual(completedSteps("projections", views(2, 2)), ["raw"]);
  assert.deepEqual(completedSteps("projections", views(2, 2, [record([drawingIds.key, cell("ed11fde03b7c2c1ba2fccc3cdba5d561")])])), ["raw"]);
  const elsewhere = views(2, 2);
  elsewhere.selection.path.list.pop();
  assert.deepEqual(completedSteps("projections", elsewhere), []);
  const separate = views(2, 2);
  separate.document.root.record[1][1] = cell("other");
  assert.deepEqual(completedSteps("projections", separate), []);
  assert.deepEqual(completedSteps("projections", { document: { root: null } }), []);
});

test("projection progress latches and resets independently", async () => {
  const { exercises, send } = await lessonPage();
  const index = exercises.findIndex((exercise) => exercise.id === "projections");
  const page = exercises[index];
  send(index, views(2, 0));
  send(index, views(4, 0));
  send(index, views(5, 1));
  send(index, views(5, 2));
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
});

const world = "05bcece72b133b42300b7629fd3f402d";
const modelState = (name = "Mars", colors = ["red", "orange"], at) => ({
  document: {
    root: record([slots[0], cell(world)], [slots[1], cell(world)]),
    cells: { [world]: planet(text(name), number(2), colors).document.root },
  },
  selection: at === undefined ? null : { view: "document", stage: "value", path: { list: [
    record([drawingIds.key, cell(slots[at])]),
    record([drawingIds.follow, cell(drawingIds.document)]),
    record([drawingIds.key, cell("6655e4ba9e8ae76706056e0b8edf94f0")]),
  ] } },
});

test("model quests follow the planet through both views and find its key in the base one", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("model", modelState()), []);
  assert.deepEqual(completedSteps("model", modelState("Venus")), ["rename"]);
  assert.deepEqual(completedSteps("model", modelState("Mars", ["red", "blue", "orange"])), ["add"]);
  assert.deepEqual(completedSteps("model", modelState("Mars", ["red", "orange"], 0)), []);
  assert.deepEqual(completedSteps("model", modelState("Mars", ["red", "orange"], 1)), ["key"]);
  const separate = modelState("Venus");
  separate.document.root.record[1][1] = cell("other");
  assert.deepEqual(completedSteps("model", separate), []);
  assert.deepEqual(completedSteps("model", { document: { root: null } }), []);
});

test("model progress latches and resets independently", async () => {
  const { exercises, send } = await lessonPage();
  const index = exercises.findIndex((exercise) => exercise.id === "model");
  const page = exercises[index];
  send(index, modelState("Venus"));
  send(index, modelState("Venus", ["red", "blue", "orange"], 1));
  send(index, modelState());
  assert.ok(page.tasks.every(t => t.classes.has("completed")));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
});

test("story checks follow the forest's shared cells", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const recipe = "e93553d9cfad696bf6be0c3618c48858";
  const named = (name) => ({ document: { root: record(), cells: { [recipe]: record(
    ["02e562654d6d0828d3a7559e6f75fffe", text(name)],
  ) } } });
  assert.deepEqual(completedSteps("trunks", named("dot")), []);
  assert.deepEqual(completedSteps("trunks", named("tree")), ["rename"]);

  const growth = "ad17c2a7379b9ae003d76e833ec41be7";
  const calls = "46cea6ad72d7e838236fcf490ac3695e";
  const x = "2bd03b91a3ab695c54a71efafcb10115";
  const forest = (value, xs) => ({ document: { root: record(), cells: {
    [growth]: number(value),
    [calls]: record(["5fab151c006ae1487c28837f2003f43c", { list: xs.map((at) => record([x, number(at)])) }]),
  } } });
  assert.deepEqual(completedSteps("growth", forest(1, [40, 128, 216])), []);
  assert.deepEqual(completedSteps("growth", forest(0.5, [40, 128, 216])), ["shrink"]);
  assert.deepEqual(completedSteps("growth", forest(1, [40, 128, 216, 260])), ["plant"]);

  const call = "0245b1a17b29143ad4b1c25d57922a06";
  const count = "97da2489d387944d9a468658328e130d";
  const counted = (value) => ({ document: { root: record(), cells: { [call]: record([count, number(value)]) } } });
  assert.deepEqual(completedSteps("counted", counted(7)), []);
  assert.deepEqual(completedSteps("counted", counted(12)), ["count"]);
});
