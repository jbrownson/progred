import { assert } from "../lib/assert"
import { mapMaybe, Maybe, maybe, maybesEqual, nothing } from "../lib/Maybe"
import { _get, environment, logID } from "./Environment"
import { ID } from "./ID"
import { SparseSpanningTree } from "./SparseSpanningTree"

export class Cursor {
  constructor(public parentCursor: Maybe<Cursor>, public parent: ID, public label: ID, public sparseSpanningTree: Maybe<SparseSpanningTree>) { assert(parentCursor !== nothing || sparseSpanningTree !== nothing) } }

export function cursorsEqual(a: Cursor, b: Cursor): boolean {
  return a === b || a.label === b.label && a.parent === b.parent && maybesEqual(a.parentCursor, b.parentCursor, cursorsEqual) }

function validateCursorImpl(cursor: Cursor, child: ID): boolean {
  mapMaybe(cursor.parentCursor, validateCursor)
  return _get(cursor.parent, cursor.label) === child }

export function validateCursor(cursor: Cursor) {
  maybe(
    cursor.parentCursor,
    () => {
      let e = environment()
      assert(cursor.parent === e.rootViews.id) },
    (parentCursor) => {
    if (!validateCursorImpl(parentCursor, cursor.parent)) {
      console.log("============ INVALID CURSOR ============")
      if (_get(parentCursor.parent, parentCursor.label) === nothing) {
        console.log("LABEL NOT FOUND ON NODE")
      } else {
        console.log("CHILD MISMATCHES ONE FOUND VIA LABEL") }
      console.log("PARENT =================================")
      logID(parentCursor.parent)
      console.log("LABEL ==================================")
      logID(parentCursor.label)
      console.log("CHHILD =================================")
      logID(cursor.parent)
      assert(false) }}) }