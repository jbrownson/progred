import { GuidId } from './gid/id'
import type { Id } from './gid/id'
import type { Gid } from './gid/gid'
import type { Maybe } from './maybe'

export type Path = GuidId[]

export const emptyPath: Path = []

export function childPath(parent: Path, label: GuidId): Path { return [...parent, label] }

export function popPath(path: Path): Maybe<{ parent: Path, label: GuidId }> {
  return path.length === 0
    ? undefined
    : { parent: path.slice(0, -1), label: path[path.length - 1] }
}

export function pathsEqual(a: Path, b: Path): boolean {
  return a.length === b.length && a.every((id, i) => id.equals(b[i]))
}

export function isEmptyPath(path: Path): boolean { return path.length === 0 }

export function pathNode(gid: Gid, root: Maybe<Id>, path: Path): Maybe<Id> {
  return path.reduce<Maybe<Id>>(
    (node, label) => node instanceof GuidId ? gid(node)?.get(label) : undefined,
    root
  )
}
