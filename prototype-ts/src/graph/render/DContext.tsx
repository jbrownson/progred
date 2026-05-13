import * as React from "react"
import { altMaybe, Maybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { ID } from "../model/ID"

export type DKind = "block" | "line" | "text" | "identicon" | "list" | "descend" | "guidEditor" | "supportsUnderselection" | "label" | "collapsible" | "collapseToggle" | "button" | "placeholderEditor" | "stringEditor" | "numberEditor"
type DProps = {dKind: DKind, dSingleLine: boolean} & Record<string, any>
export type D = React.ReactElement<DProps>

export type EditorDescend = {
  cursor: Cursor
  edgeContext: EdgeContext
  unmatching: boolean
}

export type DContextValue = {
  depth: number
  scrollParent: () => HTMLElement | null
  runE: (f: () => void) => void
  edgeContext?: EdgeContext
  editorCommands?: EditorCommands
  chooseID?: () => Maybe<ID>
  focusCursor?: Cursor
  descend?: EditorDescend
}

export const DContext = React.createContext<DContextValue>({
  depth: 0,
  scrollParent: () => null,
  runE: f => f()
})

export function dElement<P>(component: React.ComponentType<P>, props: P, kind: DKind, singleLine: boolean): D {
  return React.createElement(component, {...props, dKind: kind, dSingleLine: singleLine} as P & DProps) as D
}

export function dKind(d: D): DKind { return d.props.dKind }

export function isSingleLine(d: D): boolean { return d.props.dSingleLine }

export function isBlock(d: D): boolean { return dKind(d) === "block" }

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

export function childContext(context: DContextValue, props: Partial<DContextValue>): DContextValue {
  return {...context, ...props}
}

export function DScope(props: {context: DContextValue, children: React.ReactNode}) {
  return <DContext.Provider value={props.context}>{props.children}</DContext.Provider>
}
