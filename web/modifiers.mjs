// Winit's web backend omits pointer modifiers while its canvas is unfocused.
// Preserve the browser's observations without focusing or cancelling input.
export function forwardModifiers(canvas, host, notify) {
  let previous;
  const observe = (event) => {
    const current = [event.shiftKey, event.ctrlKey, event.altKey, event.metaKey].map(Boolean);
    if (!previous || current.some((value, index) => value !== previous[index])) {
      previous = current;
      notify(...current);
    }
  };
  for (const type of ["pointerover", "pointermove", "pointerdown", "pointerup", "wheel", "keydown", "keyup"]) {
    canvas.addEventListener(type, observe, { capture: true, passive: true });
  }
  // Focus changes can reset Winit's state without a corresponding key event.
  for (const target of [canvas, host]) {
    for (const type of ["focus", "blur"]) {
      target.addEventListener(type, () => { previous = undefined; });
    }
  }
}
