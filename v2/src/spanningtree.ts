import { Map } from 'immutable'
import type { Id, GuidId } from './gid/id'
import type { Path } from './path'

export type SpanningTree = {
  collapsed?: boolean
  children: Map<Id, SpanningTree>
}

export function emptySpanningTree(): SpanningTree {
  return { collapsed: undefined, children: Map() }
}

function setCollapsedAtEdges(tree: SpanningTree, edges: GuidId[], collapsed: boolean): SpanningTree {
  if (edges.length === 0) {
    return { ...tree, collapsed }
  }
  const [head, ...tail] = edges
  const childTree = tree.children.get(head) ?? emptySpanningTree()
  return {
    ...tree,
    children: tree.children.set(head, setCollapsedAtEdges(childTree, tail, collapsed))
  }
}

export function setCollapsedAtPath(tree: SpanningTree, path: Path, collapsed: boolean): SpanningTree {
  return setCollapsedAtEdges(tree, path.edges, collapsed)
}
