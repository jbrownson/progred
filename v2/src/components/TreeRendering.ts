import { el } from '../dom'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'

const colors = {
  string: '#2a9d2a',
  number: '#2a6a9d',
  arrow: '#999',
  toggle: '#666',
  border: '#ddd',
  btnBg: '#f8f8f8',
  setTarget: '#2563eb',
  useAsLabel: '#7c3aed',
  newNode: '#059669',
  selected: 'rgba(59, 130, 246, 0.2)',
  selectedOutline: 'rgba(59, 130, 246, 0.5)'
}

export function ValueView(id: StringId | NumberId): HTMLSpanElement {
  const isString = id instanceof StringId
  return el('span', {
    style: { color: isString ? colors.string : colors.number }
  }, isString ? `"${id.value}"` : String(id.value))
}

export function EdgeLabel(label: GuidId): [HTMLImageElement, HTMLSpanElement] {
  return [
    Identicon(label.guid, 18, true),
    el('span', { style: { color: colors.arrow, fontSize: '0.85em' } }, '→')
  ]
}

export function NodeIdenticon(node: GuidId): HTMLImageElement {
  return Identicon(node.guid, 20)
}

export function CollapseToggle(collapsed: boolean, onClick: () => void): HTMLSpanElement {
  return el('span', {
    style: { width: '1em', fontSize: '0.75em', color: colors.toggle, cursor: 'pointer' },
    onClick: (e: Event) => { e.stopPropagation(); onClick() }
  }, collapsed ? '▶' : '▼')
}

function ActionButton(
  variant: 'set-target' | 'use-as-label' | 'new-node',
  label: string,
  title: string,
  onClick: () => void
): HTMLButtonElement {
  const variantColors = {
    'set-target': colors.setTarget,
    'use-as-label': colors.useAsLabel,
    'new-node': colors.newNode
  }
  const color = variantColors[variant]
  const isNewNode = variant === 'new-node'

  return el('button', {
    class: 'hoverable',
    style: {
      padding: isNewNode ? '0.25em 0.5em' : '0.1em 0.4em',
      marginLeft: isNewNode ? '1em' : '0.25em',
      marginTop: isNewNode ? '0.5em' : undefined,
      fontSize: isNewNode ? '0.85em' : '0.75em',
      border: `1px solid ${color}`,
      borderRadius: '3px',
      background: colors.btnBg,
      color: color,
      cursor: 'pointer'
    },
    onClick: (e: Event) => { e.stopPropagation(); onClick() },
    title
  }, label)
}

export function SetTargetButton(onClick: () => void): HTMLButtonElement {
  return ActionButton('set-target', '⎆', 'Set as target', onClick)
}

export function UseAsLabelButton(onClick: () => void): HTMLButtonElement {
  return ActionButton('use-as-label', '+⏍', 'Add edge with this as label', onClick)
}

export function NewNodeButton(onClick: () => void): HTMLButtonElement {
  return ActionButton('new-node', '+ New Node', 'Create new node', onClick)
}

export function NodeHeader(
  selected: boolean,
  onClick: () => void,
  ...children: (HTMLElement | null)[]
): HTMLDivElement {
  return el('div', {
    class: 'hoverable',
    style: {
      display: 'flex',
      alignItems: 'center',
      gap: '0.5em',
      padding: '0.25em',
      cursor: 'pointer',
      borderRadius: '4px',
      ...(selected ? { background: colors.selected, outline: `2px solid ${colors.selectedOutline}` } : {})
    },
    onClick: (e: Event) => {
      e.stopPropagation()
      if ((e.target as HTMLElement).tagName === 'BUTTON') return
      onClick()
    }
  }, ...children)
}

export function EmptyNode(selected: boolean, onClick: () => void): HTMLDivElement {
  return el('div', {
    class: 'hoverable',
    style: {
      flex: '1',
      padding: '0.25em',
      cursor: 'pointer',
      borderRadius: '4px',
      ...(selected ? { background: colors.selected, outline: `2px solid ${colors.selectedOutline}` } : {})
    },
    onClick: (e: Event) => { e.stopPropagation(); onClick() }
  }, '(empty)')
}

export function LeafNode(
  value: StringId | NumberId,
  selected: boolean,
  onClick: () => void
): HTMLDivElement {
  return el('div', {
    class: 'hoverable',
    style: {
      flex: '1',
      display: 'flex',
      alignItems: 'center',
      padding: '0.25em',
      cursor: 'pointer',
      borderRadius: '4px',
      ...(selected ? { background: colors.selected, outline: `2px solid ${colors.selectedOutline}` } : {})
    },
    onClick: (e: Event) => { e.stopPropagation(); onClick() }
  }, ValueView(value))
}

export function ChildrenList(...children: HTMLElement[]): HTMLUListElement {
  return el('ul', {
    style: {
      listStyle: 'none',
      margin: '0',
      padding: '0',
      borderLeft: `1px solid ${colors.border}`,
      marginLeft: '0.5em'
    }
  }, ...children)
}

export function ChildItem(...children: (HTMLElement | HTMLImageElement | HTMLSpanElement)[]): HTMLLIElement {
  return el('li', {
    style: {
      display: 'flex',
      alignItems: 'flex-start',
      gap: '0.5em',
      padding: '0.25em 0 0.25em 0.5em'
    }
  }, ...children)
}

export function GuidNodeWrapper(header: HTMLElement, children: HTMLElement | null): HTMLDivElement {
  return el('div', { style: { flex: '1' } }, header, children)
}

export function TreeViewContainer(onDeselect: () => void, ...children: (HTMLElement | null)[]): HTMLDivElement {
  return el('div', {
    style: { textAlign: 'left', marginLeft: '1em' },
    onClick: onDeselect
  }, ...children)
}

