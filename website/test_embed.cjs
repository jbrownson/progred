const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

const root = path.resolve(__dirname, "..");
const bootstrap = fs.readFileSync(path.join(root, "web/index.html"), "utf8")
  .match(/<script type="module">([\s\S]*?)<\/script>/)[1]
  .replace(/^\s*import .*;$/gm, "");

async function start(search, { ok = true, parseError = false, platform = "Linux x86_64", storedTheme, storageDenied = false, duringInit } = {}) {
  const { commandIsMeta } = await import("../web/platform.mjs");
  const { isTheme, savedTheme } = await import("../web/theme.mjs");
  const calls = [];
  const messages = [];
  const focusEvents = [];
  const listeners = {};
  const themeChanges = [];
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
    JSON, commandIsMeta, isTheme, savedTheme, navigator: { platform },
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
  return { calls, loading, messages, onChange, focusEvents, listeners, parent, themeChanges,
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
    assert.deepEqual(canvasListeners, []);
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
  const { canvas, canvasListeners, documentElement, body } = await start("?wheel=page");
  assert.deepEqual(canvasListeners, [{ type: "wheel", options: { capture: true, passive: true } }]);
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
  assert.equal(received, 4);
});

test("invalid wheel configuration fails before starting workers or the editor", async () => {
  for (const search of ["?wheel=", "?wheel=unknown"]) {
    const { calls, loading } = await start(search);
    assert.deepEqual(calls, []);
    assert.equal(loading.style.display, "grid");
    assert.match(loading.textContent, /wheel must be 'editor' or 'page'/);
  }
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
    values: ["greeting", "count"],
    lists: ["gap", "insert", "select-list", "remove", "restore"],
    cells: ["shared-edit", "create", "link", "linked-edit"],
    grap: ["argument", "shared-edit"],
    functions: ["argument", "body", "rename"],
    drawing: ["argument", "body", "source"],
    forest: ["height", "color", "source"],
    create: ["number", "text", "list"],
  };
  const exercises = Object.keys(tasksByLesson).map((name) => {
    const listeners = {};
    const status = { textContent: "" };
    const progress = { textContent: `0 of ${tasksByLesson[name].length} steps complete` };
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
      src: `./editor/?document=../lessons/${name}.gid&menu=hidden&wheel=page&threads=1&observe=${name}-0`,
      contentWindow: {},
      getAttribute() { return this.src; },
      addEventListener: (event, callback) => { listeners[event] = callback; },
    };
    const button = { addEventListener: (event, callback) => { listeners[event] = callback; } };
    return {
      id: name, frame, status, progress, tasks, listeners,
      querySelectorAll: () => tasks,
      querySelector: (selector) => ({ iframe: frame, ".reset-status": status, ".reset": button, ".lesson-progress": progress })[selector],
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
        assert.equal(url.searchParams.get("observe"), `${exercise.id}-1`);
        assert.equal(url.searchParams.get("wheel"), "page");
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
const text = (value) => record(["332529b8-ea83-a7ba-10fd-7f6d942e5016", { blob: Buffer.from(value).toString("hex") }]);
const number = (value) => {
  const bytes = Buffer.alloc(8);
  bytes.writeDoubleLE(value);
  return record(["ed11fde0-3b7c-2c1b-a2fc-cc3cdba5d561", { blob: bytes.toString("hex") }]);
};
const values = (greeting, count) => ({ document: { root: record(
  ["60f0faf4-8244-5f9c-5d3c-d4c35d4ce902", greeting],
  ["bf0ca177-3113-4521-964d-0f3aa582fcb1", count],
) }, selection: null });
const fruit = (...names) => ({ document: { root: { list: names.map(text) } }, selection: null });
const sharedCell = "f56d42a9-7558-ccc8-205f-b4019b878945";
const cell = (id) => ({ cell: id });
const cells = (list, definitions) => ({ document: { root: { list }, cells: definitions }, selection: null });
const sharedPair = [cell(sharedCell), cell(sharedCell)];

test("value quests need changed, valid values rather than selections or missing fields", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("values", values(text("Hello, world!"), number(3))), []);
  assert.deepEqual(completedSteps("values", values(text("Hello, editor!"), number(5))), ["greeting", "count"]);
  assert.deepEqual(completedSteps("values", values(text(""), number(0))), ["greeting", "count"]);
  assert.deepEqual(completedSteps("values", values(record(), number(NaN))), []);
  assert.deepEqual(completedSteps("values", values(record(), number(Infinity))), []);
  assert.deepEqual(completedSteps("values", { document: { root: null } }), []);
});

test("list quests distinguish a pending gap, inserting peaches, and selecting the root", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const state = fruit("apples", "pears", "plums");
  assert.deepEqual(completedSteps("lists", state), []);
  const gap = { view: "document", stage: "pending", path: { list: [record(["2eb44bbe-78bb-b0e9-6af4-a9cbf6e949e1", record()])] } };
  assert.deepEqual(completedSteps("lists", { ...state, selection: gap }), ["gap"]);
  assert.deepEqual(completedSteps("lists", { ...state, selection: { ...gap, stage: "value" } }), []);
  assert.deepEqual(completedSteps("lists", fruit("apples", "peaches", "plums")), []);
  assert.deepEqual(completedSteps("lists", fruit("apples", "peaches", "pears", "plums")), ["insert"]);
  const root = { view: "document", stage: "value", path: { list: [] }, source_path: { list: [] } };
  assert.deepEqual(completedSteps("lists", { ...state, selection: root }), ["select-list"]);
  assert.deepEqual(completedSteps("lists", { ...state, selection: { ...root, view: { pane: {} } } }), []);
  assert.deepEqual(completedSteps("lists", { ...state, selection: { ...root, source_path: null } }), []);
});

