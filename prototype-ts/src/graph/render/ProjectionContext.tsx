import * as React from "react"
import { altMaybe, Maybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { ID } from "../model/ID"

export type ProjectionKind = "block" | "line" | "text" | "identicon" | "list" | "descend" | "guidEditor" | "supportsUnderselection" | "label" | "collapsible" | "collapseToggle" | "button" | "placeholderEditor" | "stringEditor" | "numberEditor"
type ProjectionProps = {projectionKind: ProjectionKind, projectionSingleLine: boolean} & Record<string, any>
export type Projection = React.ReactElement<ProjectionProps>
export type D = Projection

export type EditorDescend = {
  cursor: Cursor
  edgeContext: EdgeContext
  unmatching: boolean
}

export type ProjectionContextValue = {
  depth: number
  scrollParent: () => HTMLElement | null
  runE: (f: () => void) => void
  edgeContext?: EdgeContext
  editorCommands?: EditorCommands
  chooseID?: () => Maybe<ID>
  focusCursor?: Cursor
  descend?: EditorDescend
}

export const ProjectionContext = React.createContext<ProjectionContextValue>({
  depth: 0,
  scrollParent: () => null,
  runE: f => f()
})

export function projectionElement<P>(component: React.ComponentType<P>, props: P, kind: ProjectionKind, singleLine: boolean): D {
  return React.createElement(component, {...props, projectionKind: kind, projectionSingleLine: singleLine} as P & ProjectionProps) as D
}

export function projectionKind(d: D): ProjectionKind { return d.props.projectionKind }

export function isSingleLine(d: D): boolean { return d.props.projectionSingleLine }

export function isBlock(d: D): boolean { return projectionKind(d) === "block" }

export function mergeEditorCommands(parentCommands: Maybe<EditorCommands>, childCommands: EditorCommands): EditorCommands {
  let keyDown = parentCommands?.keyDown && childCommands.keyDown
    ? e => altMaybe(childCommands.keyDown!(e), () => parentCommands.keyDown!(e))
    : childCommands.keyDown || parentCommands?.keyDown
  return {
    ...parentCommands,
    ...childCommands,
    ...(keyDown ? {keyDown} : {}) }}

export function activeEditorCommands(edgeContext: Maybe<EdgeContext>, inheritedCommands: Maybe<EditorCommands>, editorCommands: EditorCommands): EditorCommands {
  return {
    ...inheritedCommands,
    ...editorCommands,
    commit: edgeContext?.commit || editorCommands.commit || inheritedCommands?.commit }}

export function childContext(context: ProjectionContextValue, props: Partial<ProjectionContextValue>): ProjectionContextValue {
  return {...context, ...props}
}

export function ProjectionScope(props: {context: ProjectionContextValue, children: React.ReactNode}) {
  return <ProjectionContext.Provider value={props.context}>{props.children}</ProjectionContext.Provider>
}
