// Decide cancellation from actual dispatch, while the DOM event is still live.
export function routeKeyboard(canvas, dispatch) {
  for (const type of ["keydown", "keyup"]) {
    canvas.addEventListener(type, (event) => {
      // Winit's keyboard listener must not cancel or dispatch this key again.
      // This does not cancel browser defaults; only preventDefault does that.
      event.stopImmediatePropagation();
      if (dispatch(event)) event.preventDefault();
    }, { capture: true, passive: false });
  }
}
