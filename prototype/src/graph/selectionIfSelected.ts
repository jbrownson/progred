import { bindMaybe, maybe, nothing } from "../lib/Maybe"
import { Cursor, cursorsEqual } from "./Cursor"
import { _get, environment } from "./Environment"
import { _Selection } from "./Selection"

export const enum SelectionState { Selected, Hinted }

export function selectionIfSelected(cursor: Cursor) {
  return bindMaybe(environment().selection, selection => bindMaybe(selection, selection => selectionMatchesCursor(selection, cursor) ? selection : nothing ))}

function selectionMatchesCursor(selection: _Selection, cursor: Cursor) { return cursorsEqual(selection.cursor, cursor) }

export function selectionStateFromCursor(cursor: Cursor) { return bindMaybe(environment().selection, selection =>
  selectionMatchesCursor(selection, cursor) ? SelectionState.Selected : cursorsEndTheSame(selection.cursor, cursor) ? SelectionState.Hinted : nothing )}

function cursorsEndTheSame(cursorA: Cursor, cursorB: Cursor): boolean {
  return maybe(_get(cursorA.parent, cursorA.label), () => false, idA => idA === _get(cursorB.parent, cursorB.label)) }