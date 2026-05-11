import * as React from "react"
import { act } from "react"
import { createRoot, Root } from "react-dom/client"
import { afterEach, describe, expect, it } from "vitest"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import type { CopyResult } from "../editor/Copy"
import { commitIDToActiveElement, editorCommandsForActiveElement } from "../editor/EditorCommands"
import { idFromClipboardText } from "../editor/Clipboard"
import { _get, Environment, withEnvironment } from "../Environment"
import { appCtor, checkString, ctorCtor, ctorField, fieldCtor, GUIDApp, GUIDDescend, GUIDLine, GUIDRenderCtor, headField, nameField, rootField, tailField, viewsField } from "../graph"
import { ID, sidFromID, sidFromString, stringFromID } from "../model/ID"
import { DComponent } from "./DComponent"
import { createD, Descend } from "../render/D"
import { defaultRender, tryFirst } from "../render/defaultRender"
import { renderIfApp } from "../renderIfs"
import { renderFromRender } from "../render/renderFromRender"
import { Render } from "../render/R"
import { makeTestEnvironment } from "../testHelpers"
import { SparseSpanningTree } from "../SparseSpanningTree"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

afterEach(() => {
  document.body.replaceChildren()
})

function setNativeValue(element: HTMLInputElement | HTMLTextAreaElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")?.set
  setter?.call(element, value)
}

function input(element: HTMLInputElement | HTMLTextAreaElement, value: string) {
  setNativeValue(element, value)
  element.dispatchEvent(new Event("input", {bubbles: true}))
}

function keyDown(element: Element, key: string) {
  element.dispatchEvent(new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true}))
}

class EditorHarness {
  container = document.createElement("div")
  root: Root
  rootDescend: Descend

  constructor(public environment: Environment) {
    document.body.appendChild(this.container)
    this.root = createRoot(this.container)
    act(() => this.render())
  }

  render() {
    withEnvironment(this.environment, () => {
      const {rootDescend} = createD()
      this.rootDescend = rootDescend
      this.root.render(<DComponent
        d={rootDescend}
        depth={0}
        scrollParent={() => this.container}
        runE={f => {
          f()
          this.render() }} />)
    })
  }

  run(f: () => void) {
    withEnvironment(this.environment, () => act(f))
  }

  textInput() {
    const input = this.container.querySelector("input[type=text], textarea") as HTMLInputElement | HTMLTextAreaElement | null
    expect(input).not.toBe(null)
    return input!
  }

  typeAndEnter(value: string) {
    const textInput = this.textInput()
    this.run(() => {
      input(textInput, value)
      keyDown(textInput, "Enter") })
  }

  key(key: string) {
    const textInput = this.textInput()
    this.run(() => keyDown(textInput, key))
  }

  get(parent: ID, label: ID) {
    return withEnvironment(this.environment, () => _get(parent, label))
  }

  unmount() {
    act(() => this.root.unmount())
  }
}

function rootCursor(environment: Environment) {
  if (!environment.sparseSpanningTree.map.has(rootField.id))
    environment.sparseSpanningTree.map.set(rootField.id, new SparseSpanningTree())
  if (!environment.sparseSpanningTree.map.has(viewsField.id))
    environment.sparseSpanningTree.map.set(viewsField.id, new SparseSpanningTree())
  return new Cursor(undefined, environment.rootViews.id, rootField.id, environment.sparseSpanningTree.map.get(rootField.id))
}

function rootHarness(render?: Render) {
  const environment = makeTestEnvironment({defaultRender: render ? tryFirst(render, defaultRender) : defaultRender})
  environment.selection = {cursor: rootCursor(environment)}
  return new EditorHarness(environment)
}

