import * as React from "react"
import { mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { edgeContextFromCursor } from "../editor/edgeContextFromCursor"
import { environment, get, setOrDelete, SourceType } from "../Environment"
import { ID } from "../model/ID"
import { workspaceRootField, workspaceViewField } from "../workspace"
import { defaultRender, tryFirst } from "./defaultRender"
import { descendElement } from "./ProjectionEditors"
import { D, ProjectionContext } from "./ProjectionContext"
import { alwaysFail, Render } from "./R"

export function createProjection(r: Render = alwaysFail) {
  let rootCursor = new Cursor(nothing, environment().workspace.id, workspaceRootField.id)
  let rootEdgeContext: EdgeContext = {
    commit: (id: Maybe<ID>) => setOrDelete(environment().workspace.id, workspaceRootField.id, id),
    expectedType: nothing }
  let rootSourceID = mapMaybe(environment().workspace.root, id =>
    ({id, source: {source: SourceType.DocumentType as SourceType.DocumentType, guid: environment().workspace.id}}))
  let rootDescend = descendElement(rootCursor, tryFirst(r, environment().defaultRender)(rootCursor, rootSourceID, rootEdgeContext), false, rootEdgeContext)
  let viewCursor = new Cursor(nothing, environment().workspace.id, workspaceViewField.id)
  let viewDescend = mapMaybe(get(environment().workspace.id, workspaceViewField.id), viewSourceID =>
    descendElement(viewCursor, environment().defaultRender(viewCursor, viewSourceID, edgeContextFromCursor(viewCursor)), false, edgeContextFromCursor(viewCursor)))
  return {rootDescend, viewDescend}
}

export function ProjectionRoot(props: {d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, edgeContext?: EdgeContext, editorCommands?: EditorCommands}) {
  return <ProjectionContext.Provider value={{
    depth: props.depth,
    scrollParent: props.scrollParent,
    runE: props.runE,
    edgeContext: props.edgeContext,
    editorCommands: props.editorCommands
  }}>{props.d}</ProjectionContext.Provider>
}
