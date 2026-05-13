import * as React from "react"
import { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import { D, DContext } from "./DContext"

export function DRoot(props: {d: D, depth: number, scrollParent: () => HTMLElement | null, runE: (f: () => void) => void, edgeContext?: EdgeContext, editorCommands?: EditorCommands}) {
  return <DContext.Provider value={{
    depth: props.depth,
    scrollParent: props.scrollParent,
    runE: props.runE,
    edgeContext: props.edgeContext,
    editorCommands: props.editorCommands
  }}>{props.d}</DContext.Provider>
}