describe("DComponent editor integration", () => {
  it("commits a default-rendered root placeholder by typing and pressing Enter", () => {
    const harness = rootHarness()

    harness.typeAndEnter("random node")

    const root = harness.get(harness.environment.rootViews.id, rootField.id)
    expect(root).not.toBe(undefined)
    expect(sidFromID(root!)).toBe(undefined)

    harness.unmount()
  })

  it("commits a string through a default-rendered placeholder", () => {
    const harness = rootHarness()

    harness.typeAndEnter("hello")

    expect(stringFromID(harness.get(harness.environment.rootViews.id, rootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("edits an existing string through the textarea path", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("old"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.run(() => input(harness.textInput(), "new"))

    expect(stringFromID(harness.get(environment.rootViews.id, rootField.id)!)).toBe("new")

    harness.unmount()
  })

  it("edits an existing number through input and Enter", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, 1)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    const textInput = harness.textInput()
    harness.run(() => {
      input(textInput, "42")
      keyDown(textInput, "Enter") })

    expect(harness.get(environment.rootViews.id, rootField.id)).toBe(42)

    harness.unmount()
  })

  it("creates a list from a selected placeholder with the keyboard", () => {
    const harness = rootHarness()

    harness.key("[")

    const list = harness.get(harness.environment.rootViews.id, rootField.id)
    expect(list).not.toBe(undefined)
    expect(harness.get(list!, headField.id)).toBe(undefined)
    expect(harness.get(list!, tailField.id)).not.toBe(undefined)
    expect(harness.environment.selection?.cursor.parent).toBe(list)
    expect(harness.environment.selection?.cursor.label).toBe(headField.id)

    harness.unmount()
  })

  it("commits a list item through the placeholder created by list insertion", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("hello")

    const list = harness.get(harness.environment.rootViews.id, rootField.id)
    expect(stringFromID(harness.get(list!, headField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("uses pending edge-label selection and then commits the target placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const node = "guid-node"
    environment.guidMap.set(environment.rootViews.id, rootField.id, node)
    environment.selection = {cursor: rootCursor(environment), pendingEdgeLabel: true}
    const harness = new EditorHarness(environment)

    harness.typeAndEnter("random node")
    const label = environment.selection?.cursor.label
    expect(label).not.toBe(undefined)
    expect(environment.selection?.pendingEdgeLabel).toBe(undefined)
    expect(environment.selection?.cursor.parent).toBe(node)

    harness.typeAndEnter("hello")

    expect(stringFromID(harness.get(node, label!)!)).toBe("hello")

    harness.unmount()
  })

  it("pastes a reference into the focused placeholder through editor commands", () => {
    const harness = rootHarness()
    const pastedID = idFromClipboardText(JSON.stringify({id: "guid-pasted"}))

    harness.run(() => {
      expect(pastedID).not.toBe(undefined)
      commitIDToActiveElement(pastedID!) })

    expect(harness.get(harness.environment.rootViews.id, rootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("copies the focused guid editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, "guid-node")
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    let copy: {referenceID: ID, copyResult: CopyResult} | undefined
    harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })

    expect(copy?.referenceID).toBe("guid-node")
    expect(copy?.copyResult.root).not.toBe("guid-node")

    harness.unmount()
  })

  it("edits a field exposed by a generated custom render", () => {
    const appRender = renderIfApp(name => name)
    const environment = makeTestEnvironment({defaultRender: tryFirst(appRender, defaultRender)})
    const app = withEnvironment(environment, () => GUIDApp.new())
    environment.guidMap.set(environment.rootViews.id, rootField.id, app.id)
    environment.selection = {cursor: _childCursor(rootCursor(environment), app.id, nameField.id)}
    const harness = new EditorHarness(environment)

    harness.typeAndEnter("Widget")

    expect(stringFromID(harness.get(app.id, nameField.id)!)).toBe("Widget")

    harness.unmount()
  })

  it("edits a field exposed by an in-document render template", () => {
    const environment = makeTestEnvironment()
    const {app, render} = withEnvironment(environment, () => {
      environment.guidMap.set(appCtor.id, ctorField.id, ctorCtor.id)
      environment.guidMap.set(nameField.id, ctorField.id, fieldCtor.id)
      const template = GUIDRenderCtor.new()
      const line = GUIDLine.new()
      const descend = GUIDDescend.new()
      descend.setField(nameField)
      line.setChildren([checkString(sidFromString("App "))!, descend])
      template.setForCtor(appCtor)
      template.setD(line)
      const render = renderFromRender(template)
      expect(render).not.toBe(undefined)
      return {app: GUIDApp.new(), render: render!} })
    environment.defaultRender = tryFirst(render, defaultRender)
    environment.guidMap.set(environment.rootViews.id, rootField.id, app.id)
    environment.selection = {cursor: _childCursor(rootCursor(environment), app.id, nameField.id)}
    const harness = new EditorHarness(environment)

    harness.typeAndEnter("Templated")

    expect(stringFromID(harness.get(app.id, nameField.id)!)).toBe("Templated")

    harness.unmount()
  })
})
