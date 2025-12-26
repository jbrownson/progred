import type { Id } from './gid/id'
import { GuidId } from './gid/id'
import type { Gid } from './gid/gid'
import { flatMapMaybe } from './maybe'
import type { Maybe } from './maybe'

export type RootCursor = { type: 'root' }
export type ChildCursor = { type: 'child', parent: Cursor, label: GuidId }
export type Cursor = RootCursor | ChildCursor

export const rootCursor: RootCursor = { type: 'root' }
export function childCursor(parent: Cursor, label: GuidId): ChildCursor { return { type: 'child', parent, label } }

export function cursorsEqual(a: Cursor, b: Cursor): boolean {
  if (a.type !== b.type) return false
  if (a.type === 'root') return true
  return (a as ChildCursor).label.equals((b as ChildCursor).label)
    && cursorsEqual((a as ChildCursor).parent, (b as ChildCursor).parent)
}

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
  const node = cursorNode(cursor, gid, root)
  if (node === undefined) return false
  return hasAncestorNode(cursor, node, gid, root)
}

// TODO: O(depth²) - cursorNode re-walks from root at each level. Could cache nodes on descent.
function hasAncestorNode(cursor: Cursor, node: Id, gid: Gid, root: Maybe<Id>): boolean {
  return matchCursor(cursor, {
    root: () => false,
    child: (parent, _) => {
      const parentNode = cursorNode(parent, gid, root)
      return (parentNode !== undefined && node.equals(parentNode)) || hasAncestorNode(parent, node, gid, root)
    }
  })
}
