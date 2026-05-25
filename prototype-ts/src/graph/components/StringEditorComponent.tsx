import * as React from "react"
import { Edge } from "../model/Edge"
import type { EditorDescend } from "../render/DContext"
import type { StringEditor } from "../render/DEditors"
import { sidFromString } from "../model/ID"
import { editorKeyDownAction, EditorCommands } from "../editor/EditorCommands"
import { stopPropagationForTextInputs } from "../editor/stopPropagationForTextInputs"
import { useEditorAttachment } from "./useEditorAttachment"

export function StringEditorComponent(props: {stringEditor: StringEditor, editorCommands: EditorCommands, edge?: Edge, descend?: EditorDescend, runE: (f: () => void) => void}) {
  const textArea = React.useRef<HTMLTextAreaElement | null>(null)
  useEditorAttachment(textArea, props.editorCommands, {id: props.stringEditor.id, edge: props.edge, descend: props.descend})
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
