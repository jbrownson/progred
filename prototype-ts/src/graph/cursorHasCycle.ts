import { maybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { ID } from "./ID"

export function cursorHasCycle(cursor: Cursor) {
  function cursorHasEdge(id: ID, label: ID, cursor: Cursor): boolean {
    return cursor.parent === id && cursor.label === label || maybe(cursor.parentCursor, () => false, parentCursor => cursorHasEdge(id, label, parentCursor)) }
  return maybe(cursor.parentCursor, () => false, parentCursor => cursorHasEdge(cursor.parent, cursor.label, parentCursor)) }