import { afterEach, describe, expect, it } from "vitest"
import { sidFromString } from "../model/ID"
import type { Edge } from "../model/Edge"
import type { EditorDescend } from "../render/DContext"
import { attachEditorDescend, attachEditorFocus, focusChildEditor, focusNextTabStop, focusParentEditor, focusPendingEditor, focusSiblingEditor, requestNextTabStopFromActiveElement } from "./EditorFocus"

afterEach(() => {
  document.body.replaceChildren()
})

function edge(label: string): Edge {
  return {parent: "guid-parent", label: sidFromString(label)}
}

function editor(edge: Edge, tabStop = false) {
  const span = document.createElement("span")
  const descend: EditorDescend = {edge, edgeContext: {}, unmatching: false}
  span.tabIndex = 0
  attachEditorDescend(span, descend)
  attachEditorFocus(span, {edge, descend, tabStop})
  return span
}

describe("EditorFocus", () => {
  it("tabs to placeholder-like tab stops in DOM tree order", () => {
    const root = editor(edge("root"))
    const filled = editor(edge("filled"))
    const missing = editor(edge("missing"), true)
    document.body.appendChild(root)
    root.append(filled, missing)

    root.focus()
    expect(focusNextTabStop(false)).toBe(true)
    expect(document.activeElement).toBe(missing)
    expect(focusNextTabStop(false)).toBe(false)
  })

  it("searches child tab stops before earlier siblings when reverse-tabbing", () => {
    const root = editor(edge("root"))
    const missing = editor(edge("missing"), true)
    document.body.appendChild(root)
    root.appendChild(missing)

    root.focus()
    expect(focusNextTabStop(true)).toBe(true)
    expect(document.activeElement).toBe(missing)
  })

  it("walks parent, child, and sibling editor focus nodes", () => {
    const root = editor(edge("root"))
    const first = editor(edge("first"))
    const second = editor(edge("second"))
    document.body.appendChild(root)
    root.append(first, second)

    root.focus()
    expect(focusChildEditor()).toBe(true)
    expect(document.activeElement).toBe(first)
    expect(focusSiblingEditor(1)).toBe(true)
    expect(document.activeElement).toBe(second)
    expect(focusParentEditor()).toBe(true)
    expect(document.activeElement).toBe(root)
  })

  it("can defer tab navigation from the active editor until after rerender", () => {
    const missing = editor(edge("missing"), true)
    const root = editor(edge("root"))
    document.body.appendChild(root)
    root.focus()
    requestNextTabStopFromActiveElement()
    root.appendChild(missing)

    expect(focusPendingEditor(document.body)).toBe(true)
    expect(document.activeElement).toBe(missing)
  })
})
