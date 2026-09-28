const fields = {
  greeting: "60f0faf482445f9c5d3cd4c35d4ce902",
  count: "bf0ca17731134521964d0f3aa582fcb1",
  utf8: "332529b8ea83a7ba10fd7f6d942e5016",
  f64: "ed11fde03b7c2c1ba2fccc3cdba5d561",
  element: "2eb44bbe78bbb0e96af4a9cbf6e949e1",
  evaluate: "acfc5e50881292518dab3cec77cf43ee",
  function: "751fca4373debdd0b7e6eb73e08d684b",
  left: "764f6afe17ba14e81f5ab61204be0bec",
  right: "4f53ff25390f58472d31a6142644dec2",
  name: "02e562654d6d0828d3a7559e6f75fffe",
  params: "195b378d0d31d90ab0d7366c15346b70",
  body: "986143866eda2e2fbf9ab8484357a0c9",
  key: "f12dea12c741fe36312750a264f3a235",
  follow: "33c6fb363ddc6f13fd054c68f7a37a98",
  document: "ef62fa62f008c1eca70389571933aba0",
};
const slots = [
  "9940ece27410c72a5308a544890ccc71",
  "f717b766d250a7b86c5eb842885c4417",
  "5e716c07490849f072b4e9017dd6230d",
];
const sharedCell = "f56d42a97558ccc8205fb4019b878945";
const sum = "201af445eb7e2c270bb5ead10b781fc1";
const multiply = "d6f384c439d9d69996d545df422efd79";
const drawing = {
  drawing: "6889fa235b002be4c8b106d5f31dafbf",
  program: "bdf607810b274ad5b02bd409aef34b6a",
  fill: "1624dc973ec7790203eb8fd22c9d6d05",
  shape: "fcaaadef14980397cce95363fc5a54f9",
  circle: "e06a6d094c4f75cd6c1d59ed6ed64e05",
  x: "415def0fa0a9ac40dfba5fca4d0f8876",
  y: "4e2dcde5b1ab1480a2f126176dd148c7",
  radius: "6423c35e07d7a4ff536127d1f1d8eb53",
  do: "b1fc4cb45c58b1a662c431feef5bd140",
  expressions: "5fab151c006ae1487c28837f2003f43c",
};

export function lessonProgress(lesson) {
  const completed = new Set();
  const seenLists = new Set();
  let previous;
  return (state) => {
    for (const step of completedSteps(lesson, state, previous)) completed.add(step);
    const list = state?.document?.root?.list;
    if (lesson === "lists" && Array.isArray(list) && !completed.has("restore")) {
      const snapshot = JSON.stringify(list);
      // Undo may restore the whole text-editing run, not just the last empty item.
      if (completed.has("remove") && list.length > previous?.document?.root?.list?.length
          && seenLists.has(snapshot)) {
        completed.add("restore");
        seenLists.clear();
      } else {
        seenLists.add(snapshot);
      }
    }
    previous = state;
    return completed;
  };
}

function field(value, id) {
  return value?.record?.find(([key]) => key === id)?.[1];
}

function bytes(value) {
  const hex = value?.blob;
  return typeof hex === "string" && /^(?:[0-9a-f]{2})*$/.test(hex)
    ? Uint8Array.from(hex.match(/../g) ?? [], (byte) => parseInt(byte, 16)) : undefined;
}

function text(value) {
  const data = bytes(field(value, fields.utf8));
  try {
    return data && new TextDecoder("utf-8", { fatal: true }).decode(data);
  } catch {
    return undefined;
  }
}

function number(value) {
  const data = bytes(field(value, fields.f64));
  return data?.length === 8 ? new DataView(data.buffer).getFloat64(0, true) : undefined;
}

function references(list) {
  const counts = new Map();
  for (const value of list ?? []) {
    if (typeof value?.cell === "string") {
      counts.set(value.cell, (counts.get(value.cell) ?? 0) + 1);
    }
  }
  return counts;
}

