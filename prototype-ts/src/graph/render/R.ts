import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import type { D } from "./DContext"
import { descendElement } from "./DEditors"
import { environment, get, SourceID } from "../Environment"
import { ID } from "../model/ID"
import { typeFromEdge } from "../typeFromEdge"
import { typeMatches } from "../typeMatches"
import { EdgeContext } from "../editor/EditorCommands"
import { edgeContextFromEdge } from "../editor/edgeContext"
import { emptyCyclePath, stepCyclePath, type CyclePath } from "./CyclePath"

export type Render = (cursor: Cursor, id: Maybe<SourceID>, edgeContext?: EdgeContext, cyclePath?: CyclePath) => Maybe<D>

export const alwaysFail: Render = () => nothing
export function dispatch(...renders: Render[]): Render {
  return (cursor, id, edgeContext, cyclePath) => renders.length === 0 ? nothing : altMaybe(renders[0](cursor, id, edgeContext, cyclePath), () => dispatch(...renders.slice(1))(cursor, id, edgeContext, cyclePath)) }

export function descend(cursor: Cursor, id: ID, label: ID, render = alwaysFail, edgeContext?: EdgeContext, cyclePath: CyclePath = emptyCyclePath()): D {
  let newCursor = _childCursor(cursor, id, label)
  let newSourceID = get(id, label)
  let expectedType = fromMaybe(edgeContext?.expectedType, () => typeFromEdge({parent: id, label}))
  let newEdgeContext = fromMaybe(edgeContext, () => edgeContextFromEdge({parent: id, label}, expectedType))
  let childCyclePath = stepCyclePath(cyclePath, id).path
  return descendElement(newCursor, fromMaybe(render(newCursor, newSourceID, newEdgeContext, childCyclePath), () => environment().defaultRender(newCursor, newSourceID, newEdgeContext, childCyclePath)),
    fromMaybe(bindMaybe(newSourceID, newSourceID => bindMaybe(expectedType, type => mapMaybe(typeMatches(newSourceID.id, type), typeMatches => !typeMatches))), () => false),
    newEdgeContext) }
