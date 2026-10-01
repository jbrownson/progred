// Peel a live program's projections away, top to bottom. Each layer is a set
// of libraries that stop drawing while their definitions stay loaded, so keys
// and functions keep their names. Names go last.
const layers = [
  {
    id: "pictures",
    label: "Pictures and sliders",
    names: "layout, presentation, controls",
    libraries: ["fb2a4dac87512d69448650bc0e29dc80", "d22b834154d60b1df228f9bb4d3c13de", "666ba40b81028e32c78bdc1665d6c3a9"],
    off: "The picture was a view. With the layout, presentation, and controls libraries no longer drawing, you see what they drew from: a record asking to <code>render</code> the program. Those libraries are still loaded, which is why its keys keep their names. Nothing in the document changed.",
  },
  {
    id: "colors",
    label: "Colors",
    names: "color",
    libraries: ["25d0e2034b4bd65bebb4811d65eab89c"],
    off: "The swatches were a view too. A color is three bytes under the color library's <code>rgb</code> key: <code>#399b75</code> is <code>{rgb: 0x399b75}</code>.",
  },
  {
    id: "calculations",
    label: "Calculations",
    names: "grap, control, sequence, absent",
    libraries: ["f7735b90f6826b25c350a8fd83af8c47", "ec17915df2d42377574dc90f22500fe2", "0ad8124ba821acd5fbf2c868371e1492", "873c68ac371dbbb98a4f198546d60241"],
    off: "Grap, the language, is a library, and so is the way it draws code. With that projection off, a call is the record it always was: a <code>function</code> key and its arguments filed under their parameters. Names in parentheses, like <code>(forest)</code> and <code>(slider)</code>, point at cells defined elsewhere, in this document or a library.",
  },
  {
    id: "numbers",
    label: "Numbers",
    names: "f64",
    libraries: ["1fdb573a2c56a7063546c195318214bc"],
    off: "Numbers were the f64 library reading the eight bytes filed under its <code>f64</code> key, the key the little <strong>f64</strong> names. Without that projection, <code>7<sub class=\"tag\">f64</sub></code> is <code>{f64: 0x0000000000001c40}</code>: the eight bytes of 7.0, least significant first.",
  },
  {
    id: "names",
    label: "Names",
    names: "stored with each key and cell",
    libraries: [],
    off: "Names are stored facts too, like <code>{name: \"tree count\"}</code> on the cell they name, and every label above was drawn from them. Without names, each key and cell shows its identity, a short code like <code>…5d561</code> for <code>f64</code>, and text shows as its bytes. This is what's actually stored: records <code>{ }</code>, lists <code>[ ]</code>, bytes <code>0x…</code>, and cells <code>( )</code>. Every layer you switched off only changed how it's drawn. Switch them back on, in any order.",
  },
];

const allOn = (count) => `All ${count} of this editor's libraries are drawing, grouped into the layers above, with names labeling what they draw. Each mark on the screen, from the trees to the little <strong>f64</strong> tags, comes from one of these layers. Move the slider, or switch layers off one at a time.`;

const editor = document.querySelector("#peel-editor iframe");
// The first projection able to draw something wins, so every set keeps the
// page's original load order.
const order = new URL(editor.getAttribute("src") ?? editor.dataset.src, location.href).searchParams.get("libraries").split(",");
const slider = document.querySelector("#peel-depth");
const caption = document.querySelector("#peel-caption");
const boxes = new Map();
const list = document.querySelector("#peel-layers");
for (const layer of layers) {
  const row = document.createElement("label");
  row.className = "peel-layer";
  const box = document.createElement("input");
  box.type = "checkbox";
  box.checked = true;
  box.addEventListener("change", () => {
    // Without names the editor draws the base projection, which no library
    // draws over, so names go last and come back first.
    if (layer.id === "names" && !box.checked) boxes.forEach((other) => { other.checked = false; });
    if (layer.id !== "names" && box.checked) boxes.get("names").checked = true;
    apply(layer);
  });
  const names = document.createElement("span");
  names.className = "peel-names";
  names.textContent = layer.names;
  row.append(box, document.createTextNode(" " + layer.label + " "), names);
  list.append(row);
  boxes.set(layer.id, box);
}
slider.max = String(layers.length);
slider.addEventListener("input", () => {
  const depth = Number(slider.value);
  layers.forEach((layer, index) => { boxes.get(layer.id).checked = index >= depth; });
  apply(layers[depth - 1]);
});

function apply(changed) {
  const on = layers.filter((layer) => boxes.get(layer.id).checked);
  const drawing = new Set(on.flatMap((layer) => layer.libraries));
  editor.contentWindow?.postMessage({
    type: "progred:projections",
    projections: order.filter((id) => drawing.has(id)).join(","),
    names: boxes.get("names").checked,
  }, location.origin);
  // The slider follows the checkboxes when they peel in order.
  const depth = layers.findIndex((layer) => boxes.get(layer.id).checked);
  if (on.every((layer, index) => layers.indexOf(layer) === layers.length - on.length + index)) {
    slider.value = String(depth === -1 ? layers.length : depth);
  }
  if (on.length === layers.length) caption.innerHTML = allOn(order.length);
  else if (changed && !boxes.get(changed.id).checked) caption.innerHTML = changed.off;
  else caption.innerHTML = layers.filter((layer) => !boxes.get(layer.id).checked).map((layer) => layer.off).at(-1);
}
caption.innerHTML = allOn(order.length);
// A reset reloads the editor fully drawn, and a lazy one may load after the
// slider moved, so reapply whatever is peeled once it's ready.
window.addEventListener("message", (event) => {
  if (event.origin === location.origin && event.source === editor.contentWindow
      && event.data?.type === "progred:ready" && layers.some((layer) => !boxes.get(layer.id).checked)) {
    apply();
  }
});
