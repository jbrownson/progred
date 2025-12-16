import { el } from '../dom'
import type { Id } from '../gid/id'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'
import type { Gid } from '../gid/gid'
import type { Cursor } from '../cursor'
import { rootCursor, childCursor, cursorNode, isCycle, matchCursor, cursorsEqual } from '../cursor'
import type { Maybe } from '../maybe'
import type { SpanningTree } from '../spanningtree'
import { emptySpanningTree, getCollapsed, setCollapsed } from '../spanningtree'

function ValueView(id: Id): HTMLSpanElement | null {
  return (id instanceof StringId)
    ? el('span', { class: 'value string' }, `"${id.value}"`)
    : (id instanceof NumberId)
      ? el('span', { class: 'value number' }, String(id.value))
      : null
}

type TreeNodeCallbacks = {
  onToggle: (cursor: Cursor, currentlyCollapsed: boolean) => void
  onSelect: (cursor: Cursor) => void
}

function TreeNode(
  gid: Gid,
  root: Maybe<GuidId>,
  cursor: Cursor,
  tree: SpanningTree,
  selection: Maybe<Cursor>,
  inCycle: boolean,
  callbacks: TreeNodeCallbacks
): HTMLDivElement {
  const currentNode = cursorNode(cursor, gid, root)
  const cycle = inCycle || isCycle(cursor, gid, root)
  const edges = currentNode ? [...gid(currentNode) ?? []] : []
  const explicit = getCollapsed(tree, cursor)
  const collapsed = explicit !== undefined ? explicit : cycle
  const selected = selection !== undefined && cursorsEqual(cursor, selection)

  if (!currentNode) {
    return el('div', {
      class: selected ? 'tree-node empty selected' : 'tree-node empty',
      onClick: (e: Event) => { e.stopPropagation(); callbacks.onSelect(cursor) }
    }, '(empty)')
  }

  const header = el('div', {
    class: selected ? 'tree-node-header selected' : 'tree-node-header',
    onClick: (e: Event) => { e.stopPropagation(); callbacks.onSelect(cursor) }
  },
    edges.length > 0
      ? el('span', {
          class: 'toggle',
          onClick: (e: Event) => { e.stopPropagation(); callbacks.onToggle(cursor, collapsed) }
        }, collapsed ? '▶' : '▼')
      : null,
    ...matchCursor(cursor, {
      root: () => [],
      child: (_, label) => [
        Identicon(label.guid, 18, true),
        el('span', { class: 'arrow' }, '→')
      ]
    }),
    currentNode instanceof GuidId
      ? Identicon(currentNode.guid, 20)
      : ValueView(currentNode)
  )

  const children = !collapsed ? el('ul', { class: 'tree-node-children' },
    ...edges.map(([edgeLabel, value]) => {
      const edgeCursor = childCursor(cursor, edgeLabel)
      const edgeSelected = selection !== undefined && cursorsEqual(edgeCursor, selection)
      return el('li', {},
        value instanceof GuidId
          ? TreeNode(gid, root, edgeCursor, tree, selection, cycle, callbacks)
          : el('div', {
              class: edgeSelected ? 'tree-leaf selected' : 'tree-leaf',
              onClick: (e: Event) => { e.stopPropagation(); callbacks.onSelect(edgeCursor) }
            },
              Identicon(edgeLabel.guid, 18, true),
              el('span', { class: 'arrow' }, '→'),
              ValueView(value)
            )
      )
    })
  ) : null

  return el('div', { class: 'tree-node' }, header, children)
}

export type TreeViewCallbacks = {
  onDelete?: (cursor: Cursor) => void
}

export function TreeView(gid: Gid, root: Maybe<GuidId>, callbacks: TreeViewCallbacks = {}): HTMLDivElement {
  let tree = emptySpanningTree()
  let selection: Maybe<Cursor> = undefined

  const container = el('div', {
    class: 'tree-view',
    tabIndex: 0,
    onClick: () => { selection = undefined; render() },
    onKeyDown: (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        selection = undefined
        render()
      }
      if ((e.key === 'Delete' || e.key === 'Backspace') && selection !== undefined) {
        callbacks.onDelete?.(selection)
        selection = undefined
        render()
      }
    }
  })

  const nodeCallbacks: TreeNodeCallbacks = {
    onToggle: (cursor, currentlyCollapsed) => {
      tree = setCollapsed(tree, cursor, !currentlyCollapsed)
      render()
    },
    onSelect: (cursor) => {
      selection = cursor
      render()
    }
  }

  const render = () => {
    const content = TreeNode(gid, root, rootCursor, tree, selection, false, nodeCallbacks)
    if (container.firstChild) {
      container.replaceChild(content, container.firstChild)
    } else {
      container.appendChild(content)
    }
  }

  render()
  return container
}