test("achievements latch through undo, stay independent, and ignore stale or unrelated messages", async () => {
  const { exercises, send } = await lessonPage();
  const success = values(text("Changed"), number(10));
  const page = exercises[0];
  send(0, success, { origin: "https://elsewhere.example" });
  send(0, success, { source: {} });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].marker.textContent, "✓");
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  assert.equal(exercises[1].tasks[0].classes.has("completed"), false);
  send(0, values(text("Hello, world!"), number(3)));
  assert.equal(page.tasks[0].classes.has("completed"), true);
  page.listeners.click();
  assert.equal(page.tasks[0].classes.has("completed"), false);
  assert.equal(page.progress.textContent, "0 of 2 steps complete");
  send(0, success, { data: { type: "progred:change", channel: "values-0", state: success } });
  assert.equal(page.tasks[0].classes.has("completed"), false);
  send(0, success);
  assert.equal(page.tasks[0].classes.has("completed"), true);
});

test("removal observes a shrinking list, not empty text or a missing root", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const original = fruit("apples", "pears", "plums");
  assert.deepEqual(completedSteps("lists", original), []);
  assert.deepEqual(completedSteps("lists", fruit("apples", "", "plums"), original), []);
  assert.deepEqual(completedSteps("lists", fruit("apples", "plums"), original), ["remove"]);
  assert.deepEqual(completedSteps("lists", original, fruit("apples", "peaches", "pears", "plums")), ["remove"]);
  assert.deepEqual(completedSteps("lists", { document: { root: null } }, original), []);
  assert.deepEqual(completedSteps("lists", original, { document: { root: null } }), []);
  assert.deepEqual(completedSteps("lists", original, fruit("apples", "plums")), []);
});

