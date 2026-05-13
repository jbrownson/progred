import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { D, descendElement } from "./Projection"
import { environment, get, SourceID } from "../Environment"
import { ID } from "../model/ID"
import { typeFromEdge } from "../typeFromEdge"
import { typeMatches } from "../typeMatches"
import { EdgeContext } from "../editor/EditorCommands"
import { edgeContextFromEdge } from "../editor/edgeContextFromCursor"

export type Render = (cursor: Cursor, id: Maybe<SourceID>, edgeContext?: EdgeContext) => Maybe<D>

export const alwaysFail: Render = () => nothing
export function dispatch(...renders: Render[]): Render {
  return (cursor, id, edgeContext) => renders.length === 0 ? nothing : altMaybe(renders[0](cursor, id, edgeContext), () => dispatch(...renders.slice(1))(cursor, id, edgeContext)) }

export type Change = undefined
export type Depedencies = Map/*TODO not actually Map*/<Change, Map<D, () => D>>

export function descend(cursor: Cursor, id: ID, label: ID, render = alwaysFail, edgeContext?: EdgeContext): D {
  let newCursor = _childCursor(cursor, id, label)
  let newSourceID = get(id, label)
  let expectedType = fromMaybe(edgeContext?.expectedType, () => typeFromEdge({parent: id, label}))
  let newEdgeContext = fromMaybe(edgeContext, () => edgeContextFromEdge({parent: id, label}, expectedType))
  return descendElement(newCursor, fromMaybe(render(newCursor, newSourceID, newEdgeContext), () => environment().defaultRender(newCursor, newSourceID, newEdgeContext)),
    fromMaybe(bindMaybe(newSourceID, newSourceID => bindMaybe(expectedType, type => mapMaybe(typeMatches(newSourceID.id, type), typeMatches => !typeMatches))), () => false),
    newEdgeContext) }
