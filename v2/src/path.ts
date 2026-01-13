import { GuidId } from './gid/id'
import type { Id } from './gid/id'
import type { Gid } from './gid/gid'
import type { Maybe } from './maybe'

export type Path = { rootSlot: GuidId, edges: GuidId[] }

export function rootPath(rootSlot: GuidId): Path { return { rootSlot, edges: [] } }

export function childPath(parent: Path, label: GuidId): Path {
  return { rootSlot: parent.rootSlot, edges: [...parent.edges, label] }
}

export function popPath(path: Path): Maybe<{ parent: Path, label: GuidId }> {
  return path.edges.length === 0
    ? undefined
    : { parent: { rootSlot: path.rootSlot, edges: path.edges.slice(0, -1) }, label: path.edges[path.edges.length - 1] }
}

export function pathsEqual(a: Path, b: Path): boolean {
  return a.rootSlot.equals(b.rootSlot) && a.edges.length === b.edges.length && a.edges.every((id, i) => id.equals(b.edges[i]))
}

export function isRootPath(path: Path): boolean { return path.edges.length === 0 }

export function pathNode(gid: Gid, rootNode: Maybe<Id>, path: Path): Maybe<Id> {
  return path.edges.reduce<Maybe<Id>>(
    (node, label) => node instanceof GuidId ? gid(node)?.get(label) : undefined,
    rootNode
  )
}
