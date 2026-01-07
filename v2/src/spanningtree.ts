import { Map } from 'immutable'
import type { Id } from './gid/id'
import type { Path } from './path'
import { isEmptyPath } from './path'

export type SpanningTree = {
  collapsed?: boolean
  children: Map<Id, SpanningTree>
}

export function emptySpanningTree(): SpanningTree {
  return { collapsed: undefined, children: Map() }
}

export function setCollapsedAtPath(tree: SpanningTree, path: Path, collapsed: boolean): SpanningTree {
  if (isEmptyPath(path)) {
    return { ...tree, collapsed }
  }
  const [head, ...tail] = path
  const childTree = tree.children.get(head) ?? emptySpanningTree()
  return {
    ...tree,
    children: tree.children.set(head, setCollapsedAtPath(childTree, tail, collapsed))
  }
}