test("removal and restoration have separate checkmarks and both reset", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[1];
  const removal = page.tasks.find((task) => task.dataset.task === "remove");
  const restoration = page.tasks.find((task) => task.dataset.task === "restore");
  send(1, { ...fruit("apples", "pears", "plums"), selection: {
    view: "document", stage: "pending",
    path: { list: [record(["2eb44bbe-78bb-b0e9-6af4-a9cbf6e949e1", record()])] },
  } });
  send(1, { ...fruit("apples", "peaches", "pears", "plums"), selection: {
    view: "document", stage: "value", path: { list: [] }, source_path: { list: [] },
  } });
  assert.equal(page.progress.textContent, "3 of 5 steps complete");
  send(1, fruit("apples", "peache", "pears", "plums"));
  send(1, fruit("apples", "", "pears", "plums"));
  send(1, fruit("apples", "pears", "plums"));
  assert.equal(removal.marker.textContent, "✓");
  assert.equal(restoration.classes.has("completed"), false);
  assert.equal(page.progress.textContent, "4 of 5 steps complete");
  send(1, fruit("apples", "peaches", "pears", "plums"));
  assert.equal(removal.marker.textContent, "✓");
  assert.equal(restoration.marker.textContent, "✓");
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  send(1, fruit("apples", "pears", "plums"));
  assert.equal(restoration.marker.textContent, "✓");
  page.listeners.click();
  send(1, fruit("apples", "pears", "plums"));
  assert.equal(removal.marker.textContent, 4);
  assert.equal(removal.classes.has("completed"), false);
  assert.equal(restoration.classes.has("completed"), false);
  assert.equal(page.progress.textContent, "0 of 5 steps complete");
  send(1, fruit("apples", "peaches", "pears", "plums"));
  assert.equal(restoration.classes.has("completed"), false);
});

test("restoration needs a previously seen list after removal, not an unrelated edit or insertion", async () => {
  const { lessonProgress } = await import("./public/lesson-progress.mjs");
  const observe = lessonProgress("lists");
  const original = fruit("apples", "pears", "plums");
  observe(original);
  observe(fruit("apples", "peaches", "plums"));
  assert.equal(observe(original).has("restore"), false);
  observe(fruit("apples", "plums"));
  assert.equal(observe(fruit("apples", "bananas", "plums")).has("restore"), false);
  observe({ document: { root: null } });
  assert.equal(observe(original).has("restore"), false);
  observe(fruit("apples", "plums"));
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
  const page = exercises[2];
  const original = cells(sharedPair, { [sharedCell]: number(7) });
  send(2, original);
  assert.equal(page.progress.textContent, "0 of 4 steps complete");
  send(2, cells(sharedPair, { [sharedCell]: number(8) }));
  const definitions = { [sharedCell]: number(8), new: number(11) };
  send(2, cells([...sharedPair, cell("new")], definitions));
  assert.equal(page.progress.textContent, "2 of 4 steps complete");
  const paired = [...sharedPair, cell("new"), cell("new")];
  send(2, cells(paired, definitions));
  assert.equal(page.progress.textContent, "3 of 4 steps complete");
  send(2, cells(paired, { ...definitions, new: number(12) }));
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  assert.equal(exercises[0].progress.textContent, "0 of 2 steps complete");
  assert.equal(exercises[1].progress.textContent, "0 of 5 steps complete");
  send(2, original);
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(0, values(text("Changed"), number(10)));
  page.listeners.click();
  assert.equal(page.progress.textContent, "0 of 4 steps complete");
  assert.equal(exercises[0].progress.textContent, "All steps complete. Keep experimenting!");
});

