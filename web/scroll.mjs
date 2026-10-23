// Decide ownership during the DOM event; actual editor dispatch stays in winit.
export function routeWheel(canvas, captures) {
  canvas.addEventListener("wheel", (event) => {
    const rect = canvas.getBoundingClientRect();
    const take = event.cancelable && rect.width > 0 && rect.height > 0 && captures(
      (event.clientX - rect.left) * canvas.width / rect.width,
      (event.clientY - rect.top) * canvas.height / rect.height,
      event.deltaX, event.deltaY, event.deltaMode,
    );
    if (take) event.preventDefault();
    else event.stopImmediatePropagation();
  }, { capture: true, passive: false });
}
