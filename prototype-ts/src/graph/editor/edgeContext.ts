import { mapMaybe, Maybe } from "../../lib/Maybe"
import { Field, Type } from "../graph"
import { Edge } from "../model/Edge"
import { edges, setOrDelete, SourceType } from "../Environment"
import { guidFromID } from "../model/ID"
import { Commit, EdgeContext } from "./EditorCommands"
import { typeFromEdge } from "../typeFromEdge"

function commitFromEdge(edge: Edge): Maybe<Commit> {
  let source = edges(edge.parent)?.source
  return source === undefined || source.source === SourceType.DocumentType
    ? id => mapMaybe(guidFromID(edge.parent), guid => setOrDelete(guid, edge.label, id))
    : undefined }

export function edgeContextFromEdge(edge: Edge, expectedType: Maybe<Type>): EdgeContext {
  return {
    commit: commitFromEdge(edge),
    expectedType,
    fieldName: mapMaybe(Field.fromID(edge.label), field => field.name) } }

export function edgeContextForEdge(edge: Edge): EdgeContext {
  return edgeContextFromEdge(edge, typeFromEdge(edge))
}
