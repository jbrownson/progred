import { lessonProgress } from "./lesson-progress.mjs";
import { commandIsMeta } from "./editor/platform.mjs";

for (const label of document.querySelectorAll("[data-command-key]")) {
  label.textContent = commandIsMeta(navigator.platform) ? "Cmd" : "Ctrl";
}

for (const exercise of document.querySelectorAll(".exercise")) {
  const frame = exercise.querySelector("iframe");
  const status = exercise.querySelector(".reset-status");
  const progress = exercise.querySelector(".lesson-progress");
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
    const count = tasks.filter((task) => completed.has(task.dataset.task)).length;
    progress.textContent = count === tasks.length
      ? "All steps complete. Keep experimenting!"
      : `${count} of ${tasks.length} steps complete`;
  };
  window.addEventListener("message", (event) => {
    if (event.origin === location.origin && event.source === frame.contentWindow
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
    channel = `${exercise.id}-${++generation}`;
    url.searchParams.set("observe", channel);
    frame.src = url.href;
  });
  frame.addEventListener("load", () => {
    if (status.textContent) status.textContent = "Example reset.";
  });
}