// Checks describe achievements; the page retains them until this exercise resets.
export function completedSteps(lesson, state, previous) {
  const root = state?.document?.root;
  if (lesson === "create") {
    return [
      ...(number(field(root, slots[0])) === 42 ? ["number"] : []),
      ...(text(field(root, slots[1])) === "hello" ? ["text"] : []),
      ...(field(root, slots[2])?.list?.some((value) => number(value) === 7) ? ["list"] : []),
    ];
  }
  if (lesson === "forest") {
    const height = "cc32dd050a9351804e7a704b4d59e7ac";
    const paint = "cb04728f5e6a1d93a6790238b5f1ce4d";
    const rgb = "6c8a17cbe463186cc8b07e536ccffa6b";
    const program = field(root, slots[0])?.cell;
    const fn = field(root, slots[1])?.cell;
    const contents = state?.document?.cells;
    const calls = field(contents?.[program], drawing.expressions)?.list;
    const body = field(contents?.[fn], fields.body);
    const leaves = field(body, drawing.expressions)?.list?.[1];
    const color = bytes(field(field(leaves, paint), rgb));
    if (typeof fn !== "string" || typeof program !== "string"
        || field(field(field(root, slots[2]), drawing.drawing), drawing.program)?.cell !== program
        || calls?.length !== 3
        || !calls.every((call) => field(call, fields.function)?.cell === fn)
        || field(leaves, fields.function)?.cell !== drawing.fill) return [];
    const selection = state.selection;
    const path = selection?.source_path?.list;
    return [
      ...(number(field(calls[0], height)) === 100
        && number(field(calls[1], height)) === 90 && number(field(calls[2], height)) === 72 ? ["height"] : []),
      ...(color?.length === 3 && color.some((byte, i) => byte !== [0x54, 0x8b, 0x64][i]) ? ["color"] : []),
      ...(selection?.view === "document" && selection.stage === "value" && path?.length === 5
        && field(path[0], fields.key)?.cell === slots[1]
        && field(path[1], fields.follow)?.cell === fields.document
        && field(path[2], fields.key)?.cell === fields.body
        && field(path[3], fields.key)?.cell === drawing.expressions
        && field(path[4], fields.element) ? ["source"] : []),
    ];
  }
  if (lesson === "values") {
    const greeting = text(field(root, fields.greeting));
    const count = number(field(root, fields.count));
    return [
      ...(greeting !== undefined && greeting !== "Hello, world!" ? ["greeting"] : []),
      ...(Number.isFinite(count) && count !== 3 ? ["count"] : []),
    ];
  }
  if (lesson === "lists" && Array.isArray(root?.list)) {
    const selection = state.selection;
    const path = selection?.path?.list;
    const inDocument = selection?.view === "document";
    return [
      ...(inDocument && selection.stage === "pending" && path?.length === 1
        && field(path[0], fields.element) ? ["gap"] : []),
      ...(root.list.length > 3 && root.list.some((value) => text(value) === "peaches") ? ["insert"] : []),
      ...(inDocument && selection.stage === "value" && path?.length === 0
        && selection.source_path?.list?.length === 0 ? ["select-list"] : []),
      ...(Array.isArray(previous?.document?.root?.list)
        && root.list.length < previous.document.root.list.length ? ["remove"] : []),
    ];
  }
  if (lesson === "cells" && Array.isArray(root?.list)) {
    const counts = references(root.list);
    const before = references(previous?.document?.root?.list);
    const contents = state.document.cells;
    const original = number(contents?.[sharedCell]);
    const newCells = [...counts].filter(([id]) => id !== sharedCell
      && Number.isFinite(number(contents?.[id])));
    return [
      ...(counts.get(sharedCell) >= 2 && Number.isFinite(original) && original !== 7 ? ["shared-edit"] : []),
      ...(newCells.some(([id]) => number(contents[id]) === 11) ? ["create"] : []),
      ...(newCells.some(([, count]) => count >= 2) ? ["link"] : []),
      ...(newCells.some(([id, count]) => {
        const old = number(previous?.document?.cells?.[id]);
        return count >= 2 && before.get(id) >= 2 && Number.isFinite(old)
          && old !== number(contents[id]);
      }) ? ["linked-edit"] : []),
    ];
  }
  if (lesson === "grap") {
    const contents = state?.document?.cells;
    const input = (call) => field(call, fields.left)?.cell;
    const items = slots.map((key) => field(root, key));
    const calls = items.map((item) => field(item, fields.evaluate));
    const call = (fn) => calls.find((value) => field(value, fields.function)?.cell === fn
      && typeof input(value) === "string"
      && Number.isFinite(number(contents?.[input(value)]))
      && Number.isFinite(number(field(value, fields.right))));
    const addition = call(sum);
    const multiplication = call(multiply);
    return [
      ...(number(field(addition, fields.right)) === 4 ? ["argument"] : []),
      ...(addition && multiplication && input(addition) === input(multiplication)
        && items.some((value) => value?.cell === input(addition))
        && number(contents[input(addition)]) === 5 ? ["shared-edit"] : []),
    ];
  }
  if (lesson === "functions") {
    const contents = state?.document?.cells;
    const items = slots.map((key) => field(root, key));
    const fn = items.find((value) => typeof value?.cell === "string"
      && Array.isArray(field(contents?.[value.cell], fields.params)?.list))?.cell;
    const definition = contents?.[fn];
    const parameters = field(definition, fields.params)?.list;
    const parameter = parameters?.length === 1 && parameters[0]?.cell;
    const body = field(definition, fields.body);
    const factor = number(field(body, fields.right));
    if (typeof parameter !== "string" || field(body, fields.function)?.cell !== multiply
        || field(body, fields.left)?.cell !== parameter || !Number.isFinite(factor)) return [];
    const calls = items.map((item) => field(item, fields.evaluate))
      .filter((call) => field(call, fields.function)?.cell === fn);
    const inputs = calls.map((call) => number(field(call, parameter)));
    if (inputs.length !== 2 || !inputs.every(Number.isFinite)) return [];
    return [
      ...(inputs.includes(4) && inputs.includes(5) ? ["argument"] : []),
      ...(factor === 3 ? ["body"] : []),
      ...(text(field(contents?.[parameter], fields.name)) === "amount" ? ["rename"] : []),
    ];
  }
  if (lesson === "drawing") {
    const contents = state?.document?.cells;
    const fn = field(root, slots[0])?.cell;
    const program = field(root, slots[1])?.cell;
    const definition = contents?.[fn];
    const parameters = field(definition, fields.params)?.list;
    const body = field(definition, fields.body);
    const circle = field(field(body, drawing.shape), drawing.circle);
    const radius = number(field(circle, drawing.radius));
    const calls = field(contents?.[program], drawing.expressions)?.list;
    const canvas = field(field(root, slots[2]), drawing.drawing);
    if (typeof fn !== "string" || typeof program !== "string"
        || field(canvas, drawing.program)?.cell !== program
        || parameters?.length !== 1 || parameters[0]?.cell !== drawing.x
        || field(body, fields.function)?.cell !== drawing.fill
        || field(circle, drawing.x)?.cell !== drawing.x
        || !Number.isFinite(radius) || radius <= 0
        || !Number.isFinite(number(field(circle, drawing.y)))
        || field(contents?.[program], fields.function)?.cell !== drawing.do
        || calls?.length !== 2
        || !calls.every((call) => field(call, fields.function)?.cell === fn
          && Number.isFinite(number(field(call, drawing.x))))) return [];
    const selection = state.selection;
    const path = selection?.source_path?.list;
    return [
      ...(number(field(calls[0], drawing.x)) === 80
        && number(field(calls[1], drawing.x)) === 160 ? ["argument"] : []),
      ...(radius === 36 ? ["body"] : []),
      ...(selection?.view === "document" && selection.stage === "value" && path?.length === 3
        && field(path[0], fields.key)?.cell === slots[0]
        && field(path[1], fields.follow)?.cell === fields.document
        && field(path[2], fields.key)?.cell === fields.body ? ["source"] : []),
    ];
  }
  return [];
}
