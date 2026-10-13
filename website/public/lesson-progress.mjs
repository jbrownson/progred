const fields = {
  moons: "6122302fdbe3db3624731717321b94a1",
  utf8: "332529b8ea83a7ba10fd7f6d942e5016",
  f64: "ed11fde03b7c2c1ba2fccc3cdba5d561",
  evaluate: "acfc5e50881292518dab3cec77cf43ee",
  function: "751fca4373debdd0b7e6eb73e08d684b",
  right: "4f53ff25390f58472d31a6142644dec2",
  name: "02e562654d6d0828d3a7559e6f75fffe",
  body: "986143866eda2e2fbf9ab8484357a0c9",
  key: "f12dea12c741fe36312750a264f3a235",
  paint: "cb04728f5e6a1d93a6790238b5f1ce4d",
  rgb: "6c8a17cbe463186cc8b07e536ccffa6b",
};
const slots = [
  "9940ece27410c72a5308a544890ccc71",
  "f717b766d250a7b86c5eb842885c4417",
  "5e716c07490849f072b4e9017dd6230d",
];
const sharedCell = "f56d42a97558ccc8205fb4019b878945";
// The forest's cells; every version of it shares them.
const story = {
  program: "1aef3dfdbc27183833ef29bbecaccd95",
  recipe: "e93553d9cfad696bf6be0c3618c48858",
  calls: "46cea6ad72d7e838236fcf490ac3695e",
  x: "2bd03b91a3ab695c54a71efafcb10115",
  height: "f747026de922fb0f01823df377de5cb6",
  growth: "ad17c2a7379b9ae003d76e833ec41be7",
  count: "97da2489d387944d9a468658328e130d",
  forestCall: "0245b1a17b29143ad4b1c25d57922a06",
  subtract: "08d1ebc7fd4ce62efec9671f73e9b645",
};
// The cells the later sections take apart.
const raw = "8f041a1185f53ecf4e4219effb1d32bc";
const scaled = "865b04935589991010757df2e414ea0a";
const planet = {
  mars: "dd6a976689a3981ed726be04f86bcb29",
  drawing: "9088d2284bba446106a0bd1deebd0083",
};
const drawing = {
  shape: "fcaaadef14980397cce95363fc5a54f9",
  circle: "e06a6d094c4f75cd6c1d59ed6ed64e05",
  x: "415def0fa0a9ac40dfba5fca4d0f8876",
  y: "4e2dcde5b1ab1480a2f126176dd148c7",
  radius: "6423c35e07d7a4ff536127d1f1d8eb53",
  expressions: "5fab151c006ae1487c28837f2003f43c",
  expression: "ccc55b0eb63b9f564ea74436094d4014",
  unquote: "da48703c290e3b35d7353c38110bc953",
};

export function lessonProgress(lesson) {
  const completed = new Set();
  let previous;
  return (state) => {
    for (const step of completedSteps(lesson, state, previous)) completed.add(step);
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

// A word for a known trap, read from the same document and selection as the
// checks; most of the time there is none.
export function lessonHint(lesson, state) {
  return hints[lesson]?.(state);
}

const hints = {
  growth(state) {
    return storyCalls(state)?.some((call) => field(call, story.x) !== undefined
      && field(call, story.height) === undefined) ? "Give your tree a height too." : undefined;
  },
};

// Checks describe achievements; the page retains them until this exercise resets.
export function completedSteps(lesson, state, previous) {
  return steps[lesson]?.(state, previous) ?? [];
}

// A shape with computed coordinates is quoted, and those coordinates unquoted.
function circle(fill) {
  const shape = field(fill, drawing.shape);
  return field(field(shape, drawing.expression) ?? shape, drawing.circle);
}

function coordinate(circle, key) {
  const value = field(circle, key);
  return field(value, drawing.unquote) ?? value;
}

function sourceIn(state, slot, length) {
  const path = state?.selection?.source_path?.list;
  return path?.length === length && field(path[0], fields.key)?.cell === slot;
}

function storyCalls(state) {
  return field(state?.document?.cells?.[story.calls], drawing.expressions)?.list;
}

const steps = {
  dot(state) {
    const x = number(coordinate(circle(state?.document?.cells?.[story.program]), drawing.x));
    return [
      ...(sourceIn(state, slots[0], 2) ? ["pick"] : []),
      ...(Number.isFinite(x) && x !== 60 ? ["scrub"] : []),
    ];
  },
  copies(state) {
    const dots = field(state?.document?.cells?.[story.program], drawing.expressions)?.list;
    return Array.isArray(dots) && dots.length >= 2
      && dots.every((dot) => number(coordinate(circle(dot), drawing.radius)) > 24) ? ["bigger"] : [];
  },
  recipe(state) {
    const body = field(state?.document?.cells?.[story.recipe], fields.body);
    return [
      ...(number(coordinate(circle(body), drawing.radius)) > 24 ? ["bigger"] : []),
      ...(sourceIn(state, slots[0], 3) ? ["pick"] : []),
    ];
  },
  trunks(state) {
    const name = text(field(state?.document?.cells?.[story.recipe], fields.name));
    return name && name !== "dot" ? ["rename"] : [];
  },
  heights(state) {
    const leaves = field(field(state?.document?.cells?.[story.recipe], fields.body), drawing.expressions)?.list?.[1];
    const y = coordinate(circle(leaves), drawing.y);
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
        && calls.every((call) => Number.isFinite(number(field(call, story.x)))
          && field(call, story.height) !== undefined) ? ["plant"] : []),
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
  names(state) {
    const name = text(field(state?.document?.cells?.[story.x], fields.name));
    return name && name !== "x" ? ["rename"] : [];
  },
  cells(state, previous) {
    const root = state?.document?.root;
    if (!Array.isArray(root?.list)) return [];
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
  },
  bytes(state) {
    const data = bytes(state?.document?.cells?.[raw]);
    return [
      ...(data && state.document.cells[raw].blob !== "4f50454e" ? ["hex"] : []),
      ...(data?.length === 3 ? ["three"] : []),
    ];
  },
  unfinished(state) {
    const call = field(field(state?.document?.root, slots[2]), fields.evaluate);
    return Number.isFinite(number(field(call, scaled))) ? ["fill"] : [];
  },
  views(state) {
    const contents = state?.document?.cells;
    const moons = number(field(contents?.[planet.mars], fields.moons));
    const body = field(contents?.[planet.drawing], fields.body);
    const sphere = field(field(body, drawing.expression), drawing.expressions)?.list?.[0];
    const color = bytes(field(field(sphere, fields.paint), fields.rgb));
    return [
      ...(Number.isFinite(moons) && moons !== 2 ? ["moons"] : []),
      ...(color?.length === 3 && color.some((byte, i) => byte !== [0xc1, 0x44, 0x0e][i]) ? ["color"] : []),
    ];
  },
};
