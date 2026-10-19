const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

const root = path.resolve(__dirname, "..");
const bootstrap = fs.readFileSync(path.join(root, "web/index.html"), "utf8")
  .match(/<script type="module">([\s\S]*?)<\/script>/)[1]
  .replace(/^\s*import .*;$/gm, "");

async function start(search, { ok = true, parseError = false, platform = "Linux x86_64", storedTheme, storageDenied = false, duringInit, capturesScroll = () => false, isolated = true } = {}) {
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
  const wasm = {
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
  };
  await vm.runInNewContext(`(async () => { ${bootstrap} })()`, {
    URL, URLSearchParams, Error, crossOriginIsolated: isolated,
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
    loadEditor: async (threaded) => {
      calls.push(["init", threaded ? "pkg" : "pkg-fallback"]);
      duringInit?.(listeners, parent);
      return { wasm, url: "http://localhost/editor/pkg/progred.js" };
    },
    startWorker: async (...args) => { calls.push(["workers", args[4]]); },
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
  assert.deepEqual(calls, [["init", "pkg"], ["workers", undefined], ["editor", undefined, true, undefined, undefined, undefined, false, "light"]]);
  assert.equal(loading.style.display, "none");
});

test("a page without cross-origin isolation starts the fallback build without workers", async () => {
  const { calls, loading } = await start("?threads=1", { isolated: false });
  assert.deepEqual(calls, [["init", "pkg-fallback"], ["editor", undefined, true, undefined, undefined, undefined, false, "light"]]);
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
  for (const search of ["", "?wheel=editor", "?menu=hidden&document=../lessons/cells.gid"]) {
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
  const { calls } = await start("?document=../lessons/cells.gid&menu=hidden&threads=1");
  assert.deepEqual(calls, [
    ["fetch", "http://localhost/lessons/cells.gid"], ["init", "pkg"],
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
    const { calls, loading } = await start("?document=../lessons/cells.gid", options);
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
  const { lessonHint, lessonProgress } = await import("./public/lesson-progress.mjs");
  const messageListeners = [];
  const tasksByLesson = {
    names: ["rename"],
    cells: ["shared-edit", "create", "link", "linked-edit"],
    bytes: ["hex", "three"],
    growth: ["shrink", "plant"],
    "growing-forest": [],
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
    const hint = name === "growth" ? { textContent: "", hidden: true } : undefined;
    return {
      id: name, frame, status, tasks, listeners, hint,
      querySelectorAll: () => tasks,
      querySelector: (selector) => ({ iframe: frame, ".reset-status": status, ".reset": button, ".hint": hint })[selector],
    };
  });
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "public/lessons.js"), "utf8").replace(/^import .*;$/gm, ""), {
    URL, lessonHint, lessonProgress, commandIsMeta, navigator: { platform },
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
const sharedCell = "f56d42a97558ccc8205fb4019b878945";
const cell = (id) => ({ cell: id });
const cells = (list, definitions) => ({ document: { root: { list }, cells: definitions }, selection: null });
const sharedPair = [cell(sharedCell), cell(sharedCell)];
// The forest's x parameter, by the name it wears.
const renamed = (name) => ({ document: { root: record(), cells: {
  "2bd03b91a3ab695c54a71efafcb10115": record(["02e562654d6d0828d3a7559e6f75fffe", text(name)]),
} }, selection: null });

test("achievements latch through undo, stay independent, and ignore stale or unrelated messages", async () => {
  const { exercises, send } = await lessonPage();
  const success = renamed("position");
  const page = exercises[0];
  send(0, success, { origin: "https://elsewhere.example" });
  send(0, success, { source: {} });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].marker.textContent, "✓");
  assert.equal(exercises[1].tasks[0].classes.has("completed"), false);
  send(0, renamed("x"));
  assert.equal(page.tasks[0].classes.has("completed"), true);
  page.listeners.click();
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success, { data: { type: "progred:change", channel: "names-0", state: success } });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].classes.has("completed"), true);
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
  send(0, renamed("position"));
  page.listeners.click();
  assert.equal(page.tasks.filter(t => t.classes.has("completed")).length, 0);
  assert.equal(exercises[0].tasks.filter(t => t.classes.has("completed")).length, 1);
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
  const height = "f747026de922fb0f01823df377de5cb6";
  const forest = (value, xs) => ({ document: { root: record(), cells: {
    [growth]: number(value),
    [calls]: record(["5fab151c006ae1487c28837f2003f43c", { list: xs.map((at) => record([x, number(at)], [height, number(60)])) }]),
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

test("picking finds the code stored in the cell the first slot holds", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const first = "9940ece27410c72a5308a544890ccc71";
  const steps = (...keys) => ({ list: keys.map((key) => record([key, { cell: key }])) });
  const picked = (cell, path) => ({
    document: { root: record([first, { cell: "program" }]), cells: {} },
    selection: { source: { cell, library: null, path } },
  });
  assert.deepEqual(completedSteps("dot", picked("program", steps())), ["pick"]);
  assert.deepEqual(completedSteps("dot", picked("program", steps("body"))), []);
  assert.deepEqual(completedSteps("dot", picked("elsewhere", steps())), []);
  assert.deepEqual(completedSteps("recipe", picked("program", steps("body"))), ["pick"]);
  assert.deepEqual(completedSteps("counted", picked("program", steps("body", "x"))), ["pick"]);
  assert.deepEqual(completedSteps("counted", picked("program", steps())), []);
  assert.deepEqual(completedSteps("counted", {
    document: { root: record([first, { cell: "program" }]), cells: {} },
    selection: { source: { path: steps(first) } },
  }), []);
});

const story = {
  x: "2bd03b91a3ab695c54a71efafcb10115",
  height: "f747026de922fb0f01823df377de5cb6",
  growth: "ad17c2a7379b9ae003d76e833ec41be7",
  calls: "46cea6ad72d7e838236fcf490ac3695e",
  expressions: "5fab151c006ae1487c28837f2003f43c",
};
const planted = (...trees) => ({
  document: { root: null, cells: {
    [story.growth]: number(1),
    [story.calls]: record([story.expressions, { list: trees.map(([x, height]) => record(
      ...(x === undefined ? [] : [[story.x, number(x)]]),
      ...(height === undefined ? [] : [[story.height, number(height)]]),
    )) }]),
  } },
  selection: null,
});

test("a planted tree needs both an x and a height", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const three = [[40, 60], [128, 90], [216, 72]];
  assert.deepEqual(completedSteps("growth", planted(...three)), []);
  assert.deepEqual(completedSteps("growth", planted(...three, [172, 80])), ["plant"]);
  assert.deepEqual(completedSteps("growth", planted(...three, [172, undefined])), []);
  assert.deepEqual(completedSteps("growth", planted(...three, [undefined, 80])), []);
});

test("hints name the traps a reader falls into, and nothing otherwise", async () => {
  const { lessonHint } = await import("./public/lesson-progress.mjs");
  const three = [[40, 60], [128, 90], [216, 72]];
  assert.equal(lessonHint("growth", planted(...three, [172, undefined])), "Give your tree a height too.");
  assert.equal(lessonHint("growth", planted(...three, [172, 80])), undefined);
  assert.equal(lessonHint("names", renamed("position")), undefined);
});

test("a section's hint shows while its trap holds and clears on reset", async () => {
  const page = await lessonPage();
  const index = page.exercises.findIndex((exercise) => exercise.id === "growth");
  const { hint, listeners } = page.exercises[index];
  const three = [[40, 60], [128, 90], [216, 72]];
  page.send(index, planted(...three, [172, undefined]));
  assert.equal(hint.hidden, false);
  assert.equal(hint.textContent, "Give your tree a height too.");
  page.send(index, planted(...three, [172, 80]));
  assert.equal(hint.hidden, true);
  page.send(index, planted(...three, [172, undefined]));
  listeners.click();
  assert.equal(hint.hidden, true);
  assert.equal(hint.textContent, "");
});

test("taking the forest apart ticks a renamed x, rewritten bytes, a filled call, and a changed planet", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("names", renamed("x")), []);
  assert.deepEqual(completedSteps("names", renamed("")), []);
  assert.deepEqual(completedSteps("names", renamed("position")), ["rename"]);

  const blob = (hex) => ({ document: { root: null, cells: { "8f041a1185f53ecf4e4219effb1d32bc": { blob: hex } } } });
  assert.deepEqual(completedSteps("bytes", blob("4f50454e")), []);
  assert.deepEqual(completedSteps("bytes", blob("4d617273")), ["hex"]);
  assert.deepEqual(completedSteps("bytes", blob("4f5045")), ["hex", "three"]);
  assert.deepEqual(completedSteps("bytes", { document: { root: null, cells: {} } }), []);

  const call = (...argument) => ({ document: { root: record(["5e716c07490849f072b4e9017dd6230d", record(
    ["acfc5e50881292518dab3cec77cf43ee", record(["751fca4373debdd0b7e6eb73e08d684b", cell("f5464e66a508bc1b2daf5fe7371449fd")],
      ...argument.map((value) => ["865b04935589991010757df2e414ea0a", value]))],
  )]) } });
  assert.deepEqual(completedSteps("unfinished", call()), []);
  assert.deepEqual(completedSteps("unfinished", call(text("4"))), []);
  assert.deepEqual(completedSteps("unfinished", call(number(4))), ["fill"]);

  const planet = (moons, rgb) => ({ document: { root: null, cells: {
    dd6a976689a3981ed726be04f86bcb29: record(["6122302fdbe3db3624731717321b94a1", number(moons)]),
    "9088d2284bba446106a0bd1deebd0083": record(["986143866eda2e2fbf9ab8484357a0c9", record(
      ["ccc55b0eb63b9f564ea74436094d4014", record(["5fab151c006ae1487c28837f2003f43c", { list: [
        record(["cb04728f5e6a1d93a6790238b5f1ce4d", record(["6c8a17cbe463186cc8b07e536ccffa6b", { blob: rgb }])]),
      ] }])],
    )]),
  } } });
  assert.deepEqual(completedSteps("views", planet(2, "c1440e")), []);
  assert.deepEqual(completedSteps("views", planet(5, "c1440e")), ["moons"]);
  assert.deepEqual(completedSteps("views", planet(2, "3a6ea5")), ["color"]);
  assert.deepEqual(completedSteps("views", planet(NaN, "c1440e00")), []);
});
