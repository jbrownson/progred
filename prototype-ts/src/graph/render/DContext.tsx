import * as React from "react"
import { altMaybe, Maybe, nothing } from "../../lib/Maybe"
import { EdgeContext, EditorCommands, EditorKeyDownEvent } from "../editor/EditorCommands"
import type { Environment } from "../Environment"
import { Edge } from "../model/Edge"
import { ID } from "../model/ID"

export type D = {
  singleLine: boolean
  block: boolean
  node: React.ReactElement
}

export type EditorDescend = {
  edge: Edge
  edgeContext: EdgeContext
  unmatching: boolean
}

export type DContextValue = {
  environment: Environment
  depth: number
  runE: (f: () => void) => void
  edgeContext?: EdgeContext
  editorCommands?: EditorCommands
  chooseID?: () => Maybe<ID>
  descend?: EditorDescend
}

export const DContext = React.createContext<Maybe<DContextValue>>(nothing)

export function useDContext(): DContextValue {
  const context = React.useContext(DContext)
  if (context === nothing) throw new Error("DContext missing")
  return context
}

export function dElement<P extends {}>(component: React.ComponentType<P>, props: P, metadata: {singleLine: boolean, block?: boolean}): D {
  return {singleLine: metadata.singleLine, block: metadata.block || false, node: React.createElement(component, props)}
}

export function renderD(d: D): React.ReactElement { return d.node }

export function isSingleLine(d: D): boolean { return d.singleLine }

export function isBlock(d: D): boolean { return d.block }

export function mergeEditorCommands(parentCommands: Maybe<EditorCommands>, childCommands: EditorCommands): EditorCommands {
  let keyDown = parentCommands?.keyDown && childCommands.keyDown
    ? (e: EditorKeyDownEvent) => altMaybe(childCommands.keyDown!(e), () => parentCommands.keyDown!(e))
    : childCommands.keyDown || parentCommands?.keyDown
  return {
    ...parentCommands,
    ...childCommands,
    ...(keyDown ? {keyDown} : {}) }}

export function activeEditorCommands(edgeContext: Maybe<EdgeContext>, inheritedCommands: Maybe<EditorCommands>, editorCommands: EditorCommands): EditorCommands {
  return {
    ...inheritedCommands,
    ...editorCommands,
    commit: edgeContext && "commit" in edgeContext ? edgeContext.commit : editorCommands.commit || inheritedCommands?.commit }}

export function childContext(context: DContextValue, props: Partial<DContextValue>): DContextValue {
  return {...context, ...props}
}

export function DScope(props: {context: DContextValue, children: React.ReactNode}) {
  return <DContext.Provider value={props.context}>{props.children}</DContext.Provider>
}
