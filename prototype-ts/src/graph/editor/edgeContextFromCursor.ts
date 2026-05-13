import { mapMaybe, Maybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { Type } from "../graph"
import { EdgeRef } from "../model/EdgeRef"
import { setOrDelete } from "../Environment"
import { guidFromID } from "../model/ID"
import { EdgeContext } from "./EditorCommands"
import { typeFromEdge } from "../typeFromEdge"

export function edgeContextFromEdge(edge: EdgeRef, expectedType: Maybe<Type>): EdgeContext {
  return {
    commit: id => mapMaybe(guidFromID(edge.parent), guid => setOrDelete(guid, edge.label, id)),
    expectedType } }

export function edgeContextFromCursor(cursor: Cursor): EdgeContext {
  return edgeContextFromEdge(cursor, typeFromEdge(cursor)) }
