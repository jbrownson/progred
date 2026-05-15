import { nothing, type Maybe } from "../../lib/Maybe"
import type { ID } from "../model/ID"
import { commitToActiveElement } from "./EditorCommands"
import { clearPendingFocus, requestFocusActiveEditor } from "./EditorFocus"

export function commitToActiveElementWithRefocus(id: Maybe<ID>): boolean {
  requestFocusActiveEditor()
  let committed = commitToActiveElement(id)
  if (!committed) clearPendingFocus()
  return committed
}

export function deleteActiveElementWithRefocus(): boolean {
  return commitToActiveElementWithRefocus(nothing)
}
