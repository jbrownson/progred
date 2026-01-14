import { init, classModule, propsModule, styleModule, eventListenersModule, attributesModule, h, toVNode } from 'snabbdom'
import type { VNode } from 'snabbdom'

export type { VNode }

export const patch = init([
  classModule,
  propsModule,
  styleModule,
  eventListenersModule,
  attributesModule
])

type Child = VNode | string | null | false | undefined

type ElAttrs = {
  class?: string
  style?: Record<string, string | number | undefined>
  key?: string
  [key: string]: unknown
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs?: ElAttrs,
  ...children: Child[]
): VNode {
  const data: Record<string, unknown> = {}

  if (attrs) {
    const props: Record<string, unknown> = {}
    const on: Record<string, unknown> = {}
    const remainingAttrs: Record<string, unknown> = {}

    for (const [key, value] of Object.entries(attrs)) {
      if (key === 'class') {
        data.class = parseClass(value as string)
      } else if (key === 'style') {
        data.style = value
      } else if (key === 'key') {
        data.key = value
      } else if (key === 'hook') {
        data.hook = value
      } else if (key.startsWith('on') && typeof value === 'function') {
        const event = key.slice(2).toLowerCase()
        on[event] = value
      } else if (key === 'type' || key === 'value' || key === 'checked' || key === 'placeholder' || key === 'disabled') {
        props[key] = value
      } else if (value != null) {
        remainingAttrs[key] = value
      }
    }

    if (Object.keys(props).length > 0) data.props = props
    if (Object.keys(on).length > 0) data.on = on
    if (Object.keys(remainingAttrs).length > 0) data.attrs = remainingAttrs
  }

  const filteredChildren = children.filter(
    (c): c is VNode | string => c !== null && c !== false && c !== undefined
  )

  return h(tag, data, filteredChildren)
}

function parseClass(classString: string): Record<string, boolean> {
  const result: Record<string, boolean> = {}
  for (const cls of classString.split(' ').filter(Boolean)) {
    result[cls] = true
  }
  return result
}

export { toVNode }
