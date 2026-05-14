import { mapMaybe, Maybe } from "../../lib/Maybe"
import { Field, Type } from "../graph"
import { Edge } from "../model/Edge"
import { setOrDelete } from "../Environment"
import { guidFromID } from "../model/ID"
import { EdgeContext } from "./EditorCommands"
import { typeFromEdge } from "../typeFromEdge"

export function edgeContextFromEdge(edge: Edge, expectedType: Maybe<Type>): EdgeContext {
  return {
    commit: id => mapMaybe(guidFromID(edge.parent), guid => setOrDelete(guid, edge.label, id)),
    expectedType,
    fieldName: mapMaybe(Field.fromID(edge.label), field => field.name) } }

export function edgeContextForEdge(edge: Edge): EdgeContext {
  return edgeContextFromEdge(edge, typeFromEdge(edge))
}
