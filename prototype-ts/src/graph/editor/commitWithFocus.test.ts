import { afterEach, describe, expect, it } from "vitest"
import type { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import type { EditorDescend } from "../render/DContext"
import { attachEditorCommands } from "./EditorCommands"
import { attachEditorDescend, attachEditorFocus, focusPendingEditor } from "./EditorFocus"
import { commitToActiveElementWithRefocus } from "./commitWithFocus"

afterEach(() => {
  document.body.replaceChildren()
})

function edge(label: string): Edge {
  return {parent: "guid-parent", label: sidFromString(label)}
}

function editor(edge: Edge) {
  const span = document.createElement("span")
  const descend: EditorDescend = {edge, edgeContext: {}, unmatching: false}
  span.tabIndex = 0
  attachEditorDescend(span, descend)
  attachEditorFocus(span, {edge, descend})
  return span
}

describe("commitWithFocus", () => {
  it("can commit and defer refocusing the replacement editor", () => {
    const oldRoot = editor(edge("root"))
    const committed: unknown[] = []
    attachEditorCommands(oldRoot, {commit: id => committed.push(id)})
    document.body.appendChild(oldRoot)
    oldRoot.focus()

    expect(commitToActiveElementWithRefocus("guid-target")).toBe(true)
    const newRoot = editor(edge("root"))
    document.body.replaceChildren(newRoot)

    expect(committed).toEqual(["guid-target"])
    expect(focusPendingEditor(document.body)).toBe(true)
    expect(document.activeElement).toBe(newRoot)
  })
})

