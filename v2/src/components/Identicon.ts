import { el } from '../dom'
import { minidenticon } from 'minidenticons'

export function Identicon(value: string, size = 16, label = false): HTMLImageElement {
  const svg = minidenticon(value)
  return el('img', {
    src: `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`,
    width: size,
    height: size,
    style: {
      background: '#f0f0f0',
      border: '1px solid #ccc',
      borderRadius: label ? '50%' : '3px'
    }
  })
}
