import { mapMaybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { setOrDelete } from "../Environment"
import { guidFromID } from "../model/ID"
import { EdgeContext } from "./EditorCommands"
import { typeFromCursor } from "../cursor/typeFromCursor"

export function edgeContextFromCursor(cursor: Cursor): EdgeContext {
  return {
    commit: id => mapMaybe(guidFromID(cursor.parent), guid => setOrDelete(guid, cursor.label, id)),
    expectedType: typeFromCursor(cursor) } }
