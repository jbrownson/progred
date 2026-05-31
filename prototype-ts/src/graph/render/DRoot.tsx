import * as React from "react"
import { EdgeContext, EditorCommands } from "../editor/EditorCommands"
import type { Environment } from "../Environment"
import type { ID } from "../model/ID"
import { D, DContext, renderD } from "./DContext"

export function DRoot(props: {d: D, environment: Environment, depth: number, runE: (f: () => void) => void, secondarySelectionID?: ID, edgeContext?: EdgeContext, editorCommands?: EditorCommands}) {
  return <DContext.Provider value={{
    environment: props.environment,
    depth: props.depth,
    runE: props.runE,
    secondarySelectionID: props.secondarySelectionID,
    edgeContext: props.edgeContext,
    editorCommands: props.editorCommands
  }}>{renderD(props.d)}</DContext.Provider>
}
