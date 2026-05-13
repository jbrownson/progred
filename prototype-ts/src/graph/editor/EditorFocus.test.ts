import { afterEach, describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { sidFromString } from "../model/ID"
import { Descend, DText } from "../render/D"
import { attachEditorDescend, attachEditorFocus, focusChildEditor, focusNextTabStop, focusParentEditor, focusPendingEditor, focusSiblingEditor, requestNextTabStopFromCursor } from "./EditorFocus"

afterEach(() => {
  document.body.replaceChildren()
})

function cursor(label: string) {
  return new Cursor(undefined, "guid-parent", sidFromString(label), new SparseSpanningTree())
}

function editor(cursor: Cursor, tabStop = false) {
  const span = document.createElement("span")
  const descend = new Descend(cursor, new DText(String(cursor.label)), false)
  span.tabIndex = 0
  attachEditorDescend(span, descend)
  attachEditorFocus(span, {cursor, descend, tabStop})
  return span
}

describe("EditorFocus", () => {
  it("tabs to placeholder-like tab stops in DOM tree order", () => {
    const root = editor(cursor("root"))
    const filled = editor(cursor("filled"))
    const missing = editor(cursor("missing"), true)
    document.body.appendChild(root)
    root.append(filled, missing)

    root.focus()
    expect(focusNextTabStop(false)).toBe(true)
    expect(document.activeElement).toBe(missing)
    expect(focusNextTabStop(false)).toBe(false)
  })

  it("searches child tab stops before earlier siblings when reverse-tabbing", () => {
    const root = editor(cursor("root"))
    const missing = editor(cursor("missing"), true)
    document.body.appendChild(root)
    root.appendChild(missing)

    root.focus()
    expect(focusNextTabStop(true)).toBe(true)
    expect(document.activeElement).toBe(missing)
  })

  it("walks parent, child, and sibling editor focus nodes", () => {
    const root = editor(cursor("root"))
    const first = editor(cursor("first"))
    const second = editor(cursor("second"))
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

  it("can defer tab navigation until after the target cursor is rendered", () => {
    const rootCursor = cursor("root")
    const missing = editor(cursor("missing"), true)
    requestNextTabStopFromCursor(rootCursor)

    const root = editor(rootCursor)
    document.body.appendChild(root)
    root.appendChild(missing)

    expect(focusPendingEditor(document.body)).toBe(true)
    expect(document.activeElement).toBe(missing)
  })
})
