import type { Id } from './gid/id'
import { GuidId } from './gid/id'
import type { Gid } from './gid/gid'
import { flatMapMaybe, mapMaybe } from './maybe'
import type { Maybe } from './maybe'

export type RootCursor = { type: 'root' }
export type ChildCursor = { type: 'child', parent: Cursor, label: GuidId }
export type Cursor = RootCursor | ChildCursor

export const rootCursor: RootCursor = { type: 'root' }
export function childCursor(parent: Cursor, label: GuidId): ChildCursor { return { type: 'child', parent, label } }

export function matchCursor<T>(cursor: Cursor, handlers: {
  root: () => T,
  child: (parent: Cursor, label: GuidId) => T
}): T {
  return cursor.type === 'root' ? handlers.root() : handlers.child(cursor.parent, cursor.label)
}

export function cursorNode(cursor: Cursor, gid: Gid, root: Maybe<Id>): Maybe<Id> {
  return matchCursor(cursor, {
    root: () => root,
    child: (parentCursor, label) => flatMapMaybe(cursorNode(parentCursor, gid, root), parentNode => gid(parentNode)?.get(label))
  })
}

export function isCycle(cursor: Cursor, gid: Gid, root: Maybe<Id>): boolean {
  return matchCursor(cursor, {
    root: () => false,
    child: (parent, label) =>
      mapMaybe(cursorNode(parent, gid, root), parentNode =>
        hasAncestorEdge(parent, parentNode, label, gid, root)
      ) ?? false
  })
}

// TODO: O(depth²) - cursorNode re-walks from root at each level. Could cache nodes on descent.
function hasAncestorEdge(cursor: Cursor, node: Id, label: GuidId, gid: Gid, root: Maybe<Id>): boolean {
  return matchCursor(cursor, {
    root: () => false,
    child: (parent, l) =>
      (l.equals(label) && mapMaybe(cursorNode(parent, gid, root), parentNode => parentNode.equals(node))) ||
        hasAncestorEdge(parent, node, label, gid, root)
  })
}
