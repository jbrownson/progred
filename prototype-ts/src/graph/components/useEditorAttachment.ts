import * as React from "react"
import { mapMaybe } from "../../lib/Maybe"
import { Cursor } from "../cursor/Cursor"
import { attachEditorCommands, detachEditorCommands, EditorCommands } from "../editor/EditorCommands"
import { attachEditorFocus, detachEditorFocus } from "../editor/EditorFocus"
import type { EditorDescend } from "../render/ProjectionContext"

export function useEditorAttachment<A extends HTMLElement>(
  ref: React.RefObject<A | null>,
  commands: EditorCommands,
  focus: {cursor?: Cursor, descend?: EditorDescend, activate?: () => void, focusWhenSelected?: boolean, tabStop?: boolean} = {}) {
  React.useLayoutEffect(() => {
    let element = ref.current
    if (!element) return
    attachEditorCommands(element, commands)
    mapMaybe(focus.cursor, cursor => attachEditorFocus(element, {
      cursor,
      descend: focus.descend,
      activate: focus.activate,
      focusWhenSelected: focus.focusWhenSelected,
      tabStop: focus.tabStop }))
    return () => {
      detachEditorCommands(element)
      detachEditorFocus(element) }
  })
}
