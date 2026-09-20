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
};
const slots = [
  "9940ece2-7410-c72a-5308-a544890ccc71",
  "f717b766-d250-a7b8-6c5e-b842885c4417",
  "5e716c07-4908-49f0-72b4-e9017dd6230d",
];
const sharedCell = "f56d42a9-7558-ccc8-205f-b4019b878945";
const sum = "201af445-eb7e-2c27-0bb5-ead10b781fc1";
const multiply = "d6f384c4-39d9-d699-96d5-45df422efd79";

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
  return [];
}
