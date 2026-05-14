import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import type { D } from "./DContext"
import { descendElement } from "./DEditors"
import { environment, get, SourceID } from "../Environment"
import { Edge } from "../model/Edge"
import { ID } from "../model/ID"
import { typeFromEdge } from "../typeFromEdge"
import { typeMatches } from "../typeMatches"
import { EdgeContext } from "../editor/EditorCommands"
import { edgeContextFromEdge } from "../editor/edgeContext"
import { emptyCyclePath, stepCyclePath, type CyclePath } from "./CyclePath"

export type Render = (edge: Edge, id: Maybe<SourceID>, edgeContext?: EdgeContext, cyclePath?: CyclePath) => Maybe<D>

export const alwaysFail: Render = () => nothing
export function dispatch(...renders: Render[]): Render {
  return (edge, id, edgeContext, cyclePath) => renders.length === 0 ? nothing : altMaybe(renders[0](edge, id, edgeContext, cyclePath), () => dispatch(...renders.slice(1))(edge, id, edgeContext, cyclePath)) }

export function descend(id: ID, label: ID, render = alwaysFail, edgeContext?: EdgeContext, cyclePath: CyclePath = emptyCyclePath()): D {
  let edge = {parent: id, label}
  let newSourceID = get(id, label)
  let expectedType = fromMaybe(edgeContext?.expectedType, () => typeFromEdge(edge))
  let newEdgeContext = fromMaybe(edgeContext, () => edgeContextFromEdge(edge, expectedType))
  let childCyclePath = stepCyclePath(cyclePath, id).path
  return descendElement(edge, fromMaybe(render(edge, newSourceID, newEdgeContext, childCyclePath), () => environment().defaultRender(edge, newSourceID, newEdgeContext, childCyclePath)),
    fromMaybe(bindMaybe(newSourceID, newSourceID => bindMaybe(expectedType, type => mapMaybe(typeMatches(newSourceID.id, type), typeMatches => !typeMatches))), () => false),
    newEdgeContext) }
