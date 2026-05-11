import * as React from "react"
import { act } from "react"
import { createRoot, Root } from "react-dom/client"
import { afterEach, describe, expect, it } from "vitest"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import type { CopyResult } from "../editor/Copy"
import { undoRedoECallbacks } from "../editor/ECallbacks"
import { commitIDToActiveElement, editorCommandsForActiveElement } from "../editor/EditorCommands"
import { clipboardStringForCopyResult, copyIDFromClipboardText, idFromClipboardText } from "../editor/Clipboard"
import { _get, Environment, withEnvironment } from "../Environment"
import { appCtor, checkString, ctorCtor, ctorField, emptyListCtor, evaluateCtor, fieldCtor, fieldsField, functionDeclarationCtor, GUIDApp, GUIDDescend, GUIDField, GUIDLine, GUIDRenderCtor, headField, javascriptProgramCtor, javascriptProgramField, nameField, nonemptyListCtor, rootField, statementsField, tailField, viewsField } from "../graph"
import { ID, sidFromID, sidFromString, stringFromID } from "../model/ID"
import { DComponent } from "./DComponent"
import { createD, Descend } from "../render/D"
import { defaultRender, tryFirst } from "../render/defaultRender"
import { renderIfApp } from "../renderIfs"
import { renderFromRender } from "../render/renderFromRender"
import { Render } from "../render/R"
import { makeTestEnvironment } from "../testHelpers"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { defaultKeyHandler } from "../editor/keyHandler"
import { MapIDMap } from "../model/MapIDMap"
import type { UndoRedo } from "../editor/UndoRedo"

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

function keyDown(element: Element, key: string, options: KeyboardEventInit = {}) {
  element.dispatchEvent(new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options}))
}

function click(element: Element, options: MouseEventInit = {}) {
  element.dispatchEvent(new MouseEvent("mousedown", {bubbles: true, cancelable: true, ...options}))
  element.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true, ...options}))
}

class EditorHarness {
  container = document.createElement("div")
  root: Root
  rootDescend: Descend
  undoStack: UndoRedo[][] = []
  redoStack: UndoRedo[][] = []

  constructor(public environment: Environment) {
    document.body.appendChild(this.container)
    this.root = createRoot(this.container)
    rootCursor(environment)
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
          this.runWithUndoCallbacks(f)
          this.render() }} />)
    })
  }

  runWithUndoCallbacks<A>(f: () => A): A {
    const {undoRedoArray, eCallbacks} = undoRedoECallbacks()
    const oldCallbacks = this.environment.callbacks
    this.environment.callbacks = eCallbacks
    try {
      const result = f()
      if (undoRedoArray.find(undoRedo => !undoRedo.selectionAction)) {
        this.undoStack.push(undoRedoArray)
        this.redoStack = [] }
      return result
    } finally {
      this.environment.callbacks = oldCallbacks }}

  run<A>(f: () => A): A {
    let result: A
    withEnvironment(this.environment, () => act(() => { result = f() }))
    return result!
  }

  runEdit<A>(f: () => A): A {
    return this.run(() => {
      const result = this.runWithUndoCallbacks(f)
      this.render()
      return result })
  }

  undo() {
    const actions = this.undoStack.pop()
    expect(actions).not.toBe(undefined)
    this.run(() => {
      actions!.reverse().map(undoRedo => undoRedo.undo())
      actions!.reverse()
      this.redoStack.push(actions!)
      this.render() })
  }

  redo() {
    const actions = this.redoStack.pop()
    expect(actions).not.toBe(undefined)
    this.run(() => {
      actions!.map(undoRedo => undoRedo.redo())
      this.undoStack.push(actions!)
      this.render() })
  }

  textInput() {
    const activeElement = document.activeElement
    const activeInput = activeElement instanceof HTMLInputElement && activeElement.type === "text" || activeElement instanceof HTMLTextAreaElement
      ? activeElement as HTMLInputElement | HTMLTextAreaElement
      : null
    const input = activeInput && this.container.contains(activeInput)
      ? activeInput
      : this.container.querySelector("input[type=text], textarea") as HTMLInputElement | HTMLTextAreaElement | null
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

  globalKey(key: string, options: KeyboardEventInit = {}) {
    const event = new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options})
    withEnvironment(this.environment, () => act(() =>
      defaultKeyHandler(event, this.rootDescend, undefined, f => {
        const result = this.runWithUndoCallbacks(f)
        this.render()
        return result })))
  }

  click(element: Element, options: MouseEventInit = {}) {
    this.run(() => click(element, options))
  }

  first(selector: string) {
    const element = this.container.querySelector(selector)
    expect(element).not.toBe(null)
    return element!
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

function copyActive(harness: EditorHarness) {
  let copy: {referenceID: ID, copyResult: CopyResult} | undefined
  harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })
  expect(copy).not.toBe(undefined)
  return copy!
}

