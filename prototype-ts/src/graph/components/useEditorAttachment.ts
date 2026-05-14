import * as React from "react"
import { attachEditorCommands, detachEditorCommands, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import { Edge } from "../model/Edge"
import type { EditorDescend } from "../render/DContext"

export function useEditorAttachment<A extends HTMLElement>(
  ref: React.RefObject<A | null>,
  commands: EditorCommands,
  focus: {edge?: Edge, descend?: EditorDescend, activate?: () => void, focusWhenSelected?: boolean, tabStop?: boolean} = {}) {
  React.useLayoutEffect(() => {
    let element = ref.current
    if (!element) return
    attachEditorCommands(element, commands)
    attachEditorFocus(element, {
      edge: focus.edge,
      descend: focus.descend,
      activate: focus.activate,
      focusWhenSelected: focus.focusWhenSelected,
      tabStop: focus.tabStop })
    return () => {
      detachEditorCommands(element)
      detachEditorFocus(element) }
  })
}
