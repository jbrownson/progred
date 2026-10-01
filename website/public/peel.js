// Peel libraries off a live program: each layer is a set of libraries the
// embedded editor stops drawing with, top to bottom.
const layers = [
  {
    id: "pictures",
    label: "Pictures and sliders",
    names: "layout, presentation, controls",
    libraries: ["fb2a4dac87512d69448650bc0e29dc80", "d22b834154d60b1df228f9bb4d3c13de", "666ba40b81028e32c78bdc1665d6c3a9"],
    off: "The picture was a view. Without the layout, presentation, and controls libraries, nothing draws the forest or its slider, so you see what they were drawing from: a record asking to <code>render</code> the program. Nothing in the document changed. The empty boxes are cells whose names came from the controls library, which is switched off now.",
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
    off: "Grap, the language, is a library. Without it, the editor doesn't know that a record with a <code>function</code> key is a call, or that a record with <code>params</code> and a <code>body</code> is a function. Everything is still here, as plain records. Names in parentheses, like <code>(forest)</code>, are functions this document defines; short codes like <code>…d684b</code> are keys owned by libraries that aren't loaded anymore, such as Grap's own <code>function</code> key.",
  },
  {
    id: "numbers",
    label: "Numbers",
    names: "f64",
    libraries: ["1fdb573a2c56a7063546c195318214bc"],
    off: "Numbers were the f64 library reading eight bytes under its key. The little <strong>f64</strong> was its signature; without it, <code>7<sub class=\"tag\">f64</sub></code> is <code>{…5d561: 0x0000000000001c40}</code>, the eight bytes of 7.0, least significant first.",
  },
];

const allOn = (count) => `All ${count} of this editor's libraries are on, grouped into the layers above. Each mark on the screen, from the trees to the little <strong>f64</strong> tags, is drawn by one of them. Move the slider, or switch layers off one at a time.`;
const allOff = "This is what's actually stored: records <code>{ }</code>, lists <code>[ ]</code>, bytes <code>0x…</code>, and cells <code>( )</code>, plus the names this document gave its own cells. Every library you switched off only changed how it's drawn. Switch them back on, in any order.";

const editor = document.querySelector("#peel-editor iframe");
// The first library able to draw something wins, so every stack keeps the
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
  box.addEventListener("change", () => apply(layer));
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
  const enabled = new Set(on.flatMap((layer) => layer.libraries));
  const libraries = order.filter((id) => enabled.has(id));
  editor.contentWindow?.postMessage({ type: "progred:libraries", libraries: libraries.join(",") }, location.origin);
  // The slider follows the checkboxes when they peel in order.
  const depth = layers.findIndex((layer) => boxes.get(layer.id).checked);
  if (on.every((layer, index) => layers.indexOf(layer) === layers.length - on.length + index)) {
    slider.value = String(depth === -1 ? layers.length : depth);
  }
  if (on.length === layers.length) caption.innerHTML = allOn(order.length);
  else if (on.length === 0) caption.innerHTML = allOff;
  else if (changed && !boxes.get(changed.id).checked) caption.innerHTML = changed.off;
  else caption.innerHTML = layers.filter((layer) => !boxes.get(layer.id).checked).map((layer) => layer.off).at(-1);
}
caption.innerHTML = allOn(order.length);
