import { afterEach, describe, expect, it } from "vitest"
import type { ID } from "../model/ID"
import { attachEditorCommands, commitIDToActiveElement, commitToActiveElement, detachEditorCommands, editorCommandsForActiveElement } from "./EditorCommands"

afterEach(() => {
  document.body.replaceChildren()
})

describe("EditorCommands", () => {
  it("commits an ID through the active element", () => {
    let input = document.createElement("input")
    document.body.appendChild(input)

    let committed: ID[] = []
    attachEditorCommands(input, {commit: id => { if (id !== undefined) committed.push(id) }})
    input.focus()

    expect(editorCommandsForActiveElement()).not.toBe(undefined)
    expect(commitIDToActiveElement("guid-target")).toBe(true)
    expect(committed).toEqual(["guid-target"])
  })

  it("does not commit when the active element has no commit command", () => {
    let input = document.createElement("input")
    document.body.appendChild(input)

    attachEditorCommands(input, {})
    input.focus()

    expect(commitIDToActiveElement("guid-target")).toBe(false)
  })

  it("detaches commands from an element", () => {
    let input = document.createElement("input")
    document.body.appendChild(input)

    let committed: ID[] = []
    attachEditorCommands(input, {commit: id => { if (id !== undefined) committed.push(id) }})
    input.focus()
    detachEditorCommands(input)

    expect(editorCommandsForActiveElement()).toBe(undefined)
    expect(commitIDToActiveElement("guid-target")).toBe(false)
    expect(committed).toEqual([])
  })

  it("commits undefined through the active element", () => {
    let input = document.createElement("input")
    document.body.appendChild(input)

    let committed: (ID | undefined)[] = []
    attachEditorCommands(input, {commit: id => committed.push(id)})
    input.focus()

    expect(commitToActiveElement(undefined)).toBe(true)
    expect(committed).toEqual([undefined])
  })
})
