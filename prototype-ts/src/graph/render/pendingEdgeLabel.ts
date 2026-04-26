import { maybe } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { selectionIfSelected } from "../editor/selectionIfSelected"
import { environment } from "../Environment"
import { GUID } from "../model/ID"
import { Block, D, DText, Line, Placeholder } from "./D"

export function pendingEdgeLabel(cursor: Cursor, guid: GUID): D[] {
  return maybe(selectionIfSelected(cursor), () => [], selection =>
    selection.pendingEdgeLabel
      ? [new Block(new Line(
        new Placeholder("label", {
          entries: buildEdgeLabelEntries(id => {
            environment().selection = {cursor: _childCursor(cursor, guid, id())} }),
          placeholderState: selection }),
        new DText(" →"))) ]
      : []) }
