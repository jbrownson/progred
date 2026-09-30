const fields = {
  planet: "6655e4ba9e8ae76706056e0b8edf94f0",
  moons: "6122302fdbe3db3624731717321b94a1",
  colors: "c785948e50c854a7c89bf6c4a4bdf4fc",
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
// The story page's cells; every version of its forest shares them.
const story = {
  program: "1aef3dfdbc27183833ef29bbecaccd95",
  recipe: "e93553d9cfad696bf6be0c3618c48858",
  calls: "46cea6ad72d7e838236fcf490ac3695e",
  x: "2bd03b91a3ab695c54a71efafcb10115",
  height: "f747026de922fb0f01823df377de5cb6",
  growth: "ad17c2a7379b9ae003d76e833ec41be7",
  count: "97da2489d387944d9a468658328e130d",
  forestCall: "0245b1a17b29143ad4b1c25d57922a06",
  baseSlot: "8ec6740fa565a2b3b0348132d030d03b",
  rect: "bb5e62c1b300b1e8c2484a0f4879a1fe",
  subtract: "08d1ebc7fd4ce62efec9671f73e9b645",
};
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

// The planet's colors: the list the values lesson removes from and restores.
function colors(state) {
  return field(state?.document?.root, fields.colors)?.list;
}

export function lessonProgress(lesson) {
  const completed = new Set();
  const seenLists = new Set();
  let previous;
  return (state) => {
    for (const step of completedSteps(lesson, state, previous)) completed.add(step);
    const list = colors(state);
    if (lesson === "values" && Array.isArray(list) && !completed.has("restore")) {
      const snapshot = JSON.stringify(list);
      // Undo may restore the whole text-editing run, not just the last empty item.
      if (completed.has("remove") && list.length > colors(previous)?.length
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

function selectedPath(state) {
  const selection = state?.selection;
  return selection?.view === "document" ? selection.path?.list : undefined;
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
  if (lesson in storySteps) return storySteps[lesson](state);
  if (lesson === "forest") {
    const height = "cc32dd050a9351804e7a704b4d59e7ac";
    const paint = "cb04728f5e6a1d93a6790238b5f1ce4d";
    const rgb = "6c8a17cbe463186cc8b07e536ccffa6b";
    // The original trees, by x: their starting heights.
    const planted = new Map([[40, 60], [128, 90], [216, 72]]);
    const program = field(root, slots[0])?.cell;
    const fn = field(root, slots[1])?.cell;
    const contents = state?.document?.cells;
    const calls = field(contents?.[program], drawing.expressions)?.list;
    const body = field(contents?.[fn], fields.body);
    const leaves = field(body, drawing.expressions)?.list?.[1];
    const color = bytes(field(field(leaves, paint), rgb));
    if (typeof fn !== "string" || typeof program !== "string"
        || field(field(field(root, slots[2]), drawing.drawing), drawing.program)?.cell !== program
        || !Array.isArray(calls) || calls.length < 3
        || !calls.every((call) => field(call, fields.function)?.cell === fn)
        || field(leaves, fields.function)?.cell !== drawing.fill) return [];
    const sizes = calls.map((call) => [number(field(call, drawing.x)), number(field(call, height))]);
    return [
      ...(sizes.some(([x, size]) => planted.has(x) && Number.isFinite(size) && size !== planted.get(x))
        ? ["height"] : []),
      ...(color?.length === 3 && color.some((byte, i) => byte !== [0x54, 0x8b, 0x64][i]) ? ["color"] : []),
      ...(calls.length >= 4 && sizes.every(([x, size]) => Number.isFinite(x) && Number.isFinite(size))
        ? ["plant"] : []),
    ];
  }
  if (lesson === "model") {
    const planet = field(root, slots[0])?.cell;
    const contents = state?.document?.cells?.[planet];
    const name = text(field(contents, fields.planet));
    const list = field(contents, fields.colors)?.list;
    const path = selectedPath(state);
    if (typeof planet !== "string" || field(root, slots[1])?.cell !== planet) return [];
    return [
      ...(name !== undefined && name !== "Mars" ? ["rename"] : []),
      ...(Array.isArray(list) && list.length > 2 && list.every((color) => text(color) !== undefined)
        ? ["add"] : []),
      ...(field(path?.[0], fields.key)?.cell === slots[1]
        && path.some((step) => field(step, fields.key)?.cell === fields.planet) ? ["key"] : []),
    ];
  }
  if (lesson === "projections") {
    const calculation = field(root, slots[0])?.cell;
    const right = (at) => number(field(field(at?.document?.cells?.[calculation], fields.evaluate), fields.right));
    if (typeof calculation !== "string"
        || !slots.every((slot) => field(root, slot)?.cell === calculation)) return [];
    const path = selectedPath(state);
    const inView = (index) => field(path?.[0], fields.key)?.cell === slots[index];
    const [now, before] = [right(state), right(previous)];
    // An edit counts in the view whose selection made it.
    const edited = Number.isFinite(now) && Number.isFinite(before) && now !== before;
    return [
      ...(edited && inView(0) ? ["full"] : []),
      ...(edited && inView(1) ? ["plain"] : []),
      ...(inView(2) && path.some((step) => field(step, fields.key)?.cell === fields.right) ? ["raw"] : []),
    ];
  }
  if (lesson === "values") {
    const planet = text(field(root, fields.planet));
    const moons = number(field(root, fields.moons));
    const list = colors(state);
    const before = colors(previous);
    return [
      ...(planet !== undefined && planet !== "Mars" ? ["rename"] : []),
      ...(Number.isFinite(moons) && moons !== 2 ? ["moons"] : []),
      ...(Array.isArray(list) && list.length > 2 ? ["add"] : []),
      ...(Array.isArray(list) && Array.isArray(before) && list.length < before.length ? ["remove"] : []),
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
    if (!addition || !multiplication || input(addition) !== input(multiplication)
        || !items.some((value) => value?.cell === input(addition))) return [];
    const x = number(contents[input(addition)]);
    const total = x + number(field(addition, fields.right));
    const product = x * number(field(multiplication, fields.right));
    return [
      ...(number(field(addition, fields.right)) !== 2 ? ["argument"] : []),
      ...(x !== 3 ? ["shared-edit"] : []),
      ...(total === product ? ["equal"] : []),
      ...(product === 100 ? ["hundred"] : []),
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
    const name = text(field(contents?.[parameter], fields.name));
    const firstCall = field(items[1], fields.evaluate);
    const first = field(firstCall, fields.function)?.cell === fn
      ? number(field(firstCall, parameter)) : undefined;
    return [
      ...(Number.isFinite(first) && first !== 3 ? ["argument"] : []),
      ...(factor !== 2 ? ["body"] : []),
      ...(name && name !== "x" ? ["rename"] : []),
      ...(calls.some((call) => field(field(call, parameter), fields.function)?.cell === fn) ? ["nest"] : []),
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
        || !Array.isArray(calls) || calls.length < 2
        || !calls.every((call) => field(call, fields.function)?.cell === fn)) return [];
    const positions = calls.map((call) => number(field(call, drawing.x)));
    const selection = state.selection;
    const path = selection?.source_path?.list;
    return [
      ...(Number.isFinite(positions[0]) && positions[0] !== 60 ? ["argument"] : []),
      ...(radius !== 24 ? ["body"] : []),
      ...(calls.length >= 3 && positions.every(Number.isFinite) ? ["add"] : []),
      ...(selection?.view === "document" && selection.stage === "value" && path?.length === 3
        && field(path[0], fields.key)?.cell === slots[0]
        && field(path[1], fields.follow)?.cell === fields.document
        && field(path[2], fields.key)?.cell === fields.body ? ["source"] : []),
    ];
  }
  return [];
}

function circle(fill) {
  return field(field(fill, drawing.shape), drawing.circle);
}

function sourceIn(state, slot, length) {
  const path = state?.selection?.source_path?.list;
  return path?.length === length && field(path[0], fields.key)?.cell === slot;
}

function selectedUnder(state, slot, key) {
  const path = selectedPath(state);
  return field(path?.[0], fields.key)?.cell === slot
    && path.some((step) => field(step, fields.key)?.cell === key);
}

function storyCalls(state) {
  return field(state?.document?.cells?.[story.calls], drawing.expressions)?.list;
}

const storySteps = {
  dot(state) {
    const x = number(field(circle(state?.document?.cells?.[story.program]), drawing.x));
    return [
      ...(sourceIn(state, slots[0], 2) ? ["pick"] : []),
      ...(Number.isFinite(x) && x !== 60 ? ["scrub"] : []),
    ];
  },
  copies(state) {
    const dots = field(state?.document?.cells?.[story.program], drawing.expressions)?.list;
    return Array.isArray(dots) && dots.length >= 2
      && dots.every((dot) => number(field(circle(dot), drawing.radius)) > 24) ? ["bigger"] : [];
  },
  recipe(state) {
    const body = field(state?.document?.cells?.[story.recipe], fields.body);
    return [
      ...(number(field(circle(body), drawing.radius)) > 24 ? ["bigger"] : []),
      ...(sourceIn(state, slots[0], 3) ? ["pick"] : []),
    ];
  },
  trunks(state) {
    const name = text(field(state?.document?.cells?.[story.recipe], fields.name));
    return name && name !== "dot" ? ["rename"] : [];
  },
  ladder(state) {
    return [
      ...(selectedUnder(state, slots[0], story.x) ? ["top"] : []),
      ...(selectedUnder(state, story.baseSlot, story.x) ? ["bottom"] : []),
    ];
  },
  heights(state) {
    const leaves = field(field(state?.document?.cells?.[story.recipe], fields.body), drawing.expressions)?.list?.[1];
    const y = field(circle(leaves), drawing.y);
    return [
      ...(field(y, fields.function)?.cell === story.subtract
        && Math.abs(number(field(y, fields.right)) - 90) <= 3 ? ["fit"] : []),
    ];
  },
  growth(state) {
    const growth = number(state?.document?.cells?.[story.growth]);
    const calls = storyCalls(state);
    return [
      ...(Number.isFinite(growth) && growth !== 1 ? ["shrink"] : []),
      ...(Array.isArray(calls) && calls.length > 3
        && calls.every((call) => Number.isFinite(number(field(call, story.x)))) ? ["plant"] : []),
    ];
  },
  counted(state) {
    const count = number(field(state?.document?.cells?.[story.forestCall], story.count));
    return [
      ...(Number.isFinite(count) && count !== 7 ? ["count"] : []),
      ...(state?.selection?.source_path?.list?.length > 2
        && field(state.selection.source_path.list[0], fields.key)?.cell === slots[0] ? ["pick"] : []),
    ];
  },
};
