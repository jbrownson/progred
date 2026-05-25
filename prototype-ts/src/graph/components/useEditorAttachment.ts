import * as React from "react"
import { attachEditorCommands, detachEditorCommands, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import { Edge } from "../model/Edge"
import { ID } from "../model/ID"
import type { EditorDescend } from "../render/DContext"

export function useEditorAttachment<A extends HTMLElement>(
  ref: React.RefObject<A | null>,
  commands: EditorCommands,
  focus: {id?: ID, edge?: Edge, descend?: EditorDescend, focusWhenSelected?: boolean, tabStop?: boolean} = {}) {
  React.useLayoutEffect(() => {
    let element = ref.current
    if (!element) return
    attachEditorCommands(element, commands)
    attachEditorFocus(element, {
      id: focus.id,
      edge: focus.edge,
      descend: focus.descend,
      focusWhenSelected: focus.focusWhenSelected,
      tabStop: focus.tabStop })
    return () => {
      detachEditorCommands(element)
      detachEditorFocus(element) }
  })
}
