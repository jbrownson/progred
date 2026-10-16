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
  // A canvas hears keys only while focused, and sees pointer flags only while
  // the pointer is over it. Once it can do neither, a modifier released
  // elsewhere would stay held here, as when Ctrl+Home scrolls editors past the
  // pointer, so release them until input says otherwise.
  const released = [false, false, false, false];
  const release = () => {
    if (!previous || previous.some(Boolean)) notify(...released);
    previous = released;
  };
  for (const target of [canvas, host]) {
    target.addEventListener("focus", () => { previous = undefined; });
    target.addEventListener("blur", release);
  }
  canvas.addEventListener("pointerleave", () => {
    if (canvas.ownerDocument?.activeElement !== canvas) release();
  });
}
