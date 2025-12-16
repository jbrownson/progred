import { el } from '../dom'
import type { Id } from '../gid/id'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'
import type { Gid } from '../gid/gid'
import type { Cursor } from '../cursor'
import { rootCursor, childCursor, cursorNode, isCycle, matchCursor } from '../cursor'
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

function TreeNode(
  gid: Gid,
  root: Maybe<GuidId>,
  cursor: Cursor,
  tree: SpanningTree,
  inCycle: boolean,
  onToggle: (cursor: Cursor, currentlyCollapsed: boolean) => void
): HTMLDivElement { // 
  const currentNode = cursorNode(cursor, gid, root)
  const cycle = inCycle || isCycle(cursor, gid, root)
  const edges = currentNode ? [...gid(currentNode) ?? []] : []
  const explicit = getCollapsed(tree, cursor)
  const collapsed = explicit !== undefined ? explicit : cycle

  if (!currentNode) {
    return el('div', { class: 'tree-node empty' }, '(empty)')
  }

  const header = el('div', {
    class: 'tree-node-header',
    onClick: () => onToggle(cursor, collapsed)
  },
    edges.length > 0 ? el('span', { class: 'toggle' }, collapsed ? '▶' : '▼') : null,
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
    ...edges.map(([edgeLabel, value]) =>
      el('li', {},
        value instanceof GuidId
          ? TreeNode(gid, root, childCursor(cursor, edgeLabel), tree, cycle, onToggle)
          : el('div', { class: 'tree-leaf' },
              Identicon(edgeLabel.guid, 18, true),
              el('span', { class: 'arrow' }, '→'),
              ValueView(value)
            )
      )
    )
  ) : null

  return el('div', { class: 'tree-node' }, header, children)
}

export function TreeView(gid: Gid, root: Maybe<GuidId>): HTMLDivElement {
  let tree = emptySpanningTree()
  const container = el('div', { class: 'tree-view' })

  const render = () => {
    const content = TreeNode(gid, root, rootCursor, tree, false, toggle)
    if (container.firstChild) {
      container.replaceChild(content, container.firstChild)
    } else {
      container.appendChild(content)
    }
  }

  const toggle = (cursor: Cursor, currentlyCollapsed: boolean) => {
    tree = setCollapsed(tree, cursor, !currentlyCollapsed)
    render()
  }

  render()
  return container
}
