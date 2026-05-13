import * as React from "react"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, it } from "vitest"
import { noopECallbacks } from "../editor/ECallbacks"
import { attachEditorCommands } from "../editor/EditorCommands"
import { Environment, withEnvironment } from "../Environment"
import { GUIDRootViews } from "../graph"
import type { ID } from "../model/ID"
import { GUIDMap } from "../model/GUIDMap"
import { Cursor } from "../cursor/Cursor"
import { DText, Descend, Label } from "../render/D"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { DComponent } from "./DComponent"

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

describe("DComponent choose ID", () => {
  it("chooses an edge label through the focused editor commands", () => {
    let committed: ID[] = []
    focusedInput(committed)

    let container = document.createElement("div")
    document.body.appendChild(container)
    let root = createRoot(container)
    let labelID = "guid-label"
    let cursor = new Cursor(undefined, "guid-parent", labelID, new SparseSpanningTree())

    act(() => root.render(
      <DComponent
        d={new Label(cursor, new DText("edge label"))}
        depth={0}
        scrollParent={() => null}
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
      new GUIDRootViews("guid-root"),
      new SparseSpanningTree(),
      () => new DText(""),
      noopECallbacks)

    withEnvironment(environment, () => {
      let cursor = new Cursor(undefined, parent, label, new SparseSpanningTree())
      act(() => root.render(
        <DComponent
          d={new Descend(cursor, new DText("node"), false)}
          depth={0}
          scrollParent={() => null}
          runE={f => { f() }} />))

      act(() => clickAlt(textElement(container, "node"))) })

    expect(committed).toEqual([target])

    act(() => root.unmount())
  })
})
