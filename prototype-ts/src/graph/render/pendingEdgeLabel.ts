import { mapMaybe, maybe } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { buildEdgeLabelEntries } from "../editor/buildEntries"
import { selectionIfSelected } from "../editor/selectionIfSelected"
import { environment } from "../Environment"
import { GUID } from "../model/ID"
import { Block, D, DText, Line, PlaceholderEditor } from "./D"

export function pendingEdgeLabel(cursor: Cursor, guid: GUID): D[] {
  return maybe(selectionIfSelected(cursor), () => [], selection =>
    selection.pendingEdgeLabel
      ? [new Block(new Line(
        new PlaceholderEditor("label", {
          entries: buildEdgeLabelEntries(id => {
            environment().selection = {cursor: _childCursor(cursor, guid, id())} }),
          editorState: selection },
          {commit: id => mapMaybe(id, id => environment().selection = {cursor: _childCursor(cursor, guid, id)})}),
        new DText(" →"))) ]
      : []) }
