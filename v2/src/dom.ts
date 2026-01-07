type Child = Node | string | null | false | undefined

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs?: Record<string, unknown>,
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag)
  if (attrs) {
    for (const [key, value] of Object.entries(attrs)) {
      if (key === 'class') {
        element.className = value as string
      } else if (key === 'style' && typeof value === 'object' && value !== null) {
        Object.assign(element.style, value)
      } else if (key.startsWith('on') && typeof value === 'function') {
        const event = key.slice(2).toLowerCase()
        element.addEventListener(event, value as EventListener)
      } else if (value != null) {
        element.setAttribute(key, String(value))
      }
    }
  }
  for (const child of children) {
    if (child === null || child === false || child === undefined) continue
    if (typeof child === 'string') {
      element.appendChild(document.createTextNode(child))
    } else {
      element.appendChild(child)
    }
  }
  return element
}
