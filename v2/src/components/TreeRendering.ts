import { el } from '../dom'
import type { VNode } from '../dom'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'

const colors = {
  string: '#2a9d2a',
  number: '#2a6a9d',
  toggle: '#666',
  border: '#ddd',
  btnBg: '#f8f8f8',
  setTarget: '#2563eb',
  useAsLabel: '#7c3aed',
  newNode: '#059669',
  selected: 'rgba(59, 130, 246, 0.2)',
  selectedOutline: 'rgba(59, 130, 246, 0.5)'
}

const layout = {
  rowHeight: 28,
  labelWidth: 20,
  nodeIdenticonSize: 20,
  labelIdenticonSize: 15,
  headerGap: 4,
  headerPadding: 4,
  itemPaddingY: 2,
  contentPaddingX: 4,
  containerPadding: 8,
  lineWidth: 1,
  insertionHeight: 8,
  insertionCaretOffset: 4,
  insertionCaretSize: 12,
  borderRadius: 4,
  outlineWidth: 2,
  inputWidthPadding: 4,

  get childIndent() { return this.headerPadding },
  get lineX() { return this.childIndent + this.labelWidth / 2 },
  get lineLeft() { return this.lineX - this.lineWidth / 2 },
  get hLineStart() { return (this.labelWidth + this.labelIdenticonSize) / 2 },
  get hLineEnd() { return this.labelWidth + this.headerGap + this.headerPadding },
  get vLineTop() { return this.headerPadding + this.nodeIdenticonSize },
  get insertionCaretOverhang() { return (this.insertionCaretSize - this.insertionHeight) / 2 },
  get firstInsertionPullUp() { return this.containerPadding - this.insertionCaretOverhang }
}

const selectedStyle = {
  background: colors.selected,
  outline: `${layout.outlineWidth}px solid ${colors.selectedOutline}`
}

const flexCenter = {
  display: 'flex',
  alignItems: 'center'
}

const inlineFlexCenter = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center'
}

const resetInputStyle = {
  fontFamily: 'inherit',
  fontSize: 'inherit',
  border: 'none',
  background: 'transparent',
  padding: '0',
  margin: '0',
  outline: 'none'
}

const clickable = (onClick: () => void) => ({
  cursor: 'pointer',
  onClick: (e: Event) => { e.stopPropagation(); onClick() }
})

export function ValueView(id: StringId | NumberId): VNode {
  const isString = id instanceof StringId
  return el('span', {
    style: { color: isString ? colors.string : colors.number }
  }, isString ? `"${id.value}"` : String(id.value))
}

export function EdgeLabel(label: GuidId): VNode {
  return el('span', {
    style: {
      ...inlineFlexCenter,
      width: `${layout.labelWidth}px`,
      height: `${layout.rowHeight}px`
    }
  }, Identicon(label.guid, layout.labelIdenticonSize, true))
}

export function NodeIdenticon(node: GuidId): VNode {
  return Identicon(node.guid, layout.nodeIdenticonSize)
}

export function CollapseToggle(collapsed: boolean, onClick: () => void): VNode {
  return el('span', {
    style: {
      ...inlineFlexCenter,
      fontSize: '0.7em',
      color: colors.toggle,
      marginLeft: `${layout.headerGap}px`,
      width: '1.4em',
      height: '1.4em',
      borderRadius: '4px',
      background: colors.btnBg,
      border: `1px solid ${colors.border}`
    },
    ...clickable(onClick)
  }, collapsed ? '▸' : '▾')
}

function ActionButton(
  variant: 'set-target' | 'use-as-label' | 'new-node',
  label: string,
  title: string,
  onClick: () => void
): VNode {
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
      borderRadius: `${layout.borderRadius}px`,
      background: colors.btnBg,
      color: color
    },
    cursor: 'pointer',
    onClick: (e: Event) => { e.preventDefault(); e.stopPropagation(); onClick() },
    title
  }, label)
}

export function SetTargetButton(onClick: () => void): VNode {
  return ActionButton('set-target', '⎆', 'Set as target', onClick)
}

export function UseAsLabelButton(onClick: () => void): VNode {
  return ActionButton('use-as-label', '+⏍', 'Add edge with this as label', onClick)
}

export function NewNodeButton(onClick: () => void): VNode {
  return ActionButton('new-node', '+ New Node', 'Create new node', onClick)
}

export function InsertionPoint(selected: boolean, isFirst: boolean, onClick: () => void): VNode {
  return el('div', {
    class: `insertion-point${selected ? ' selected' : ''}`,
    style: {
      position: 'relative',
      height: `${layout.insertionHeight}px`,
      marginTop: isFirst ? `${-layout.firstInsertionPullUp}px` : undefined
    },
    ...clickable(onClick)
  },
    el('span', {
      class: `insertion-caret${selected ? ' selected' : ''}`,
      style: {
        position: 'absolute',
        left: `${layout.insertionCaretOffset}px`,
        top: '50%',
        transform: 'translateY(-50%)',
        fontSize: `${layout.insertionCaretSize}px`,
        color: selected ? colors.setTarget : colors.toggle,
        transition: 'opacity 0.1s'
      }
    }, '▶')
  )
}

