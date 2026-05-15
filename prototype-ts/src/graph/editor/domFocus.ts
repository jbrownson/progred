export function focus(htmlElement: HTMLElement) {
  if (document.activeElement !== htmlElement)
    htmlElement.focus()
}