const grapInput = "4df73aa7-950e-afc6-5d50-d9c6ceca0721";
const slots = [
  "9940ece2-7410-c72a-5308-a544890ccc71",
  "f717b766-d250-a7b8-6c5e-b842885c4417",
  "5e716c07-4908-49f0-72b4-e9017dd6230d",
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
const grapState = (argument = 2, input = 3) => stacked(
  [cell(grapInput), ...["201af445-eb7e-2c27-0bb5-ead10b781fc1", "d6f384c4-39d9-d699-96d5-45df422efd79"].map((fn, index) => record(
    ["acfc5e50-8812-9251-8dab-3cec77cf43ee", record(
      ["751fca43-73de-bdd0-b7e6-eb73e08d684b", cell(fn)],
      ["764f6afe-17ba-14e8-1f5a-b61204be0bec", cell(grapInput)],
      ["4f53ff25-390f-5847-2d31-a6142644dec2", number(index === 0 ? argument : 2)],
    )],
  ))],
  { [grapInput]: number(input) },
);

test("Grap quests distinguish literal arguments from the shared input", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("grap", grapState()), []);
  assert.deepEqual(completedSteps("grap", grapState(4)), ["argument"]);
  assert.deepEqual(completedSteps("grap", grapState(2, 5)), ["shared-edit"]);
  assert.deepEqual(completedSteps("grap", grapState(4, 5)), ["argument", "shared-edit"]);
  assert.deepEqual(completedSteps("grap", grapState(NaN, 3)), []);
  assert.deepEqual(completedSteps("grap", grapState(2, Infinity)), []);
  assert.deepEqual(completedSteps("grap", grapState(4, NaN)), []);
  assert.deepEqual(completedSteps("grap", { document: { root: null } }), []);
  const missing = grapState(4, 5);
  removeSlot(missing, 2);
  assert.deepEqual(completedSteps("grap", missing), ["argument"]);
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
  const page = exercises[3];
  send(3, grapState());
  assert.equal(page.progress.textContent, "0 of 2 steps complete");
  send(3, grapState(4));
  assert.equal(page.progress.textContent, "1 of 2 steps complete");
  send(3, grapState(4, 5));
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  send(3, grapState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(0, values(text("Changed"), number(10)));
  page.listeners.click();
  assert.equal(page.progress.textContent, "0 of 2 steps complete");
  assert.equal(exercises[0].progress.textContent, "All steps complete. Keep experimenting!");
});

const scaleFunction = "27b02645-cf74-e4db-930f-7ace768a0aaf";
const scaleParameter = "a6ea8f49-8dc1-9d41-294f-9610ac9e8eed";
const functionsState = ({ argument = 3, factor = 2, name = "x" } = {}) => stacked(
  [cell(scaleFunction), ...[argument, 5].map((value) => record(
    ["acfc5e50-8812-9251-8dab-3cec77cf43ee", record(
      ["751fca43-73de-bdd0-b7e6-eb73e08d684b", cell(scaleFunction)],
      [scaleParameter, number(value)],
    )],
  ))],
  {
    [scaleFunction]: record(
      ["02e56265-4d6d-0828-d3a7-559e6f75fffe", text("scale")],
      ["195b378d-0d31-d90a-b0d7-366c15346b70", { list: [cell(scaleParameter)] }],
      ["98614386-6eda-2e2f-bf9a-b8484357a0c9", record(
        ["751fca43-73de-bdd0-b7e6-eb73e08d684b", cell("d6f384c4-39d9-d699-96d5-45df422efd79")],
        ["764f6afe-17ba-14e8-1f5a-b61204be0bec", cell(scaleParameter)],
        ["4f53ff25-390f-5847-2d31-a6142644dec2", number(factor)],
      )],
    ),
    [scaleParameter]: record(["02e56265-4d6d-0828-d3a7-559e6f75fffe", text(name)]),
  },
);

test("function quests distinguish arguments, the body, and a parameter rename", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("functions", functionsState()), []);
  assert.deepEqual(completedSteps("functions", functionsState({ argument: 4 })), ["argument"]);
  assert.deepEqual(completedSteps("functions", functionsState({ factor: 3 })), ["body"]);
  assert.deepEqual(completedSteps("functions", functionsState({ name: "amount" })), ["rename"]);
  assert.deepEqual(completedSteps("functions", functionsState({ argument: 4, factor: 3, name: "amount" })), ["argument", "body", "rename"]);
  for (const invalid of [NaN, Infinity]) {
    assert.deepEqual(completedSteps("functions", functionsState({ argument: invalid, factor: 3 })), []);
    assert.deepEqual(completedSteps("functions", functionsState({ factor: invalid, name: "amount" })), []);
  }
  assert.deepEqual(completedSteps("functions", { document: { root: null } }), []);
});

