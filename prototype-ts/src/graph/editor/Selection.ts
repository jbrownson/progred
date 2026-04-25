import { Cursor } from "../cursor/Cursor"
import { cursorsEqual } from "../cursor/Cursor"
import { NumberEditorState, PlaceholderState } from "../render/D"

export type _Selection = { cursor: Cursor, pendingEdgeLabel?: true } & NumberEditorState & PlaceholderState

export function selectionsEqual(lhs: _Selection, rhs: _Selection): boolean {
  return cursorsEqual(lhs.cursor, rhs.cursor) &&
    lhs.pendingEdgeLabel === rhs.pendingEdgeLabel }
