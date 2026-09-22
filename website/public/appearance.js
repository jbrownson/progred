import { isTheme, savedTheme, saveTheme } from "./editor/theme.mjs";

const button = document.querySelector("#theme-toggle");
const frames = [...document.querySelectorAll(".exercise iframe")];
let theme = savedTheme(window);
const send = (frame) => frame.contentWindow?.postMessage({ type: "progred:theme", theme }, location.origin);
const apply = () => {
  document.documentElement.dataset.theme = theme;
  button.setAttribute("aria-checked", String(theme === "dark"));
  button.setAttribute("title", theme === "dark" ? "Switch to light mode" : "Switch to dark mode");
  for (const frame of frames) send(frame);
};
button.addEventListener("click", () => {
  theme = theme === "light" ? "dark" : "light";
  saveTheme(window, theme);
  apply();
});
for (const frame of frames) frame.addEventListener("load", () => send(frame));
window.addEventListener("message", (event) => {
  if (event.origin === location.origin && event.data?.type === "progred:ready") {
    const frame = frames.find((frame) => frame.contentWindow === event.source);
    if (frame) send(frame);
  }
});
window.addEventListener("storage", (event) => {
  if (event.key === "progred-theme" || event.key === null) {
    theme = isTheme(event.newValue) ? event.newValue : "light";
    apply();
  }
});
apply();
