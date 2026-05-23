import { bindMaybe, mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { EdgeContext } from "../editor/EditorCommands"
import { edgeContextForEdge } from "../editor/edgeContext"
import { environment, get, setOrDelete, SourceType } from "../Environment"
import { ID } from "../model/ID"
import { workspaceRootField, workspaceViewField } from "../workspace"
import { defaultRender, tryFirst } from "./defaultRender"
import { descendElement } from "./DEditors"
import { alwaysFail, Render } from "./R"
import { emptyCyclePath } from "./CyclePath"
import type { D } from "./DContext"

export function createProjection(r: Render = alwaysFail) {
  let rootEdge = {parent: environment().workspace.id, label: workspaceRootField.id}
  let rootEdgeContext: EdgeContext = {
    commit: (id: Maybe<ID>) => setOrDelete(environment().workspace.id, workspaceRootField.id, id),
    expectedType: nothing,
    fieldName: "root" }
  let rootSourceID = mapMaybe(environment().workspace.root, id =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().workspace.id}}))
  let rootDescend = descendElement(rootEdge, tryFirst(r, environment().defaultRender)(rootEdge, rootSourceID, rootEdgeContext, emptyCyclePath()), false, rootEdgeContext)
  let viewEdge = {parent: environment().workspace.id, label: workspaceViewField.id}
  let viewDescend = mapMaybe(get(environment().workspace.id, workspaceViewField.id), viewSourceID => {
    let viewEdgeContext = {...edgeContextForEdge(viewEdge), fieldName: "view"}
    return descendElement(viewEdge, environment().defaultRender(viewEdge, viewSourceID, viewEdgeContext, emptyCyclePath()), false, viewEdgeContext) })
  return {rootDescend, viewDescend}
}

export function createRootRenderDescend(r: Render, fieldName: string): Maybe<D> {
  let rootEdge = {parent: environment().workspace.id, label: workspaceRootField.id}
  let rootEdgeContext: EdgeContext = {
    commit: (id: Maybe<ID>) => setOrDelete(environment().workspace.id, workspaceRootField.id, id),
    expectedType: nothing,
    fieldName }
  let rootSourceID = mapMaybe(environment().workspace.root, id =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().workspace.id}}))
  return bindMaybe(rootSourceID, rootSourceID =>
    mapMaybe(r(rootEdge, rootSourceID, rootEdgeContext, emptyCyclePath()), d =>
      descendElement(rootEdge, d, false, rootEdgeContext))) }