test("function achievements require the shown definition and its connected calls", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  for (const disconnect of [
    (state) => { delete state.document.cells[scaleFunction]; },
    (state) => { removeSlot(state, 0); },
    (state) => { removeSlot(state, 2); },
    (state) => { state.document.cells[scaleFunction].record[1][1].list.push(cell("other")); },
    (state) => { state.document.cells[scaleFunction].record[2][1].record[1][1] = cell("other"); },
    (state) => { items(state)[1].record[0][1].record[0][1] = cell("other"); },
  ]) {
    const state = functionsState({ argument: 4, factor: 3, name: "amount" });
    disconnect(state);
    assert.deepEqual(completedSteps("functions", state), []);
  }
  const renamedFunction = functionsState();
  renamedFunction.document.cells[scaleFunction].record[0][1] = text("amount");
  assert.deepEqual(completedSteps("functions", renamedFunction), []);
});

test("function progress latches through undo and resets only its own lesson", async () => {
  const { exercises, send } = await lessonPage();
  const page = exercises[4];
  send(4, functionsState());
  assert.equal(page.progress.textContent, "0 of 3 steps complete");
  send(4, functionsState({ argument: 4 }));
  assert.equal(page.progress.textContent, "1 of 3 steps complete");
  send(4, functionsState({ argument: 4, factor: 3 }));
  assert.equal(page.progress.textContent, "2 of 3 steps complete");
  send(4, functionsState({ argument: 4, factor: 3, name: "amount" }));
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  send(4, functionsState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(3, grapState(4, 5));
  page.listeners.click();
  assert.equal(page.progress.textContent, "0 of 3 steps complete");
  assert.equal(exercises[3].progress.textContent, "All steps complete. Keep experimenting!");
});

const drawingIds = {
  fn: "c1ae4281-e689-9b6a-268b-c154fe127b7b",
  program: "d4ebfe96-3093-8281-1fa2-1c889be44f53",
  function: "751fca43-73de-bdd0-b7e6-eb73e08d684b",
  params: "195b378d-0d31-d90a-b0d7-366c15346b70",
  body: "98614386-6eda-2e2f-bf9a-b8484357a0c9",
  drawing: "6889fa23-5b00-2be4-c8b1-06d5f31dafbf",
  programField: "bdf60781-0b27-4ad5-b02b-d409aef34b6a",
  fill: "1624dc97-3ec7-7902-03eb-8fd22c9d6d05",
  shape: "fcaaadef-1498-0397-cce9-5363fc5a54f9",
  circle: "e06a6d09-4c4f-75cd-6c1d-59ed6ed64e05",
  x: "415def0f-a0a9-ac40-dfba-5fca4d0f8876",
  y: "4e2dcde5-b1ab-1480-a2f1-26176dd148c7",
  radius: "6423c35e-07d7-a4ff-5361-27d1f1d8eb53",
  do: "b1fc4cb4-5c58-b1a6-62c4-31feef5bd140",
  expressions: "5fab151c-006a-e148-7c28-837f2003f43c",
  key: "f12dea12-c741-fe36-3127-50a264f3a235",
  follow: "33c6fb36-3ddc-6f13-fd05-4c68f7a37a98",
  document: "ef62fa62-f008-c1ec-a703-89571933aba0",
};
const drawingState = ({ x = 60, radius = 24 } = {}) => {
  const d = drawingIds;
  return stacked([
    cell(d.fn), cell(d.program), record([d.drawing, record([d.programField, cell(d.program)])]),
  ], {
    [d.fn]: record(
      [d.params, { list: [cell(d.x)] }],
      [d.body, record([d.function, cell(d.fill)], [d.shape, record([d.circle, record(
        [d.x, cell(d.x)], [d.y, number(50)], [d.radius, number(radius)],
      )])])],
    ),
    [d.program]: record([d.function, cell(d.do)], [d.expressions, { list: [x, 160].map((value) =>
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

test("creation steps require each value in its own slot, not merely a selection", async () => {
  const { completedSteps, lessonProgress } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("create", stacked([])), []);
  assert.deepEqual(completedSteps("create", stacked([number(42), text("hello"), { list: [number(7)] }])), ["number", "text", "list"]);
  assert.deepEqual(completedSteps("create", stacked([text("42"), number(42), { list: [text("7")] }])), []);
  const observe = lessonProgress("create");
  observe(stacked([number(42)]));
  assert.deepEqual([...observe(stacked([]))], ["number"]);
  const { exercises, send } = await lessonPage();
  send(7, stacked([number(42), text("hello"), { list: [number(7)] }]));
  assert.equal(exercises[7].progress.textContent, "All steps complete. Keep experimenting!");
  exercises[7].listeners.click();
  assert.equal(exercises[7].progress.textContent, "0 of 3 steps complete");
});

test("opening scene checks one tree's height, shared paint, and a source occurrence", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  const d = drawingIds;
  const height = "cc32dd05-0a93-5180-4e7a-704b4d59e7ac";
  const paint = "cb04728f-5e6a-1d93-a679-0238b5f1ce4d";
  const rgb = "6c8a17cb-e463-186c-c8b0-7e536ccffa6b";
  const scene = (h = 60, color = "548b64") => stacked([
    cell(d.program), cell(d.fn), record([d.drawing, record([d.programField, cell(d.program)])]),
  ], {
    [d.program]: record([d.expressions, { list: [h, 90, 72].map((h) => record([d.function, cell(d.fn)], [height, number(h)])) }]),
    [d.fn]: record([d.body, record([d.expressions, { list: [record(), record([d.function, cell(d.fill)], [paint, record([rgb, { blob: color }])])] }])]),
  });
  assert.deepEqual(completedSteps("forest", scene()), []);
  assert.deepEqual(completedSteps("forest", scene(100, "cc7733")), ["height", "color"]);
  assert.deepEqual(completedSteps("forest", scene(NaN, "bad")), []);
  const state = scene(100, "cc7733");
  state.selection = drawingSource();
  state.selection.source_path.list[0] = record([d.key, cell(slots[1])]);
  state.selection.source_path.list.push(record([d.key, cell(d.expressions)]), record(["2eb44bbe-78bb-b0e9-6af4-a9cbf6e949e1", record()]));
  assert.deepEqual(completedSteps("forest", state), ["height", "color", "source"]);
  state.selection.source_path.list.pop();
  assert.deepEqual(completedSteps("forest", state), ["height", "color"]);
  removeSlot(state, 2);
  assert.deepEqual(completedSteps("forest", state), []);
});

test("drawing quests distinguish one call's argument, shared radius, and source selection", async () => {
  const { completedSteps } = await import("./public/lesson-progress.mjs");
  assert.deepEqual(completedSteps("drawing", drawingState()), []);
  assert.deepEqual(completedSteps("drawing", drawingState({ x: 80 })), ["argument"]);
  assert.deepEqual(completedSteps("drawing", drawingState({ radius: 36 })), ["body"]);
  assert.deepEqual(completedSteps("drawing", { ...drawingState(), selection: drawingSource() }), ["source"]);
  assert.deepEqual(completedSteps("drawing", { document: { root: null } }), []);
  for (const invalid of [NaN, Infinity]) {
    assert.deepEqual(completedSteps("drawing", drawingState({ x: invalid, radius: 36 })), []);
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
  const page = exercises[5];
  send(5, drawingState());
  assert.equal(page.progress.textContent, "0 of 3 steps complete");
  send(5, drawingState({ x: 80 }));
  assert.equal(page.progress.textContent, "1 of 3 steps complete");
  send(5, drawingState({ x: 80, radius: 36 }));
  assert.equal(page.progress.textContent, "2 of 3 steps complete");
  send(5, { ...drawingState({ x: 80, radius: 36 }), selection: drawingSource() });
  assert.equal(page.progress.textContent, "All steps complete. Keep experimenting!");
  send(5, drawingState());
  assert.equal(page.tasks.every((task) => task.classes.has("completed")), true);
  send(3, grapState(4, 5));
  page.listeners.click();
  assert.equal(page.progress.textContent, "0 of 3 steps complete");
  assert.equal(exercises[3].progress.textContent, "All steps complete. Keep experimenting!");
});
