import { Map } from 'immutable'
import type { Id } from './gid/id'
import type { Cursor } from './cursor'
import { matchCursor } from './cursor'

export type SpanningTree = {
  collapsed?: boolean
  children: Map<Id, SpanningTree>
}

export function emptySpanningTree(): SpanningTree {
  return { collapsed: undefined, children: Map() }
}

export function getCollapsed(tree: SpanningTree, cursor: Cursor): boolean | undefined {
  return matchCursor(cursor, {
    root: () => tree.collapsed,
    child: (parent, label) => {
      const parentNode = matchCursor(parent, {
        root: () => tree,
        child: () => getNode(tree, parent)
      })
      return parentNode?.children.get(label)?.collapsed
    }
  })
}

function getNode(tree: SpanningTree, cursor: Cursor): SpanningTree | undefined {
  return matchCursor(cursor, {
    root: () => tree,
    child: (parent, label) => {
      const parentNode = getNode(tree, parent)
      return parentNode?.children.get(label)
    }
  })
}

export function setCollapsed(tree: SpanningTree, cursor: Cursor, collapsed: boolean | undefined): SpanningTree {
  return matchCursor(cursor, {
    root: () => ({ ...tree, collapsed }),
    child: (parent, label) => {
      const childTree = getNode(tree, cursor) ?? emptySpanningTree()
      const newChild = { ...childTree, collapsed }
      return setChild(tree, parent, label, newChild)
    }
  })
}

function setChild(tree: SpanningTree, cursor: Cursor, label: Id, child: SpanningTree): SpanningTree {
  return matchCursor(cursor, {
    root: () => ({ ...tree, children: tree.children.set(label, child) }),
    child: (parent, parentLabel) => {
      const parentNode = getNode(tree, cursor) ?? emptySpanningTree()
      const newParent = { ...parentNode, children: parentNode.children.set(label, child) }
      return setChild(tree, parent, parentLabel, newParent)
    }
  })
}