function pasteStructureIntoActive(harness: EditorHarness, copy: {referenceID: ID, copyResult: CopyResult}) {
  return harness.runEdit(() => {
    const id = copyIDFromClipboardText(clipboardStringForCopyResult(copy.referenceID, copy.copyResult))
    expect(id).not.toBe(undefined)
    expect(commitIDToActiveElement(id!)).toBe(true)
    return id! })
}

function testLibrary() {
  const evaluateFields = "guid-test-evaluate-fields"
  const evaluateFieldsTail = "guid-test-evaluate-fields-tail"
  const javascriptProgramFields = "guid-test-javascript-program-fields"
  const javascriptProgramFieldsTail = "guid-test-javascript-program-fields-tail"
  const functionDeclarationFields = "guid-test-function-declaration-fields"
  const root = "guid-test-library"
  return new Map([["Test", {
    root,
    idMap: new MapIDMap(new Map<string, Map<ID, ID>>([
      [root, new Map<ID, ID>([
        [nameField.id, sidFromString("Test")],
        [sidFromString("evaluate"), evaluateCtor.id],
        [sidFromString("javascriptProgram"), javascriptProgramCtor.id],
        [sidFromString("functionDeclaration"), functionDeclarationCtor.id] ])],
      [evaluateCtor.id, new Map<ID, ID>([
        [ctorField.id, ctorCtor.id],
        [nameField.id, sidFromString("Evaluate")],
        [fieldsField.id, evaluateFields] ])],
      [evaluateFields, new Map<ID, ID>([
        [ctorField.id, nonemptyListCtor.id],
        [headField.id, javascriptProgramField.id],
        [tailField.id, evaluateFieldsTail] ])],
      [evaluateFieldsTail, new Map<ID, ID>([[ctorField.id, emptyListCtor.id]])],
      [javascriptProgramField.id, new Map<ID, ID>([
        [ctorField.id, fieldCtor.id],
        [nameField.id, sidFromString("javascript program")] ])],
      [javascriptProgramCtor.id, new Map<ID, ID>([
        [ctorField.id, ctorCtor.id],
        [nameField.id, sidFromString("JavaScriptProgram")],
        [fieldsField.id, javascriptProgramFields] ])],
      [javascriptProgramFields, new Map<ID, ID>([
        [ctorField.id, nonemptyListCtor.id],
        [headField.id, statementsField.id],
        [tailField.id, javascriptProgramFieldsTail] ])],
      [javascriptProgramFieldsTail, new Map<ID, ID>([[ctorField.id, emptyListCtor.id]])],
      [statementsField.id, new Map<ID, ID>([
        [ctorField.id, fieldCtor.id],
        [nameField.id, sidFromString("statements")] ])],
      [functionDeclarationCtor.id, new Map<ID, ID>([
        [ctorField.id, ctorCtor.id],
        [nameField.id, sidFromString("FunctionDeclaration")],
        [fieldsField.id, functionDeclarationFields] ])],
      [functionDeclarationFields, new Map<ID, ID>([[ctorField.id, emptyListCtor.id]])] ])) }]])
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

  it("copies the focused string editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("copy me"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    let copy: {referenceID: ID, copyResult: CopyResult} | undefined
    harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })

    expect(copy?.referenceID).toBe(sidFromString("copy me"))
    expect(copy?.copyResult.root).toBe(sidFromString("copy me"))
    expect(copy?.copyResult.guidMap.map.size).toBe(0)

    harness.unmount()
  })

  it("copies the focused number editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, 7)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    let copy: {referenceID: ID, copyResult: CopyResult} | undefined
    harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })

    expect(copy?.referenceID).toBe(7)
    expect(copy?.copyResult.root).toBe(7)
    expect(copy?.copyResult.guidMap.map.size).toBe(0)

    harness.unmount()
  })

  it("pastes a structure copy through the focused placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-original"
    const child = "guid-child"
    const childLabel = sidFromString("child")
    const childNameLabel = sidFromString("name")
    const copyLabel = sidFromString("copy")
    environment.guidMap.set(environment.rootViews.id, rootField.id, original)
    environment.guidMap.set(original, childLabel, child)
    environment.guidMap.set(child, childNameLabel, sidFromString("Child"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)
    const copy = copyActive(harness)

    harness.run(() => {
      environment.selection = {cursor: _childCursor(rootCursor(environment), original, copyLabel)}
      harness.render() })
    const pasted = pasteStructureIntoActive(harness, copy)
    const pastedChild = harness.get(pasted, childLabel)

    expect(harness.get(original, copyLabel)).toBe(pasted)
    expect(pasted).not.toBe(original)
    expect(pastedChild).not.toBe(undefined)
    expect(pastedChild).not.toBe(child)
    expect(stringFromID(harness.get(pastedChild!, childNameLabel)!)).toBe("Child")

    harness.unmount()
  })

  it("preserves cycles when pasting a structure copy", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-cycle"
    const selfLabel = sidFromString("self")
    const copyLabel = sidFromString("copy")
    environment.guidMap.set(environment.rootViews.id, rootField.id, original)
    environment.guidMap.set(original, selfLabel, original)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)
    const copy = copyActive(harness)

    harness.run(() => {
      environment.selection = {cursor: _childCursor(rootCursor(environment), original, copyLabel)}
      harness.render() })
    const pasted = pasteStructureIntoActive(harness, copy)

    expect(pasted).not.toBe(original)
    expect(harness.get(pasted, selfLabel)).toBe(pasted)

    harness.unmount()
  })

  it("preserves shared children when pasting a structure copy", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-original"
    const shared = "guid-shared"
    const firstLabel = sidFromString("first")
    const secondLabel = sidFromString("second")
    const copyLabel = sidFromString("copy")
    environment.guidMap.set(environment.rootViews.id, rootField.id, original)
    environment.guidMap.set(original, firstLabel, shared)
    environment.guidMap.set(original, secondLabel, shared)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)
    const copy = copyActive(harness)

    harness.run(() => {
      environment.selection = {cursor: _childCursor(rootCursor(environment), original, copyLabel)}
      harness.render() })
    const pasted = pasteStructureIntoActive(harness, copy)

    expect(harness.get(pasted, firstLabel)).toBe(harness.get(pasted, secondLabel))
    expect(harness.get(pasted, firstLabel)).not.toBe(shared)

    harness.unmount()
  })

  it("remaps GUID edge labels when pasting a structure copy", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-original"
    const target = "guid-target"
    const label = "guid-label"
    const labelName = sidFromString("displayName")
    const copyLabel = sidFromString("copy")
    environment.guidMap.set(environment.rootViews.id, rootField.id, original)
    environment.guidMap.set(original, label, target)
    environment.guidMap.set(label, labelName, sidFromString("Label"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)
    const copy = copyActive(harness)

    harness.run(() => {
      environment.selection = {cursor: _childCursor(rootCursor(environment), original, copyLabel)}
      harness.render() })
    const pasted = pasteStructureIntoActive(harness, copy)
    const pastedEdges = environment.guidMap.edges(pasted as string)!
    const copiedEntry = Array.from(pastedEdges).find(([edgeLabel]) => edgeLabel !== nameField.id)
    expect(copiedEntry).not.toBe(undefined)
    const [copiedLabel, copiedTarget] = copiedEntry!

    expect(copiedLabel).not.toBe(label)
    expect(copiedTarget).not.toBe(target)
    expect(stringFromID(harness.get(copiedLabel, labelName)!)).toBe("Label")

    harness.unmount()
  })

  it("does not expose copy from a focused placeholder", () => {
    const harness = rootHarness()

    let hasCopy = true
    harness.run(() => { hasCopy = editorCommandsForActiveElement()?.copy !== undefined })

    expect(hasCopy).toBe(false)

    harness.unmount()
  })

  it("pastes a reference into the focused string editor", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("old"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.run(() => { commitIDToActiveElement("guid-pasted") })

    expect(harness.get(environment.rootViews.id, rootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("pastes a reference into the focused number editor", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, 1)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.run(() => { commitIDToActiveElement("guid-pasted") })

    expect(harness.get(environment.rootViews.id, rootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("undoes and redoes a placeholder commit", () => {
    const harness = rootHarness()

    harness.typeAndEnter("hello")
    expect(stringFromID(harness.get(harness.environment.rootViews.id, rootField.id)!)).toBe("hello")

    harness.undo()
    expect(harness.get(harness.environment.rootViews.id, rootField.id)).toBe(undefined)

    harness.redo()
    expect(stringFromID(harness.get(harness.environment.rootViews.id, rootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("undoes and redoes deleting a selected edge", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("delete me"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.globalKey("Delete")
    expect(harness.get(environment.rootViews.id, rootField.id)).toBe(undefined)

    harness.undo()
    expect(stringFromID(harness.get(environment.rootViews.id, rootField.id)!)).toBe("delete me")

    harness.redo()
    expect(harness.get(environment.rootViews.id, rootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("undoes and redoes a pasted structure copy as one edit", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-original"
    const child = "guid-child"
    const childLabel = sidFromString("child")
    const copyLabel = sidFromString("copy")
    environment.guidMap.set(environment.rootViews.id, rootField.id, original)
    environment.guidMap.set(original, childLabel, child)
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)
    const copy = copyActive(harness)

    harness.run(() => {
      environment.selection = {cursor: _childCursor(rootCursor(environment), original, copyLabel)}
      harness.render() })
    const pasted = pasteStructureIntoActive(harness, copy)
    expect(harness.get(original, copyLabel)).toBe(pasted)

    harness.undo()
    expect(harness.get(original, copyLabel)).toBe(undefined)
    expect(environment.guidMap.edges(pasted as string)).toBe(undefined)

    harness.redo()
    expect(harness.get(original, copyLabel)).toBe(pasted)
    expect(harness.get(pasted, childLabel)).not.toBe(undefined)

    harness.unmount()
  })

  it("commits a placeholder with Tab", () => {
    const harness = rootHarness()
    const textInput = harness.textInput()

    harness.run(() => {
      input(textInput, "hello")
      keyDown(textInput, "Tab") })

    expect(stringFromID(harness.get(harness.environment.rootViews.id, rootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("keeps selection when Tab has no placeholder to move to", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("only"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.globalKey("Tab")

    expect(harness.environment.selection?.cursor.parent).toBe(environment.rootViews.id)
    expect(harness.environment.selection?.cursor.label).toBe(rootField.id)
    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  it("opens completion with Enter and commits a clicked entry", () => {
    const harness = rootHarness()

    harness.key("Enter")
    harness.click(harness.first(".entrylist li"))

    const root = harness.get(harness.environment.rootViews.id, rootField.id)
    expect(root).not.toBe(undefined)
    expect(sidFromID(root!)).toBe(undefined)

    harness.unmount()
  })

  it("updates completion selection on mouse move", () => {
    const harness = rootHarness()

    harness.key("Enter")
    const entries = harness.container.querySelectorAll(".entrylist li")
    expect(entries.length).toBeGreaterThan(1)
    harness.run(() => entries[1].dispatchEvent(new MouseEvent("mousemove", {bubbles: true, cancelable: true})))

    expect(harness.container.querySelectorAll(".entrylist li")[1].classList.contains("selected")).toBe(true)

    harness.unmount()
  })

  it("navigates through rendered graph structure with arrow keys", () => {
    const environment = makeTestEnvironment({defaultRender})
    const root = "guid-root"
    const firstLabel = sidFromString("first")
    const secondLabel = sidFromString("second")
    environment.guidMap.set(environment.rootViews.id, rootField.id, root)
    environment.guidMap.set(root, firstLabel, sidFromString("First"))
    environment.guidMap.set(root, secondLabel, sidFromString("Second"))
    environment.selection = undefined
    const harness = new EditorHarness(environment)

    harness.globalKey("ArrowRight")
    expect(environment.selection?.cursor.parent).toBe(environment.rootViews.id)
    expect(environment.selection?.cursor.label).toBe(rootField.id)

    harness.globalKey("ArrowRight")
    expect(environment.selection?.cursor.parent).toBe(root)
    expect(environment.selection?.cursor.label).toBe(firstLabel)

    harness.globalKey("ArrowDown")
    expect(environment.selection?.cursor.parent).toBe(root)
    expect(environment.selection?.cursor.label).toBe(secondLabel)

    harness.globalKey("ArrowUp")
    expect(environment.selection?.cursor.parent).toBe(root)
    expect(environment.selection?.cursor.label).toBe(firstLabel)

    harness.globalKey("ArrowLeft")
    expect(environment.selection?.cursor.parent).toBe(environment.rootViews.id)
    expect(environment.selection?.cursor.label).toBe(rootField.id)

    harness.unmount()
  })

  it("clears selection with Escape", () => {
    const harness = rootHarness()

    harness.globalKey("Escape")

    expect(harness.environment.selection).toBe(undefined)

    harness.unmount()
  })

  it("inserts a list item with the global comma key and commits it", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.rootViews.id, rootField.id)
    harness.globalKey(",", {metaKey: true})
    const inserted = harness.environment.selection?.cursor.parent
    harness.typeAndEnter("second")

    expect(stringFromID(harness.get(list!, headField.id)!)).toBe("first")
    expect(stringFromID(harness.get(inserted!, headField.id)!)).toBe("second")
    expect(harness.get(list!, tailField.id)).toBe(inserted)

    harness.unmount()
  })

  it("deletes a selected edge with the global Delete key", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("delete me"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.globalKey("Delete")

    expect(harness.get(environment.rootViews.id, rootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("deletes a selected edge with the global Backspace key", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, sidFromString("delete me"))
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.globalKey("Backspace")

    expect(harness.get(environment.rootViews.id, rootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("selects a guid editor by mouse click", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, "guid-node")
    environment.selection = undefined
    const harness = new EditorHarness(environment)

    harness.click(harness.first(".guidEditor"))

    expect(harness.environment.selection?.cursor.parent).toBe(environment.rootViews.id)
    expect(harness.environment.selection?.cursor.label).toBe(rootField.id)

    harness.unmount()
  })

  it("chooses an existing rendered node by alt-clicking into a focused placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const parent = "guid-parent"
    const target = "guid-target"
    const existingLabel = sidFromString("existing")
    const missingLabel = sidFromString("missing")
    environment.guidMap.set(environment.rootViews.id, rootField.id, parent)
    environment.guidMap.set(parent, existingLabel, target)
    environment.selection = {cursor: _childCursor(rootCursor(environment), parent, missingLabel)}
    const harness = new EditorHarness(environment)

    const identicons = harness.container.querySelectorAll(".identicon")
    expect(identicons.length).toBeGreaterThan(1)
    harness.click(identicons[1], {altKey: true})

    expect(harness.get(parent, missingLabel)).toBe(target)

    harness.unmount()
  })

  it("renders and edits unknown fields alongside generated custom renders", () => {
    const appRender = renderIfApp(name => name)
    const environment = makeTestEnvironment({defaultRender: tryFirst(appRender, defaultRender)})
    const {app, extraField} = withEnvironment(environment, () => {
      const app = GUIDApp.new()
      const extraField = GUIDField.new().setName("Extra")
      return {app, extraField} })
    environment.guidMap.set(environment.rootViews.id, rootField.id, app.id)
    environment.guidMap.set(app.id, extraField.id, sidFromString("old"))
    environment.selection = {cursor: _childCursor(rootCursor(environment), app.id, extraField.id)}
    const harness = new EditorHarness(environment)

    expect(harness.container.textContent).toContain("Extra")
    harness.run(() => input(harness.textInput(), "new"))

    expect(stringFromID(harness.get(app.id, extraField.id)!)).toBe("new")

    harness.unmount()
  })

  it("does not edit read-only library string editors", () => {
    const libRoot = "guid-lib-root"
    const environment = makeTestEnvironment({
      libraries: new Map([[
        "library",
        {
          idMap: new MapIDMap(new Map([[libRoot, new Map([[nameField.id, sidFromString("old")]])]])),
          root: libRoot }]]),
      defaultRender})
    environment.guidMap.set(environment.rootViews.id, rootField.id, libRoot)
    environment.selection = {cursor: _childCursor(rootCursor(environment), libRoot, nameField.id)}
    const harness = new EditorHarness(environment)

    harness.run(() => input(harness.textInput(), "new"))

    expect(stringFromID(harness.get(libRoot, nameField.id)!)).toBe("old")

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

  it("creates an Evaluate, JavaScriptProgram, statement list, and statement through completions", () => {
    const environment = makeTestEnvironment({libraries: testLibrary(), defaultRender})
    environment.selection = {cursor: rootCursor(environment)}
    const harness = new EditorHarness(environment)

    harness.typeAndEnter("new Evaluate")
    const evaluate = harness.get(environment.rootViews.id, rootField.id)
    expect(evaluate).not.toBe(undefined)
    expect(harness.get(evaluate!, ctorField.id)).toBe(evaluateCtor.id)
    expect(environment.selection?.cursor.parent).toBe(evaluate)
    expect(environment.selection?.cursor.label).toBe(javascriptProgramField.id)

    harness.typeAndEnter("new JavaScriptProgram")
    const javascriptProgram = harness.get(evaluate!, javascriptProgramField.id)
    expect(javascriptProgram).not.toBe(undefined)
    expect(harness.get(javascriptProgram!, ctorField.id)).toBe(javascriptProgramCtor.id)
    expect(environment.selection?.cursor.parent).toBe(javascriptProgram)
    expect(environment.selection?.cursor.label).toBe(statementsField.id)

    harness.key("[")
    const statements = harness.get(javascriptProgram!, statementsField.id)
    expect(statements).not.toBe(undefined)
    expect(environment.selection?.cursor.parent).toBe(statements)
    expect(environment.selection?.cursor.label).toBe(headField.id)

    harness.typeAndEnter("new FunctionDeclaration")
    const statement = harness.get(statements!, headField.id)
    expect(statement).not.toBe(undefined)
    expect(harness.get(statement!, ctorField.id)).toBe(functionDeclarationCtor.id)

    harness.unmount()
  })
})
