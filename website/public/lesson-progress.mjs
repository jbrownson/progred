const fields = {
  greeting: "60f0faf4-8244-5f9c-5d3c-d4c35d4ce902",
  count: "bf0ca177-3113-4521-964d-0f3aa582fcb1",
  utf8: "332529b8-ea83-a7ba-10fd-7f6d942e5016",
  f64: "ed11fde0-3b7c-2c1b-a2fc-cc3cdba5d561",
  element: "2eb44bbe-78bb-b0e9-6af4-a9cbf6e949e1",
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
  return [];
}
