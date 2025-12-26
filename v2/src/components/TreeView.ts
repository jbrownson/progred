import { el } from '../dom'
import { GuidId, StringId, NumberId } from '../gid/id'
import { Identicon } from './Identicon'
import type { Gid } from '../gid/gid'
import type { Cursor } from '../cursor'
import { rootCursor, childCursor, cursorNode, isCycle, matchCursor, cursorsEqual } from '../cursor'
import type { Maybe } from '../maybe'
import type { SpanningTree } from '../spanningtree'
import { emptySpanningTree, getCollapsed, setCollapsed } from '../spanningtree'

function ValueView(id: StringId | NumberId): HTMLSpanElement {
  return (id instanceof StringId)
    ? el('span', { class: 'value string' }, `"${id.value}"`)
    : el('span', { class: 'value number' }, String(id.value))
}

type TreeNodeCallbacks = {
  setCollapsed: (cursor: Cursor, collapsed: boolean) => void
  select: (cursor: Cursor) => void
}

function TreeNode(
  gid: Gid,
  root: Maybe<GuidId>,
  cursor: Cursor,
  tree: SpanningTree,
  selection: Maybe<Cursor>,
  parentInCycle: boolean,
  callbacks: TreeNodeCallbacks
): HTMLDivElement {
  const node = cursorNode(cursor, gid, root)
  const inCycle = parentInCycle || isCycle(cursor, gid, root)
  const edges = node ? [...gid(node) ?? []] : []
  const collapsed = getCollapsed(tree, cursor) ?? inCycle
  const selected = selection && cursorsEqual(cursor, selection)

  if (!node) {
    return el('div', {
      class: selected ? 'tree-node empty selected' : 'tree-node empty',
      onClick: (e: Event) => { e.stopPropagation(); callbacks.select(cursor) }
    }, '(empty)')
  }

  const header = el('div', {
    class: selected ? 'tree-node-header selected' : 'tree-node-header',
    onClick: (e: Event) => { e.stopPropagation(); callbacks.select(cursor) }
  },
    edges.length > 0
      ? el('span', {
        class: 'toggle',
        onClick: (e: Event) => { e.stopPropagation(); callbacks.setCollapsed(cursor, !collapsed) }
      }, collapsed ? '▶' : '▼')
      : null,
    ...matchCursor(cursor, {
      root: () => [],
      child: (_, label) => [
        Identicon(label.guid, 18, true),
        el('span', { class: 'arrow' }, '→')
      ]
    }),
    node instanceof GuidId
      ? Identicon(node.guid, 20)
      : ValueView(node)
  )

  const children = !collapsed ? el('ul', { class: 'tree-node-children' },
    ...edges.map(([edgeLabel, value]) => {
      const edgeCursor = childCursor(cursor, edgeLabel)
      const edgeSelected = selection !== undefined && cursorsEqual(edgeCursor, selection)
      return el('li', {},
        value instanceof GuidId
          ? TreeNode(gid, root, edgeCursor, tree, selection, inCycle, callbacks)
          : el('div', {
            class: edgeSelected ? 'tree-leaf selected' : 'tree-leaf',
            onClick: (e: Event) => { e.stopPropagation(); callbacks.select(edgeCursor) }
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

export type TreeViewState = {
  tree: SpanningTree
  selection: Maybe<Cursor>
}

export function emptyTreeViewState(): TreeViewState {
  return { tree: emptySpanningTree(), selection: undefined }
}

export type TreeViewCallbacks = {
  onStateChange: (state: TreeViewState) => void
}

export function TreeView(
  gid: Gid,
  root: Maybe<GuidId>,
  state: TreeViewState,
  callbacks: TreeViewCallbacks
): HTMLDivElement {
  const { tree, selection } = state

  const nodeCallbacks: TreeNodeCallbacks = {
    setCollapsed: (cursor, collapsed) => {
      callbacks.onStateChange({ ...state, tree: setCollapsed(tree, cursor, collapsed) })
    },
    select: (cursor) => {
      callbacks.onStateChange({ ...state, selection: cursor })
    }
  }

  return el('div',
    {
      class: 'tree-view',
      onClick: () => callbacks.onStateChange({ ...state, selection: undefined })
    },
    TreeNode(gid, root, rootCursor, tree, selection, false, nodeCallbacks)
  )
}
