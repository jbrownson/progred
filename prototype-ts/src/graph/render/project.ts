import { mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { EdgeContext } from "../editor/EditorCommands"
import { edgeContextFromCursor } from "../editor/edgeContextFromCursor"
import { environment, get, setOrDelete, SourceType } from "../Environment"
import { ID } from "../model/ID"
import { workspaceRootField, workspaceViewField } from "../workspace"
import { defaultRender, tryFirst } from "./defaultRender"
import { descendElement } from "./DEditors"
import { alwaysFail, Render } from "./R"

export function createProjection(r: Render = alwaysFail) {
  let rootCursor = new Cursor(nothing, environment().workspace.id, workspaceRootField.id)
  let rootEdgeContext: EdgeContext = {
    commit: (id: Maybe<ID>) => setOrDelete(environment().workspace.id, workspaceRootField.id, id),
    expectedType: nothing,
    fieldName: "root" }
  let rootSourceID = mapMaybe(environment().workspace.root, id =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().workspace.id}}))
  let rootDescend = descendElement(rootCursor, tryFirst(r, environment().defaultRender)(rootCursor, rootSourceID, rootEdgeContext), false, rootEdgeContext)
  let viewCursor = new Cursor(nothing, environment().workspace.id, workspaceViewField.id)
  let viewDescend = mapMaybe(get(environment().workspace.id, workspaceViewField.id), viewSourceID => {
    let viewEdgeContext = {...edgeContextFromCursor(viewCursor), fieldName: "view"}
    return descendElement(viewCursor, environment().defaultRender(viewCursor, viewSourceID, viewEdgeContext), false, viewEdgeContext) })
  return {rootDescend, viewDescend}
}