export function LabelSlot(
  label: string,
  value: GuidId | undefined,
  selected: boolean,
  onClick: () => void
): VNode {
  return el('div', {
    class: 'hoverable',
    style: {
      ...flexCenter,
      gap: `${layout.headerGap}px`,
      padding: `${layout.headerPadding}px`,
      marginBottom: `${layout.headerGap}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...(selected ? selectedStyle : {})
    },
    ...clickable(onClick)
  },
    el('span', { style: { color: colors.toggle, fontSize: '0.85em' } }, `${label}:`),
    value ? Identicon(value.guid, layout.nodeIdenticonSize) : el('span', { style: { color: colors.toggle } }, '(empty)')
  )
}

export function NodeHeader(
  selected: boolean,
  onClick: () => void,
  ...children: (VNode | null)[]
): VNode {
  return el('div', {
    class: 'hoverable',
    style: {
      ...flexCenter,
      gap: `${layout.headerGap}px`,
      padding: `${layout.headerPadding}px`,
      cursor: 'pointer',
      borderRadius: `${layout.borderRadius}px`,
      ...(selected ? selectedStyle : {})
    },
    onClick: (e: Event) => { e.stopPropagation(); onClick() }
  }, ...children)
}

function getTextWidth(text: string): number {
  const canvas = document.createElement('canvas')
  const context = canvas.getContext('2d')!
  context.font = getComputedStyle(document.body).font
  return context.measureText(text).width
}

function resizeInput(input: HTMLInputElement): void {
  input.style.width = `${getTextWidth(input.value) + layout.inputWidthPadding}px`
}

export function EmptyNode(selected: boolean, onClick: () => void): VNode {
  return el('div', {
    class: 'hoverable',
    style: {
      flex: '1',
      minHeight: `${layout.rowHeight}px`,
      padding: `0 ${layout.contentPaddingX}px`,
      marginLeft: `${layout.headerGap}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...(selected ? selectedStyle : {})
    },
    ...clickable(onClick)
  }, '(empty)')
}

// TODO: Auto-detecting number vs string is too simplistic - "123" can't be entered as a string.
// Need completion UI with named entries (like v1's PlaceholderComponent) to let user pick type explicitly.
export function EditablePlaceholder(
  onCreate: (id: StringId | NumberId) => void,
  onBlur: () => void
): VNode {
  let hasFocused = false

  const commit = (input: HTMLInputElement) => {
    if (!input.isConnected) return
    if (!hasFocused) return
    const value = input.value
    if (value.length > 0) {
      const num = +value
      onCreate(isNaN(num) ? new StringId(value) : new NumberId(num))
    }
    onBlur()
  }

  return el('div', {
    style: {
      ...flexCenter,
      flex: '1',
      minHeight: `${layout.rowHeight}px`,
      padding: `0 ${layout.contentPaddingX}px`,
      marginLeft: `${layout.headerGap}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...selectedStyle
    },
    onClick: (e: Event) => e.stopPropagation()
  },
    el('input', {
      type: 'text',
      placeholder: 'value...',
      style: {
        ...resetInputStyle,
        color: colors.toggle
      },
      hook: {
        insert: (vnode: VNode) => {
          const input = vnode.elm as HTMLInputElement
          input.focus()
        }
      },
      onFocus: () => { hasFocused = true },
      onInput: (e: Event) => resizeInput(e.target as HTMLInputElement),
      onBlur: (e: Event) => commit(e.target as HTMLInputElement),
      onClick: (e: Event) => e.stopPropagation(),
      onKeyDown: (e: KeyboardEvent) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          commit(e.target as HTMLInputElement)
        } else if (e.key === 'Escape') {
          // Let Escape bubble to document handler
        } else if (e.key === 'Backspace' || e.key === 'Delete') {
          if ((e.target as HTMLInputElement).value.length === 0) {
            commit(e.target as HTMLInputElement)
          } else {
            e.stopPropagation()
          }
        } else {
          e.stopPropagation()
        }
      }
    })
  )
}

export function LeafNode(
  value: StringId | NumberId,
  selected: boolean,
  onClick: () => void
): VNode {
  return el('div', {
    class: 'hoverable',
    style: {
      ...flexCenter,
      minHeight: `${layout.rowHeight}px`,
      padding: `0 ${layout.contentPaddingX}px`,
      marginLeft: `${layout.headerGap}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...(selected ? selectedStyle : {})
    },
    ...clickable(onClick)
  }, ValueView(value))
}

