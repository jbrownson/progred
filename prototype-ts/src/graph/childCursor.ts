import { bindMaybe, mapMaybe } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { _get } from "./Environment"
import { ID } from "./ID"

export function childCursor(cursor: Cursor, label: ID) { return mapMaybe(_get(cursor.parent, label), newParent => _childCursor(cursor, newParent, label)) }

export function _childCursor(cursor: Cursor, parent: ID, label: ID) {
  return new Cursor(cursor, parent, label, bindMaybe(cursor.sparseSpanningTree, sparseSpanningTree => sparseSpanningTree.map.get(label))) }