import { lessonProgress } from "./lesson-progress.mjs";
import { commandIsMeta } from "./editor/platform.mjs";

for (const label of document.querySelectorAll("[data-command-key]")) {
  label.textContent = commandIsMeta(navigator.platform) ? "Cmd" : "Ctrl";
}

// Each editor reports its content's height. Its frame grows to fit that, up to
// most of the window (a longer program scrolls inside the editor), but never
// below the page's own height for it, which leaves room for the picker.
const contentHeights = new Map();
const designs = new Map();
const fit = (frame) => {
  if (!designs.has(frame)) designs.set(frame, frame.style.height);
  frame.style.height = designs.get(frame);
  const designed = frame.offsetHeight;
  const border = designed - frame.clientHeight;
  const fitted = Math.min(contentHeights.get(frame), Math.round(innerHeight * 0.8)) + border;
  frame.style.height = `${Math.max(designed, fitted)}px`;
};
window.addEventListener("message", (event) => {
  if (event.origin !== location.origin || event.data?.type !== "progred:size"
      || !Number.isFinite(event.data.height)) return;
  const frame = [...document.querySelectorAll("iframe")].find((frame) => frame.contentWindow === event.source);
  // Showcases and the peel hold whole programs meant to scroll inside; growing
  // them would move the page, or the peel's controls, under the reader.
  if (!frame || frame.closest(".showcase, #peel-editor")) return;
  contentHeights.set(frame, event.data.height);
  fit(frame);
});
window.addEventListener("resize", () => contentHeights.forEach((_, frame) => fit(frame)));

for (const exercise of document.querySelectorAll(".exercise")) {
  const frame = exercise.querySelector("iframe");
  const status = exercise.querySelector(".reset-status");
  const tasks = [...exercise.querySelectorAll("[data-task]")];
  let observe = lessonProgress(exercise.id);
  let completed = new Set();
  let generation = 0;
  let channel = new URL(frame.getAttribute("src"), location.href).searchParams.get("observe");
  const update = () => {
    for (const [index, task] of tasks.entries()) {
      const done = completed.has(task.dataset.task);
      task.classList.toggle("completed", done);
      task.querySelector(".task-marker").textContent = done ? "✓" : index + 1;
      task.querySelector(".task-status").textContent = done ? "Completed: " : "Not yet completed: ";
    }
  };
  window.addEventListener("message", (event) => {
    if (tasks.length && event.origin === location.origin && event.source === frame.contentWindow
        && event.data?.type === "progred:change" && event.data.channel === channel) {
      const before = completed.size;
      completed = observe(event.data.state);
      if (completed.size !== before) update();
    }
  });
  exercise.querySelector(".reset").addEventListener("click", () => {
    observe = lessonProgress(exercise.id);
    completed = new Set();
    update();
    status.textContent = "Starting over…";
    const url = new URL(frame.getAttribute("src"), location.href);
    if (tasks.length) {
      channel = `${exercise.id}-${++generation}`;
      url.searchParams.set("observe", channel);
    }
    frame.src = url.href;
  });
  frame.addEventListener("load", () => {
    if (status.textContent) status.textContent = "Example reset.";
  });
}
