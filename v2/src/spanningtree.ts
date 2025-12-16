import type { Cursor } from './cursor'
import { matchCursor } from './cursor'

export type SpanningTree = {
  collapsed?: boolean
  children: Map<string, SpanningTree>  // keyed by label guid
}

export function emptySpanningTree(): SpanningTree {
  return { children: new Map() }
}

function getNode(tree: SpanningTree, cursor: Cursor): SpanningTree | undefined {
  return matchCursor(cursor, {
    root: () => tree,
    child: (parent, label) => {
      const parentNode = getNode(tree, parent)
      return parentNode?.children.get(label.guid)
    }
  })
}

function getOrCreateNode(tree: SpanningTree, cursor: Cursor): SpanningTree {
  return matchCursor(cursor, {
    root: () => tree,
    child: (parent, label) => {
      const parentNode = getOrCreateNode(tree, parent)
      let node = parentNode.children.get(label.guid)
      if (!node) {
        node = { children: new Map() }
        parentNode.children.set(label.guid, node)
      }
      return node
    }
  })
}

export function getCollapsed(tree: SpanningTree, cursor: Cursor): boolean | undefined {
  return getNode(tree, cursor)?.collapsed
}

export function setCollapsed(tree: SpanningTree, cursor: Cursor, collapsed: boolean | undefined): void {
  const node = getOrCreateNode(tree, cursor)
  node.collapsed = collapsed
}
