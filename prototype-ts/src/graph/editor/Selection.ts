import { Cursor } from "../cursor/Cursor"
import { cursorsEqual } from "../cursor/Cursor"
import { NumberEditorState, PlaceholderEditorState } from "../render/D"

export type _Selection = { cursor: Cursor, pendingEdgeLabel?: true } & NumberEditorState & PlaceholderEditorState

export function selectionsEqual(lhs: _Selection, rhs: _Selection): boolean {
  return cursorsEqual(lhs.cursor, rhs.cursor) &&
    lhs.pendingEdgeLabel === rhs.pendingEdgeLabel }
