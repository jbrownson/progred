import * as React from "react"
import { Cursor } from "../cursor/Cursor"
import type { EditorDescend } from "../render/ProjectionContext"
import type { StringEditor } from "../render/ProjectionEditors"
import { sidFromString } from "../model/ID"
import { editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { useEditorAttachment } from "./useEditorAttachment"

export function StringEditorComponent(props: {stringEditor: StringEditor, editorCommands: EditorCommands, cursor?: Cursor, descend?: EditorDescend, runE: (f: () => void) => void}) {
  const textArea = React.useRef<HTMLTextAreaElement | null>(null)
  useEditorAttachment(textArea, props.editorCommands, {cursor: props.cursor, descend: props.descend})
  let onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    let keyDownAction = editorKeyDownAction(props.editorCommands, e)
    if (keyDownAction) { props.runE(keyDownAction); return }
    if (!((e.key === "Backspace" || e.key === "Delete") && e.currentTarget.value.length === 0)) stopPropagationForTextInputs(e) }
  return <textarea
    className="string i"
    rows={1}
    wrap="off"
    spellCheck={false}
    onChange={e => { if (props.stringEditor.writable)
      props.runE(() => props.editorCommands.commit?.(sidFromString(e.currentTarget.value)))}}
    value={props.stringEditor.string}
    onClick={e => e.stopPropagation() }
    onKeyDown={onKeyDown}
    ref={textArea} />
}
