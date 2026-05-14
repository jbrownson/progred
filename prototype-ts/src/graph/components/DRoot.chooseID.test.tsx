import * as React from "react"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, it } from "vitest"
import { noopECallbacks } from "../editor/ECallbacks"
import { attachEditorCommands } from "../editor/EditorCommands"
import { Environment, withEnvironment } from "../Environment"
import type { Edge } from "../model/Edge"
import type { ID } from "../model/ID"
import { GUIDMap } from "../model/GUIDMap"
import { descendElement, dText, label, DRoot } from "../render/D"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

afterEach(() => {
  document.body.replaceChildren()
})

function focusedInput(committed: ID[]) {
  let input = document.createElement("input")
  document.body.appendChild(input)
  attachEditorCommands(input, {commit: id => { if (id !== undefined) committed.push(id) }})
  input.focus()
}

function clickAlt(element: Element) {
  element.dispatchEvent(new MouseEvent("mousedown", {bubbles: true, cancelable: true, altKey: true}))
  element.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true, altKey: true}))
}

function textElement(container: HTMLElement, text: string) {
  let element = Array.from(container.querySelectorAll("span")).find(element => element.textContent === text && element.children.length === 0)
  expect(element).not.toBe(undefined)
  return element as HTMLElement
}

function edge(parent: ID, label: ID): Edge {
  return {parent, label}
}

describe("DRoot choose ID", () => {
  it("chooses an edge label through the focused editor commands", () => {
    let committed: ID[] = []
    focusedInput(committed)

    let container = document.createElement("div")
    document.body.appendChild(container)
    let root = createRoot(container)
    let labelID = "guid-label"

    act(() => root.render(
      <DRoot
        d={label(edge("guid-parent", labelID), dText("edge label"))}
        depth={0}
        runE={f => { f() }} />))

    act(() => clickAlt(textElement(container, "edge label")))

    expect(committed).toEqual([labelID])

    act(() => root.unmount())
  })

  it("chooses a descended node through the focused editor commands", () => {
    let committed: ID[] = []
    focusedInput(committed)

    let container = document.createElement("div")
    document.body.appendChild(container)
    let root = createRoot(container)
    let parent = "guid-parent"
    let label = "guid-label"
    let target = "guid-target"
    let environment = new Environment(
      new Map(),
      new GUIDMap(new Map([[parent, new Map([[label, target]])]])),
      {id: "guid-workspace", root: undefined, view: undefined},
      () => dText(""),
      noopECallbacks)

    withEnvironment(environment, () => {
      act(() => root.render(
        <DRoot
          d={descendElement(edge(parent, label), dText("node"), false)}
          depth={0}
          runE={f => { f() }} />))

      act(() => clickAlt(textElement(container, "node"))) })

    expect(committed).toEqual([target])

    act(() => root.unmount())
  })
})
