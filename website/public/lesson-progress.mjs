const fields = {
  greeting: "60f0faf4-8244-5f9c-5d3c-d4c35d4ce902",
  count: "bf0ca177-3113-4521-964d-0f3aa582fcb1",
  utf8: "332529b8-ea83-a7ba-10fd-7f6d942e5016",
  f64: "ed11fde0-3b7c-2c1b-a2fc-cc3cdba5d561",
  element: "2eb44bbe-78bb-b0e9-6af4-a9cbf6e949e1",
  evaluate: "acfc5e50-8812-9251-8dab-3cec77cf43ee",
  function: "751fca43-73de-bdd0-b7e6-eb73e08d684b",
  left: "764f6afe-17ba-14e8-1f5a-b61204be0bec",
  right: "4f53ff25-390f-5847-2d31-a6142644dec2",
  name: "02e56265-4d6d-0828-d3a7-559e6f75fffe",
  params: "195b378d-0d31-d90a-b0d7-366c15346b70",
  body: "98614386-6eda-2e2f-bf9a-b8484357a0c9",
  key: "f12dea12-c741-fe36-3127-50a264f3a235",
  follow: "33c6fb36-3ddc-6f13-fd05-4c68f7a37a98",
  document: "ef62fa62-f008-c1ec-a703-89571933aba0",
};
const slots = [
  "9940ece2-7410-c72a-5308-a544890ccc71",
  "f717b766-d250-a7b8-6c5e-b842885c4417",
  "5e716c07-4908-49f0-72b4-e9017dd6230d",
];
const sharedCell = "f56d42a9-7558-ccc8-205f-b4019b878945";
const sum = "201af445-eb7e-2c27-0bb5-ead10b781fc1";
const multiply = "d6f384c4-39d9-d699-96d5-45df422efd79";
const drawing = {
  drawing: "6889fa23-5b00-2be4-c8b1-06d5f31dafbf",
  program: "bdf60781-0b27-4ad5-b02b-d409aef34b6a",
  fill: "1624dc97-3ec7-7902-03eb-8fd22c9d6d05",
  shape: "fcaaadef-1498-0397-cce9-5363fc5a54f9",
  circle: "e06a6d09-4c4f-75cd-6c1d-59ed6ed64e05",
  x: "415def0f-a0a9-ac40-dfba-5fca4d0f8876",
  y: "4e2dcde5-b1ab-1480-a2f1-26176dd148c7",
  radius: "6423c35e-07d7-a4ff-5361-27d1f1d8eb53",
  do: "b1fc4cb4-5c58-b1a6-62c4-31feef5bd140",
  expressions: "5fab151c-006a-e148-7c28-837f2003f43c",
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
    const height = "cc32dd05-0a93-5180-4e7a-704b4d59e7ac";
    const paint = "cb04728f-5e6a-1d93-a679-0238b5f1ce4d";
    const rgb = "6c8a17cb-e463-186c-c8b0-7e536ccffa6b";
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
