import { maybe, nothing } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { _get, environment, set } from "../Environment"
import { guidFromID, ID } from "../model/ID"

export function chooseIDForSelection(id: ID): boolean {
  return maybe(environment().selection, () => false, selection => {
    if (selection.pendingEdgeLabel) {
      return maybe(_get(selection.cursor.parent, selection.cursor.label), () => false, selectedID =>
        maybe(guidFromID(selectedID), () => false, guid => {
          environment().selection = {cursor: _childCursor(selection.cursor, guid, id)}
          return true })) }
    if (_get(selection.cursor.parent, selection.cursor.label) !== nothing) return false
    return maybe(guidFromID(selection.cursor.parent), () => false, guid => {
      set(guid, selection.cursor.label, id)
      return true }) }) }
