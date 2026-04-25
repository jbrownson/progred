import { bindMaybe, maybeToArray, nothing } from "../../lib/Maybe"
import { Cursor, cursorsEqual } from "../cursor/Cursor"
import { environment } from "../Environment"
import { ID } from "../model/ID"

export function selectedMissingLabels(cursor: Cursor, id: ID, renderedLabels: ID[]): ID[] {
  return maybeToArray(bindMaybe(environment().selection, selection => {
    if (selection.pendingEdgeLabel) return nothing
    const selectedCursor = selection.cursor
    return bindMaybe(selectedCursor.parentCursor, parentCursor =>
      selectedCursor.parent === id &&
      cursorsEqual(parentCursor, cursor) &&
      !renderedLabels.includes(selectedCursor.label)
        ? selectedCursor.label
        : nothing) }))
}