export function EditableStringNode(
  value: string,
  onChange: (value: string) => void,
  onBlur: () => void
): VNode {
  let hasFocused = false

  const commit = (input: HTMLInputElement) => {
    if (!input.isConnected) return
    if (!hasFocused) return
    onChange(input.value)
    onBlur()
  }

  return el('div', {
    style: {
      ...flexCenter,
      flex: '1',
      padding: `0 ${layout.contentPaddingX}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...selectedStyle
    },
    onClick: (e: Event) => e.stopPropagation()
  },
    el('span', { style: { color: colors.string } }, '"'),
    el('input', {
      type: 'text',
      value,
      style: {
        ...resetInputStyle,
        color: colors.string
      },
      hook: {
        insert: (vnode: VNode) => {
          const input = vnode.elm as HTMLInputElement
          resizeInput(input)
          input.focus()
        }
      },
      onFocus: () => { hasFocused = true },
      onInput: (e: Event) => resizeInput(e.target as HTMLInputElement),
      onBlur: (e: Event) => commit(e.target as HTMLInputElement),
      onClick: (e: Event) => e.stopPropagation(),
      onKeyDown: (e: KeyboardEvent) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          commit(e.target as HTMLInputElement)
        } else if (e.key === 'Escape') {
          // Let Escape bubble to document handler
        } else if (!((e.key === 'Backspace' || e.key === 'Delete') && (e.target as HTMLInputElement).value.length === 0)) {
          e.stopPropagation()
        }
      }
    }),
    el('span', { style: { color: colors.string } }, '"')
  )
}

export function EditableNumberNode(
  value: number,
  onChange: (value: number) => void,
  onBlur: () => void
): VNode {
  let hasFocused = false

  const commit = (input: HTMLInputElement) => {
    if (!input.isConnected) return
    if (!hasFocused) return
    const num = +input.value
    if (!isNaN(num)) onChange(num)
    onBlur()
  }

  return el('div', {
    style: {
      ...flexCenter,
      flex: '1',
      padding: `0 ${layout.contentPaddingX}px`,
      borderRadius: `${layout.borderRadius}px`,
      ...selectedStyle
    },
    onClick: (e: Event) => e.stopPropagation()
  },
    el('input', {
      type: 'text',
      value: String(value),
      style: {
        ...resetInputStyle,
        color: colors.number
      },
      hook: {
        insert: (vnode: VNode) => {
          const input = vnode.elm as HTMLInputElement
          resizeInput(input)
          input.focus()
        }
      },
      onFocus: () => { hasFocused = true },
      onInput: (e: Event) => resizeInput(e.target as HTMLInputElement),
      onBlur: (e: Event) => commit(e.target as HTMLInputElement),
      onClick: (e: Event) => e.stopPropagation(),
      onKeyDown: (e: KeyboardEvent) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          commit(e.target as HTMLInputElement)
        } else if (e.key === 'Escape') {
          // Let Escape bubble to document handler
        } else if (!((e.key === 'Backspace' || e.key === 'Delete') && (e.target as HTMLInputElement).value.length === 0)) {
          e.stopPropagation()
        }
      }
    })
  )
}

export function ChildrenList(...children: VNode[]): VNode {
  return el('ul', {
    style: {
      listStyle: 'none',
      margin: '0',
      padding: '0',
      marginLeft: `${layout.childIndent}px`
    }
  }, ...children)
}

export function ChildItem(label: VNode, content: VNode): VNode {
  const hLine = el('div', {
    style: {
      position: 'absolute',
      left: `${layout.hLineStart}px`,
      width: `${layout.hLineEnd - layout.hLineStart}px`,
      top: `${layout.itemPaddingY + layout.rowHeight / 2 - layout.lineWidth / 2}px`,
      height: `${layout.lineWidth}px`,
      background: colors.border
    }
  })

  return el('li', {
    style: {
      display: 'grid',
      gridTemplateColumns: `${layout.labelWidth}px 1fr`,
      alignItems: 'start',
      gap: `${layout.headerGap}px`,
      padding: `${layout.itemPaddingY}px 0`,
      position: 'relative'
    }
  }, label, hLine, content)
}

export function GuidNodeWrapper(header: VNode, children: VNode | null): VNode {
  if (!children) {
    return el('div', { style: { flex: '1' } }, header)
  }

  const line = el('div', {
    style: {
      position: 'absolute',
      left: `${layout.lineLeft}px`,
      top: `${layout.vLineTop}px`,
      bottom: '0',
      width: `${layout.lineWidth}px`,
      background: colors.border,
      zIndex: '-1'
    }
  })

  return el('div', { style: { flex: '1', position: 'relative' } }, header, line, children)
}

export function TreeViewContainer(onDeselect: () => void, ...children: (VNode | null)[]): VNode {
  return el('div', {
    style: { textAlign: 'left', padding: `${layout.containerPadding}px`, minHeight: '100vh', boxSizing: 'border-box' },
    onClick: onDeselect
  }, ...children)
}
