const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { test } = require("node:test");

const root = path.resolve(__dirname, "..");
const bootstrap = fs.readFileSync(path.join(root, "web/index.html"), "utf8")
  .match(/<script type="module">([\s\S]*?)<\/script>/)[1]
  .replace(/^\s*import .*;$/gm, "");

async function start(search, { ok = true, parseError = false } = {}) {
  const calls = [];
  const messages = [];
  let onChange;
  const loading = { style: {} };
  const canvas = {};
  await vm.runInNewContext(`(async () => { ${bootstrap} })()`, {
    URL, URLSearchParams, Error, crossOriginIsolated: true,
    JSON,
    location: { search, href: `http://localhost/editor/${search}`, origin: "http://localhost" },
    window: { parent: { postMessage: (...args) => messages.push(structuredClone(args)) } },
    document: { querySelector: (selector) => selector === "#loading" ? loading : canvas },
    console: { info() {}, error() {} },
    fetch: async (url) => {
      calls.push(["fetch", url.href]);
      return { ok, status: ok ? 200 : 404, text: async () => "example source" };
    },
    init: async () => { calls.push(["init"]); },
    startWorker: async (...args) => { calls.push(["workers", args[4]]); },
    wasm: {
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
  return { calls, loading, messages, onChange };
}

test("standalone startup remains blank with its full menu and default workers", async () => {
  const { calls, loading } = await start("");
  assert.deepEqual(calls, [["init"], ["workers", undefined], ["editor", undefined, true, undefined, undefined]]);
  assert.equal(loading.style.display, "none");
});

test("embed fetches its document and supplies ordinary startup options", async () => {
  const { calls } = await start("?document=../lessons/values.gid&menu=hidden&threads=1");
  assert.deepEqual(calls, [
    ["fetch", "http://localhost/lessons/values.gid"], ["init"],
    ["workers", 1], ["editor", "example source", false, undefined, undefined],
  ]);
});

test("explicit library lists, including an empty list, reach editor startup unchanged", async () => {
  for (const libraries of ["3209ad5d23a0c8513f6bd76324a5cf60,eaaf309c36a65d2811083944da29aec9", ""]) {
    const { calls } = await start(`?libraries=${libraries}`);
    assert.equal(calls.find(([name]) => name === "editor")[4], libraries);
  }
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
  const state = { document: { root: { list: [] }, cells: [] }, selection: null };
  onChange(JSON.stringify(state));
  assert.equal(messages.length, 1);
  assert.deepEqual(messages[0], [{ type: "progred:change", channel: "values-0", state }, "http://localhost"]);
  assert.equal((await start("")).onChange, undefined);
});

async function lessonPage() {
  const { lessonProgress } = await import("./public/lesson-progress.mjs");
  const messageListeners = [];
  const exercises = ["values", "lists"].map((name) => {
    const listeners = {};
    const status = { textContent: "" };
    const progress = { textContent: "" };
    const tasks = (name === "values" ? ["greeting", "count"] : ["gap", "insert", "select-list", "remove", "restore"]).map((task) => {
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
      src: `./editor/?document=../lessons/${name}.gid&menu=hidden&threads=1&observe=${name}-0`,
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
    URL, lessonProgress,
    location: { href: "http://localhost/", origin: "http://localhost" },
    window: { addEventListener: (_, callback) => messageListeners.push(callback) },
    document: { querySelectorAll: () => exercises },
  });
  return {
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
  const resetCounts = [0, 0];
  for (const [index, exercise] of exercises.entries()) {
    const src = exercise.frame.src;
    Object.defineProperty(exercise.frame, "src", {
      get: () => src,
      set: (value) => {
        const url = new URL(value);
        assert.equal(url.searchParams.get("document"), `../lessons/${exercise.id}.gid`);
        assert.equal(url.searchParams.get("observe"), `${exercise.id}-1`);
        resetCounts[index]++;
      },
    });
  }
  exercises[0].listeners.click();
  assert.deepEqual(resetCounts, [1, 0]);
  assert.equal(exercises[1].status.textContent, "");
  exercises[0].listeners.load();
  assert.equal(exercises[0].status.textContent, "Example reset.");
  exercises[1].listeners.click();
  assert.deepEqual(resetCounts, [1, 1]);
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
