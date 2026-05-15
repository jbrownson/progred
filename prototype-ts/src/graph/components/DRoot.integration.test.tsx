import * as React from "react"
import { act } from "react"
import { flushSync } from "react-dom"
import { createRoot, Root } from "react-dom/client"
import { afterEach, describe, expect, it, vi } from "vitest"
import { mapMaybe, Maybe, nothing } from "../../lib/Maybe"
import type { CopyResult } from "../editor/Copy"
import { undoRedoECallbacks } from "../editor/ECallbacks"
import { commitIDToActiveElement, editorCommandsForActiveElement } from "../editor/EditorCommands"
import { clipboardStringForCopyResult, copyIDFromClipboardText, idFromClipboardText } from "../editor/Clipboard"
import { _get, Environment, set, withEnvironment } from "../Environment"
import { appCtor, checkString, ctorCtor, ctorField, emptyListCtor, evaluateCtor, fieldCtor, fieldsField, functionDeclarationCtor, functionField, GUIDApp, GUIDDescend, GUIDEmptyList, GUIDField, GUIDLabel, GUIDLine, GUIDRenderCtor, headField, javascriptProgramCtor, javascriptProgramField, nameField, nonemptyListCtor, parametersField, returnCtor, statementsField, tailField } from "../graph"
import { ID, sidFromID, sidFromString, stringFromID } from "../model/ID"
import { DRoot, type D } from "../render/D"
import { createProjection } from "../render/project"
import { defaultRender, tryFirst } from "../render/defaultRender"
import { renderIfApp } from "../renderIfs"
import { renderFromRender } from "../render/renderFromRender"
import { dispatch, Render } from "../render/R"
import { makeTestEnvironment } from "../testHelpers"
import { defaultKeyHandler } from "../editor/keyHandler"
import { editorFocusForActiveElement, focusFirstEditor, focusPendingEditor } from "../editor/EditorFocus"
import { MapIDMap } from "../model/MapIDMap"
import type { UndoRedo } from "../editor/UndoRedo"
import { libraries } from "../libraries/libraries"
import { renders } from "../render/renders"
import { renderFromLibraries } from "../render/renderFromLibraries"
import { workspaceRootField } from "../workspace"

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
  rootDescend!: D
  undoStack: UndoRedo[][] = []
  redoStack: UndoRedo[][] = []
  initialFocusConsumed = false

  constructor(public environment: Environment, public initialFocus = false) {
    document.body.appendChild(this.container)
    this.root = createRoot(this.container)
    act(() => this.render())
  }

  render() {
    withEnvironment(this.environment, () => {
      const {rootDescend} = createProjection()
      this.rootDescend = rootDescend
      flushSync(() => this.root.render(<DRoot
          d={rootDescend}
          depth={0}
          runE={f => {
            this.runWithUndoCallbacks(f)
            this.render() }} />))
      if (!focusPendingEditor(this.container) && !this.initialFocusConsumed)
        if (this.initialFocus && focusFirstEditor(this.container))
          this.initialFocusConsumed = true
    })
  }

  runWithUndoCallbacks<A>(f: () => A): A {
    const {undoRedoArray, eCallbacks} = undoRedoECallbacks()
    const oldCallbacks = this.environment.callbacks
    this.environment.callbacks = eCallbacks
    try {
      const result = f()
      if (undoRedoArray.length > 0) {
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

  key(key: string, options: KeyboardEventInit = {}) {
    const textInput = this.textInput()
    this.run(() => keyDown(textInput, key, options))
  }

  activeKey(key: string, options: KeyboardEventInit = {}) {
    const activeElement = document.activeElement
    expect(activeElement).toBeInstanceOf(Element)
    expect(this.container.contains(activeElement)).toBe(true)
    this.run(() => keyDown(activeElement!, key, options))
  }

  activeKeyThroughEditor(key: string, options: KeyboardEventInit = {}) {
    const activeElement = document.activeElement
    expect(activeElement).toBeInstanceOf(Element)
    expect(this.container.contains(activeElement)).toBe(true)
    const event = new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options})
    withEnvironment(this.environment, () => act(() => {
      const keydown = (e: KeyboardEvent) => {
        expect(defaultKeyHandler(e as KeyboardEvent, f => {
          const result = this.runWithUndoCallbacks(f)
          this.render()
          return result })
        ).toBe(true)
      }
      window.addEventListener("keydown", keydown)
      try {
        activeElement!.dispatchEvent(event)
      } finally {
        window.removeEventListener("keydown", keydown) }}))
  }

  activeEdge() {
    const edge = editorFocusForActiveElement()?.edge
    expect(edge).not.toBe(undefined)
    return edge!
  }

  expectActive(parent: ID, label: ID) {
    const edge = this.activeEdge()
    expect(edge.parent).toBe(parent)
    expect(edge.label).toBe(label)
  }

  globalKey(key: string, options: KeyboardEventInit = {}) {
    const event = new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options})
    withEnvironment(this.environment, () => act(() =>
      defaultKeyHandler(event, f => {
        const result = this.runWithUndoCallbacks(f)
        this.render()
        return result })))
  }

  arrowLeft(count: number, parent: ID, label: ID) {
    for (let i = 0; i < count; i++)
      this.globalKey("ArrowLeft")
    this.expectActive(parent, label)
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

function rootHarness(render?: Render) {
  const environment = makeTestEnvironment({defaultRender: render ? tryFirst(render, defaultRender) : defaultRender})
  return new EditorHarness(environment, true)
}

function emptyListHarness() {
  const environment = makeTestEnvironment({defaultRender})
  let list: GUIDEmptyList
  withEnvironment(environment, () => {
    list = GUIDEmptyList.new()
    set(environment.workspace.id, workspaceRootField.id, list.id) })
  return {harness: new EditorHarness(environment), list: list!}
}

function appLikeEnvironment() {
  const environment = makeTestEnvironment({libraries, defaultRender})
  environment.defaultRender = withEnvironment(environment, () => tryFirst(dispatch(renders, renderFromLibraries(libraries)), defaultRender))
  return environment
}

function emptyAppListHarness() {
  const environment = appLikeEnvironment()
  let list: GUIDEmptyList
  withEnvironment(environment, () => {
    list = GUIDEmptyList.new()
    set(environment.workspace.id, workspaceRootField.id, list.id) })
  return {harness: new EditorHarness(environment), list: list!}
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

function pasteReferenceIntoActive(harness: EditorHarness, copy: {referenceID: ID, copyResult: CopyResult}) {
  return harness.runEdit(() => {
    const id = idFromClipboardText(clipboardStringForCopyResult(copy.referenceID, copy.copyResult))
    expect(id).not.toBe(undefined)
    expect(commitIDToActiveElement(id!)).toBe(true)
    return id! })
}

function startNewEdgeFromActive(harness: EditorHarness) {
  harness.run(() => {
    const newEdge = editorCommandsForActiveElement()?.newEdge
    expect(newEdge).not.toBe(undefined)
    newEdge!() })
}

function listItems(harness: EditorHarness, list: ID) {
  const items: ID[] = []
  let current = list
  for (let i = 0; i < 20; i++) {
    const ctor = harness.get(current, ctorField.id)
    if (ctor === emptyListCtor.id) return items
    expect(ctor).toBe(nonemptyListCtor.id)
    const item = harness.get(current, headField.id)
    expect(item).not.toBe(undefined)
    items.push(item!)
    current = harness.get(current, tailField.id)!
    expect(current).not.toBe(undefined)
  }
  throw new Error("List did not terminate")
}

function listStrings(harness: EditorHarness, list: ID) {
  return listItems(harness, list).map(id => stringFromID(id))
}

function listLength(harness: EditorHarness, list: ID) {
  let length = 0
  let current = list
  for (let i = 0; i < 20; i++) {
    const ctor = harness.get(current, ctorField.id)
    if (ctor === emptyListCtor.id) return length
    expect(ctor).toBe(nonemptyListCtor.id)
    length++
    current = harness.get(current, tailField.id)!
    expect(current).not.toBe(undefined)
  }
  throw new Error("List did not terminate")
}

function withJavascriptHost<A>(f: (javascriptCalls: string[]) => A): A {
  const oldProgred = window.progred
  const javascriptCalls: string[] = []
  const log = vi.spyOn(console, "log").mockImplementation(() => {})
  window.progred = {
    runJavascript: (javascript: string, sandboxObject: Record<string, unknown> = {}) => {
      javascriptCalls.push(javascript)
      return Function("sandbox", "javascript", "with (sandbox) { return eval(javascript) }")(sandboxObject, javascript) },
  } as typeof window.progred
  try {
    return f(javascriptCalls)
  } finally {
    if (oldProgred) window.progred = oldProgred
    else delete (window as unknown as {progred?: typeof window.progred}).progred
    log.mockRestore() }}

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

describe("DRoot editor integration", () => {
  it("commits a default-rendered root placeholder by typing and pressing Enter", () => {
    const harness = rootHarness()

    harness.typeAndEnter("random node")

    const root = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(root).not.toBe(undefined)
    expect(sidFromID(root!)).toBe(undefined)

    harness.unmount()
  })

  it("activates an unselected placeholder by clicking it", () => {
    const environment = makeTestEnvironment({defaultRender})
    const harness = new EditorHarness(environment)

    expect(harness.container.querySelector("input[type=text], textarea")).toBe(null)
    harness.click(harness.first(".uneditable"))
    harness.typeAndEnter("hello")

    expect(stringFromID(harness.get(environment.workspace.id, workspaceRootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("activates an unselected placeholder by focusing it with keyboard navigation", () => {
    const environment = makeTestEnvironment({defaultRender})
    const harness = new EditorHarness(environment)

    expect(harness.container.querySelector("input[type=text], textarea")).toBe(null)
    harness.globalKey("ArrowRight")

    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  it("dismisses a locally activated placeholder with Escape", () => {
    const environment = makeTestEnvironment({defaultRender})
    const harness = new EditorHarness(environment)

    harness.click(harness.first(".uneditable"))
    expect(harness.container.querySelector("input[type=text], textarea")).not.toBe(null)
    harness.key("Escape")

    expect(harness.container.querySelector("input[type=text], textarea")).toBe(null)

    harness.unmount()
  })

  it("commits a string through a default-rendered placeholder", () => {
    const harness = rootHarness()

    harness.typeAndEnter("hello")

    expect(stringFromID(harness.get(harness.environment.workspace.id, workspaceRootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("edits an existing string through the textarea path", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("old")
    const harness = new EditorHarness(environment, true)

    harness.run(() => input(harness.textInput(), "new"))

    expect(stringFromID(harness.get(environment.workspace.id, workspaceRootField.id)!)).toBe("new")

    harness.unmount()
  })

  it("edits an existing number through input and Enter", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = 1
    const harness = new EditorHarness(environment, true)

    const textInput = harness.textInput()
    harness.run(() => {
      input(textInput, "42")
      keyDown(textInput, "Enter") })

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(42)

    harness.unmount()
  })

  it("closes a placeholder completion when arrow navigation moves focus away", () => {
    const harness = new EditorHarness(appLikeEnvironment(), true)

    harness.typeAndEnter("new Module")
    const textInput = harness.textInput()
    harness.run(() => input(textInput, "a"))
    harness.run(() => harness.textInput().setSelectionRange(0, 0))
    expect(harness.textInput().selectionStart).toBe(0)
    expect(harness.container.querySelector(".entrylist")).not.toBe(null)

    harness.activeKeyThroughEditor("ArrowLeft")

    harness.expectActive(harness.environment.workspace.id, workspaceRootField.id)
    expect(harness.container.querySelector(".entrylist")).toBe(null)

    harness.unmount()
  })

  it("creates a list from a selected placeholder with the keyboard", () => {
    const harness = rootHarness()

    harness.key("[")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    expect(harness.get(list!, headField.id)).toBe(undefined)
    expect(harness.get(list!, tailField.id)).not.toBe(undefined)
    harness.expectActive(list!, headField.id)

    harness.unmount()
  })

  it("commits a list item through the placeholder created by list insertion", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("hello")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(stringFromID(harness.get(list!, headField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("focuses a list insertion point without editing the graph", () => {
    const {harness, list} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))

    expect(harness.get(harness.environment.workspace.id, workspaceRootField.id)).toBe(list.id)
    expect(harness.get(list.id, ctorField.id)).toBe(emptyListCtor.id)
    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  const activeListInsertionIsMultiline = (harness: EditorHarness) => {
    const placeholder = harness.textInput().parentElement
    return placeholder?.previousSibling instanceof HTMLSpanElement && placeholder.previousSibling.previousSibling instanceof HTMLBRElement
  }

  const renderedRootListIsMultiline = (harness: EditorHarness) => harness.container.querySelector("br") !== null

  it("keeps an active empty-list insertion point inline", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))

    expect(activeListInsertionIsMultiline(harness)).toBe(false)

    harness.unmount()
  })

  it("renders an active list insertion point multiline when it would add a second item", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("first")
    harness.click(harness.container.querySelectorAll(".listInsertionPoint")[1])

    expect(activeListInsertionIsMultiline(harness)).toBe(true)

    harness.unmount()
  })

  it("keeps a committed one-item list inline", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("first")

    expect(renderedRootListIsMultiline(harness)).toBe(false)

    harness.unmount()
  })

  it("renders a committed two-item list multiline", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("first")
    harness.click(harness.container.querySelectorAll(".listInsertionPoint")[1])
    harness.typeAndEnter("second")

    expect(renderedRootListIsMultiline(harness)).toBe(true)

    harness.unmount()
  })

  it("commits a list insertion point without a placeholder edit", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("hello")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    expect(listStrings(harness, list!)).toEqual(["hello"])
    harness.expectActive(list!, headField.id)
    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  it("opens an empty list insertion point with comma when the list is focused", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".guidEditor"))
    harness.activeKey(",")
    expect(document.activeElement).toBe(harness.textInput())
    harness.typeAndEnter("hello")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    expect(listStrings(harness, list!)).toEqual(["hello"])
    expect(harness.container.textContent).not.toBe("")

    harness.unmount()
  })

  it("commits a structured item through an empty list insertion point", () => {
    const {harness} = emptyAppListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("new Return")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    const head = harness.get(list!, headField.id)
    expect(head).not.toBe(undefined)
    expect(harness.get(head!, ctorField.id)).toBe(returnCtor.id)
    expect(harness.container.textContent).not.toBe("")

    harness.unmount()
  })

  it("commits a structured item through an empty list insertion point opened with comma", () => {
    const {harness} = emptyAppListHarness()

    harness.click(harness.first(".guidEditor"))
    harness.activeKey(",")
    harness.typeAndEnter("new Return")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    const head = harness.get(list!, headField.id)
    expect(head).not.toBe(undefined)
    expect(harness.get(head!, ctorField.id)).toBe(returnCtor.id)
    expect(harness.container.textContent).not.toBe("")

    harness.unmount()
  })

  it("does not blank when committing the return completion into an empty app list", () => {
    const {harness} = emptyAppListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("return")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    const head = harness.get(list!, headField.id)
    expect(head).not.toBe(undefined)
    expect(harness.container.textContent).not.toBe("")

    harness.unmount()
  })

  it("does not blank when committing Return into an Evaluate statements list", () => withJavascriptHost(() => {
    const environment = appLikeEnvironment()
    const harness = new EditorHarness(environment, true)

    harness.typeAndEnter("new Evaluate")
    harness.typeAndEnter("new JavaScriptProgram")
    harness.key("[")
    harness.typeAndEnter("new Return")

    const evaluate = harness.get(environment.workspace.id, workspaceRootField.id)
    const javascriptProgram = harness.get(evaluate!, javascriptProgramField.id)
    const statements = harness.get(javascriptProgram!, statementsField.id)
    const statement = harness.get(statements!, headField.id)
    expect(statement).not.toBe(undefined)
    expect(harness.get(statement!, ctorField.id)).toBe(returnCtor.id)
    expect(harness.container.textContent).not.toBe("")

    harness.unmount()
  }))

  it("commits a list insertion point by clicking a completion entry", () => {
    const {harness} = emptyListHarness()

    harness.click(harness.first(".listInsertionPoint"))
    harness.run(() => input(harness.textInput(), "hello"))
    harness.click(harness.first(".entrylist li"))

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(list).not.toBe(undefined)
    expect(listStrings(harness, list!)).toEqual(["hello"])
    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  it("renders separators around active list insertion points", () => {
    const {harness} = emptyListHarness()
    const commaCount = () => harness.container.textContent!.split(",").length - 1

    harness.click(harness.first(".listInsertionPoint"))
    harness.typeAndEnter("first")
    harness.click(harness.container.querySelectorAll(".listInsertionPoint")[1])
    harness.typeAndEnter("second")
    expect(commaCount()).toBe(1)

    harness.click(harness.container.querySelectorAll(".listInsertionPoint")[1])

    expect(commaCount()).toBe(2)
    expect(document.activeElement).toBe(harness.container.querySelector("input[placeholder=item]"))
    harness.typeAndEnter("zero")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(listStrings(harness, list!)).toEqual(["first", "zero", "second"])

    harness.unmount()
  })

  it("inserts a list item in the middle with keyboard navigation and comma", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.key(",", {metaKey: true})
    harness.typeAndEnter("third")
    harness.globalKey("ArrowUp")
    harness.key(",", {metaKey: true})
    harness.typeAndEnter("second")

    expect(listStrings(harness, list!)).toEqual(["first", "second", "third"])

    harness.unmount()
  })

  it("uses comma without meta for list insertion after a GUID item", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("random node")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.activeKey(",")

    expect(listLength(harness, list!)).toBe(1)
    expect(document.activeElement).toBe(harness.textInput())

    harness.typeAndEnter("second")

    expect(listLength(harness, list!)).toBe(2)

    harness.unmount()
  })

  it("does not use comma without meta for list insertion after a string item", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(document.activeElement).toBe(harness.container.querySelector("textarea"))
    const stringEditor = document.activeElement
    harness.activeKey(",")

    expect(listLength(harness, list!)).toBe(1)
    expect(document.activeElement).toBe(stringEditor)

    harness.unmount()
  })

  it("deletes a middle list item with the global Delete key", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.activeKey(",", {metaKey: true})
    harness.typeAndEnter("third")
    harness.globalKey("ArrowUp")
    harness.activeKey(",", {metaKey: true})
    harness.typeAndEnter("second")

    expect(listStrings(harness, list!)).toEqual(["first", "second", "third"])
    harness.globalKey("Delete")

    expect(listStrings(harness, list!)).toEqual(["first", "third"])

    harness.unmount()
  })

  it("focuses the list after deleting its only item", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("only")
    harness.globalKey("Delete")

    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    expect(harness.get(list!, ctorField.id)).toBe(emptyListCtor.id)
    harness.expectActive(harness.environment.workspace.id, workspaceRootField.id)

    harness.unmount()
  })

  it("does not edit a list until an insertion point is committed", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.activeKey(",", {metaKey: true})
    expect(listLength(harness, list!)).toBe(1)
    expect(document.activeElement).toBe(harness.textInput())

    harness.typeAndEnter("second")
    const inserted = harness.get(list!, tailField.id)
    expect(listLength(harness, list!)).toBe(2)

    harness.undo()
    expect(listLength(harness, list!)).toBe(1)

    harness.redo()
    expect(listLength(harness, list!)).toBe(2)
    expect(harness.get(list!, tailField.id)).toBe(inserted)

    harness.unmount()
  })

  it("closes a list insertion placeholder when arrow navigation moves focus away", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    harness.activeKey(",", {metaKey: true})
    expect(harness.container.querySelector("input[placeholder=item]")).not.toBe(null)

    harness.globalKey("ArrowRight")

    expect(harness.container.querySelector("input[placeholder=item]")).toBe(null)

    harness.unmount()
  })

  it("uses pending edge-label selection and then commits the target placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const node = "guid-node"
    environment.workspace.root = node
    const harness = new EditorHarness(environment, true)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("random node")
    expect(document.activeElement).toBe(harness.textInput())
    harness.typeAndEnter("hello")

    const label = Array.from(environment.guidMap.edges(node)!.keys())[0]
    expect(label).not.toBe(undefined)

    expect(stringFromID(harness.get(node, label!)!)).toBe("hello")

    harness.unmount()
  })

  it("uses an existing named edge label and then commits its target placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const node = "guid-node"
    const field = "guid-field"
    environment.workspace.root = node
    environment.guidMap.set(node, sidFromString("available label"), field)
    environment.guidMap.set(field, nameField.id, sidFromString("Existing Label"))
    const harness = new EditorHarness(environment, true)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("Existing Label")

    expect(harness.get(node, field)).toBe(undefined)
    expect(document.activeElement).toBe(harness.textInput())

    harness.typeAndEnter("target")

    expect(stringFromID(harness.get(node, field)!)).toBe("target")

    harness.unmount()
  })

  it("pastes a reference into the focused placeholder through editor commands", () => {
    const harness = rootHarness()
    const pastedID = idFromClipboardText(JSON.stringify({id: "guid-pasted"}))

    harness.run(() => {
      expect(pastedID).not.toBe(undefined)
      commitIDToActiveElement(pastedID!) })

    expect(harness.get(harness.environment.workspace.id, workspaceRootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("copies the focused guid editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = "guid-node"
    const harness = new EditorHarness(environment, true)

    let copy: {referenceID: ID, copyResult: CopyResult} | undefined
    harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })

    expect(copy?.referenceID).toBe("guid-node")
    expect(copy?.copyResult.root).not.toBe("guid-node")

    harness.unmount()
  })

  it("copies the focused string editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("copy me")
    const harness = new EditorHarness(environment, true)

    let copy: {referenceID: ID, copyResult: CopyResult} | undefined
    harness.run(() => { copy = editorCommandsForActiveElement()?.copy?.() })

    expect(copy?.referenceID).toBe(sidFromString("copy me"))
    expect(copy?.copyResult.root).toBe(sidFromString("copy me"))
    expect(copy?.copyResult.guidMap.map.size).toBe(0)

    harness.unmount()
  })

  it("keeps editor commands attached across D rerenders", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("copy me")
    const harness = new EditorHarness(environment, true)
    const activeElement = document.activeElement

    harness.render()

    expect(document.activeElement).toBe(activeElement)
    expect(editorCommandsForActiveElement()?.copy?.().referenceID).toBe(sidFromString("copy me"))

    harness.unmount()
  })

  it("copies the focused number editor through editor commands", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = 7
    const harness = new EditorHarness(environment, true)

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
    environment.workspace.root = original
    environment.guidMap.set(original, childLabel, child)
    environment.guidMap.set(child, childNameLabel, sidFromString("Child"))
    const harness = new EditorHarness(environment, true)
    const copy = copyActive(harness)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("copy")
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
    environment.workspace.root = original
    environment.guidMap.set(original, selfLabel, original)
    const harness = new EditorHarness(environment, true)
    const copy = copyActive(harness)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("copy")
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
    environment.workspace.root = original
    environment.guidMap.set(original, firstLabel, shared)
    environment.guidMap.set(original, secondLabel, shared)
    const harness = new EditorHarness(environment, true)
    const copy = copyActive(harness)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("copy")
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
    environment.workspace.root = original
    environment.guidMap.set(original, label, target)
    environment.guidMap.set(label, labelName, sidFromString("Label"))
    const harness = new EditorHarness(environment, true)
    const copy = copyActive(harness)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("copy")
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

  it("copies a function declaration and pastes a reference into a function call", () => {
    const environment = makeTestEnvironment({libraries, defaultRender})
    const harness = new EditorHarness(environment, true)

    harness.typeAndEnter("new JavaScriptProgram")
    const javascriptProgram = harness.get(environment.workspace.id, workspaceRootField.id)
    harness.key("[")
    const statements = harness.get(javascriptProgram!, statementsField.id)
    harness.typeAndEnter("new Function Declaration")
    const factorial = harness.get(statements!, headField.id)
    harness.typeAndEnter("factorial")
    harness.arrowLeft(1, statements!, headField.id)
    const copy = copyActive(harness)

    harness.activeKey(",", {metaKey: true})
    harness.typeAndEnter("new Function Call")
    const callList = harness.get(statements!, tailField.id)
    const call = harness.get(callList!, headField.id)
    pasteReferenceIntoActive(harness, copy)

    expect(harness.get(call!, functionField.id)).toBe(factorial)

    harness.unmount()
  })

  it("pastes a function declaration structure into a statement list with remapped internals", () => {
    const environment = makeTestEnvironment({libraries, defaultRender})
    const harness = new EditorHarness(environment, true)

    harness.typeAndEnter("new JavaScriptProgram")
    const javascriptProgram = harness.get(environment.workspace.id, workspaceRootField.id)
    harness.key("[")
    const statements = harness.get(javascriptProgram!, statementsField.id)
    harness.typeAndEnter("new Function Declaration")
    const original = harness.get(statements!, headField.id)
    harness.typeAndEnter("factorial")
    harness.key("[")
    const originalParameters = harness.get(original!, parametersField.id)
    harness.typeAndEnter("new Parameter")
    const originalParameter = harness.get(originalParameters!, headField.id)
    harness.typeAndEnter("n")
    harness.arrowLeft(1, statements!, headField.id)
    const copy = copyActive(harness)

    harness.activeKey(",", {metaKey: true})
    const pasted = pasteStructureIntoActive(harness, copy)
    const copyList = harness.get(statements!, tailField.id)
    const pastedParameters = harness.get(pasted, parametersField.id)
    const pastedParameter = harness.get(pastedParameters!, headField.id)

    expect(harness.get(copyList!, headField.id)).toBe(pasted)
    expect(pasted).not.toBe(original)
    expect(stringFromID(harness.get(pasted, nameField.id)!)).toBe("factorial")
    expect(pastedParameters).not.toBe(originalParameters)
    expect(pastedParameter).not.toBe(originalParameter)
    expect(stringFromID(harness.get(pastedParameter!, nameField.id)!)).toBe("n")

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
    environment.workspace.root = sidFromString("old")
    const harness = new EditorHarness(environment, true)

    harness.run(() => { commitIDToActiveElement("guid-pasted") })

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("pastes a reference into the focused number editor", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = 1
    const harness = new EditorHarness(environment, true)

    harness.run(() => { commitIDToActiveElement("guid-pasted") })

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe("guid-pasted")

    harness.unmount()
  })

  it("undoes and redoes a placeholder commit", () => {
    const harness = rootHarness()

    harness.typeAndEnter("hello")
    expect(stringFromID(harness.get(harness.environment.workspace.id, workspaceRootField.id)!)).toBe("hello")

    harness.undo()
    expect(harness.get(harness.environment.workspace.id, workspaceRootField.id)).toBe(undefined)

    harness.redo()
    expect(stringFromID(harness.get(harness.environment.workspace.id, workspaceRootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("undoes and redoes deleting a selected edge", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("delete me")
    const harness = new EditorHarness(environment, true)

    harness.globalKey("Delete")
    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(undefined)

    harness.undo()
    expect(stringFromID(harness.get(environment.workspace.id, workspaceRootField.id)!)).toBe("delete me")

    harness.redo()
    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("deletes the focused editor instead of a stale logical selection", () => {
    const environment = makeTestEnvironment({defaultRender})
    const node = "guid-node"
    const childLabel = sidFromString("child")
    environment.workspace.root = node
    environment.guidMap.set(node, childLabel, sidFromString("keep me"))
    const harness = new EditorHarness(environment, true)

    harness.globalKey("Delete")

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(undefined)
    expect(stringFromID(harness.get(node, childLabel)!)).toBe("keep me")

    harness.unmount()
  })

  it("undoes and redoes a pasted structure copy as one edit", () => {
    const environment = makeTestEnvironment({defaultRender})
    const original = "guid-original"
    const child = "guid-child"
    const childLabel = sidFromString("child")
    const copyLabel = sidFromString("copy")
    environment.workspace.root = original
    environment.guidMap.set(original, childLabel, child)
    const harness = new EditorHarness(environment, true)
    const copy = copyActive(harness)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("copy")
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

    expect(stringFromID(harness.get(harness.environment.workspace.id, workspaceRootField.id)!)).toBe("hello")

    harness.unmount()
  })

  it("keeps selection when Tab has no placeholder to move to", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("only")
    const harness = new EditorHarness(environment, true)

    harness.globalKey("Tab")

    harness.expectActive(environment.workspace.id, workspaceRootField.id)
    expect(document.activeElement).toBe(harness.textInput())

    harness.unmount()
  })

  it("opens completion with Enter and commits a clicked entry", () => {
    const harness = rootHarness()

    harness.key("Enter")
    harness.click(harness.first(".entrylist li"))

    const root = harness.get(harness.environment.workspace.id, workspaceRootField.id)
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
    environment.workspace.root = root
    environment.guidMap.set(root, firstLabel, sidFromString("First"))
    environment.guidMap.set(root, secondLabel, sidFromString("Second"))
    const harness = new EditorHarness(environment)

    harness.globalKey("ArrowRight")
    harness.expectActive(environment.workspace.id, workspaceRootField.id)

    harness.globalKey("ArrowRight")
    harness.expectActive(root, firstLabel)

    harness.globalKey("ArrowDown")
    harness.expectActive(root, secondLabel)

    harness.globalKey("ArrowUp")
    harness.expectActive(root, firstLabel)

    harness.globalKey("ArrowLeft")
    harness.expectActive(environment.workspace.id, workspaceRootField.id)

    harness.unmount()
  })

  it("navigates from the focused editor instead of stale logical selection", () => {
    const environment = makeTestEnvironment({defaultRender})
    const root = "guid-root"
    const firstLabel = sidFromString("first")
    const secondLabel = sidFromString("second")
    environment.workspace.root = root
    environment.guidMap.set(root, firstLabel, sidFromString("First"))
    environment.guidMap.set(root, secondLabel, sidFromString("Second"))
    const harness = new EditorHarness(environment, true)

    harness.globalKey("ArrowRight")
    const focusedInput = harness.textInput()
    expect(document.activeElement).toBe(focusedInput)
    harness.run(() => { focusedInput.focus() })
    harness.globalKey("ArrowDown")

    harness.expectActive(root, secondLabel)

    harness.unmount()
  })

  it("defaults cycles collapsed and toggles them without changing selection", () => {
    const environment = makeTestEnvironment({defaultRender})
    const root = "guid-cycle-root"
    environment.workspace.root = root
    environment.guidMap.set(root, nameField.id, sidFromString("Cycle"))
    environment.guidMap.set(root, sidFromString("self"), root)
    const harness = new EditorHarness(environment, true)
    const expandedBefore = Array.from(harness.container.querySelectorAll(".collapseToggle")).filter(toggle => toggle.textContent === "▾").length
    const collapsed = Array.from(harness.container.querySelectorAll(".collapseToggle")).filter(toggle => toggle.textContent === "▸")
    expect(collapsed.length).toBeGreaterThan(0)

    harness.click(collapsed[0])

    harness.expectActive(environment.workspace.id, workspaceRootField.id)
    expect(Array.from(harness.container.querySelectorAll(".collapseToggle")).filter(toggle => toggle.textContent === "▾").length).toBeGreaterThan(expandedBefore)

    harness.unmount()
  })

  it("clears selection with Escape", () => {
    const harness = rootHarness()

    harness.globalKey("Escape")

    expect(document.activeElement).not.toBe(undefined)

    harness.unmount()
  })

  it("inserts a list item with comma and commits it", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.activeKey(",", {metaKey: true})
    expect(listLength(harness, list!)).toBe(1)
    harness.typeAndEnter("second")
    const inserted = harness.get(list!, tailField.id)

    expect(stringFromID(harness.get(list!, headField.id)!)).toBe("first")
    expect(stringFromID(harness.get(inserted!, headField.id)!)).toBe("second")
    expect(harness.get(list!, tailField.id)).toBe(inserted)

    harness.unmount()
  })

  it("inserts after the focused list item instead of stale logical selection", () => {
    const harness = rootHarness()

    harness.key("[")
    harness.typeAndEnter("first")
    const list = harness.get(harness.environment.workspace.id, workspaceRootField.id)
    harness.activeKey(",", {metaKey: true})
    expect(listLength(harness, list!)).toBe(1)
    harness.typeAndEnter("second")
    const inserted = harness.get(list!, tailField.id)

    expect(listStrings(harness, list!)).toEqual(["first", "second"])
    expect(harness.get(list!, tailField.id)).toBe(inserted)

    harness.unmount()
  })

  it("deletes a selected edge with the global Delete key", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("delete me")
    const harness = new EditorHarness(environment, true)

    harness.globalKey("Delete")

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("deletes a selected edge with the global Backspace key", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = sidFromString("delete me")
    const harness = new EditorHarness(environment, true)

    harness.globalKey("Backspace")

    expect(harness.get(environment.workspace.id, workspaceRootField.id)).toBe(undefined)

    harness.unmount()
  })

  it("selects a guid editor by mouse click", () => {
    const environment = makeTestEnvironment({defaultRender})
    environment.workspace.root = "guid-node"
    const harness = new EditorHarness(environment)

    harness.click(harness.first(".guidEditor"))

    harness.expectActive(environment.workspace.id, workspaceRootField.id)

    harness.unmount()
  })

  it("focuses a custom-rendered guid editor when clicked", () => {
    const appRender = renderIfApp(name => name)
    const environment = makeTestEnvironment({defaultRender: tryFirst(appRender, defaultRender)})
    const app = withEnvironment(environment, () => GUIDApp.new().setName("Widget"))
    environment.workspace.root = app.id
    const harness = new EditorHarness(environment)
    const guidEditor = harness.first(".guidEditor")

    harness.click(guidEditor)

    expect(document.activeElement).toBe(guidEditor)
    harness.expectActive(environment.workspace.id, workspaceRootField.id)

    harness.unmount()
  })

  it("chooses an existing rendered node by alt-clicking into a focused placeholder", () => {
    const environment = makeTestEnvironment({defaultRender})
    const parent = "guid-parent"
    const target = "guid-target"
    const existingLabel = sidFromString("existing")
    const missingLabel = sidFromString("missing")
    environment.workspace.root = parent
    environment.guidMap.set(parent, existingLabel, target)
    const harness = new EditorHarness(environment, true)

    startNewEdgeFromActive(harness)
    harness.typeAndEnter("missing")
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
    environment.workspace.root = app.id
    environment.guidMap.set(app.id, extraField.id, sidFromString("old"))
    const harness = new EditorHarness(environment, true)

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
    environment.workspace.root = libRoot
    const harness = new EditorHarness(environment, true)

    harness.run(() => input(harness.textInput(), "new"))

    expect(stringFromID(harness.get(libRoot, nameField.id)!)).toBe("old")

    harness.unmount()
  })

  it("edits a field exposed by a generated custom render", () => {
    const appRender = renderIfApp(name => name)
    const environment = makeTestEnvironment({defaultRender: tryFirst(appRender, defaultRender)})
    const app = withEnvironment(environment, () => GUIDApp.new())
    environment.workspace.root = app.id
    const harness = new EditorHarness(environment, true)

    harness.globalKey("ArrowRight")
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
    environment.workspace.root = app.id
    const harness = new EditorHarness(environment, true)

    harness.globalKey("ArrowRight")
    harness.typeAndEnter("Templated")

    expect(stringFromID(harness.get(app.id, nameField.id)!)).toBe("Templated")

    harness.unmount()
  })

  it("renders labels in in-document render templates against the rendered node", () => {
    const environment = makeTestEnvironment()
    const {app, render} = withEnvironment(environment, () => {
      environment.guidMap.set(appCtor.id, ctorField.id, ctorCtor.id)
      environment.guidMap.set(nameField.id, ctorField.id, fieldCtor.id)
      const template = GUIDRenderCtor.new()
      const line = GUIDLine.new()
      const label = GUIDLabel.new()
      label.setField(nameField)
      label.setChild(checkString(sidFromString("Name"))!)
      line.setChildren([label])
      template.setForCtor(appCtor)
      template.setD(line)
      const render = renderFromRender(template)
      expect(render).not.toBe(undefined)
      return {app: GUIDApp.new(), render: render!} })
    environment.defaultRender = tryFirst(render, defaultRender)
    environment.workspace.root = app.id
    const harness = new EditorHarness(environment, true)

    expect(harness.container.textContent).toContain("Name")

    harness.unmount()
  })

  it("creates an Evaluate, JavaScriptProgram, statement list, and statement through completions", () => {
    const environment = makeTestEnvironment({libraries: testLibrary(), defaultRender})
    const harness = new EditorHarness(environment, true)

    harness.typeAndEnter("new Evaluate")
    const evaluate = harness.get(environment.workspace.id, workspaceRootField.id)
    expect(evaluate).not.toBe(undefined)
    expect(harness.get(evaluate!, ctorField.id)).toBe(evaluateCtor.id)
    harness.expectActive(evaluate!, javascriptProgramField.id)

    harness.typeAndEnter("new JavaScriptProgram")
    const javascriptProgram = harness.get(evaluate!, javascriptProgramField.id)
    expect(javascriptProgram).not.toBe(undefined)
    expect(harness.get(javascriptProgram!, ctorField.id)).toBe(javascriptProgramCtor.id)
    harness.expectActive(javascriptProgram!, statementsField.id)

    harness.key("[")
    const statements = harness.get(javascriptProgram!, statementsField.id)
    expect(statements).not.toBe(undefined)
    harness.expectActive(statements!, headField.id)

    harness.typeAndEnter("new FunctionDeclaration")
    const statement = harness.get(statements!, headField.id)
    expect(statement).not.toBe(undefined)
    expect(harness.get(statement!, ctorField.id)).toBe(functionDeclarationCtor.id)

    harness.unmount()
  })

  it("enters and evaluates a factorial program through editor interactions", () => withJavascriptHost(javascriptCalls => {
    const environment = makeTestEnvironment({libraries, defaultRender: tryFirst(renders, defaultRender)})
    const harness = new EditorHarness(environment, true)

    harness.typeAndEnter("new Evaluate")
    const evaluate = harness.get(environment.workspace.id, workspaceRootField.id)
    expect(evaluate).not.toBe(undefined)
    expect(harness.get(evaluate!, ctorField.id)).toBe(evaluateCtor.id)

    harness.typeAndEnter("new JavaScriptProgram")
    const javascriptProgram = harness.get(evaluate!, javascriptProgramField.id)
    expect(javascriptProgram).not.toBe(undefined)
    expect(harness.get(javascriptProgram!, ctorField.id)).toBe(javascriptProgramCtor.id)

    harness.key("[")
    const topStatements = harness.get(javascriptProgram!, statementsField.id)
    expect(topStatements).not.toBe(undefined)

    harness.typeAndEnter("new Function Declaration")
    const factorial = harness.get(topStatements!, headField.id)
    expect(factorial).not.toBe(undefined)
    expect(harness.get(factorial!, ctorField.id)).toBe(functionDeclarationCtor.id)

    harness.typeAndEnter("factorial")
    expect(stringFromID(harness.get(factorial!, nameField.id)!)).toBe("factorial")

    harness.key("[")
    const parameters = harness.get(factorial!, parametersField.id)
    expect(parameters).not.toBe(undefined)

    harness.typeAndEnter("new Parameter")
    const n = harness.get(parameters!, headField.id)
    expect(n).not.toBe(undefined)

    harness.typeAndEnter("n")
    expect(stringFromID(harness.get(n!, nameField.id)!)).toBe("n")

    harness.key("[")
    const bodyStatements = harness.get(factorial!, statementsField.id)
    expect(bodyStatements).not.toBe(undefined)

    harness.typeAndEnter("new Return")
    const returnStatement = harness.get(bodyStatements!, headField.id)
    expect(returnStatement).not.toBe(undefined)

    harness.typeAndEnter("new Conditional")
    harness.typeAndEnter("new Binary Inline")
    harness.typeAndEnter("n")
    harness.typeAndEnter("new Less Than or Equal To")
    harness.typeAndEnter("1")
    harness.typeAndEnter("1")
    harness.typeAndEnter("new Binary Inline")
    harness.typeAndEnter("n")
    harness.typeAndEnter("new Product")
    harness.typeAndEnter("new Function Call")
    harness.typeAndEnter("factorial")
    harness.key("[")
    const recursiveArguments = harness.activeEdge().parent
    expect(recursiveArguments).not.toBe(undefined)

    harness.typeAndEnter("new Binary Inline")
    harness.typeAndEnter("n")
    harness.typeAndEnter("new Difference")
    harness.typeAndEnter("1")

    harness.arrowLeft(8, topStatements!, headField.id)
    harness.activeKey(",", {metaKey: true})
    harness.typeAndEnter("new Function Call")
    const topCallList = harness.get(topStatements!, tailField.id)
    expect(topCallList).not.toBe(undefined)
    const topCall = harness.get(topCallList!, headField.id)
    expect(topCall).not.toBe(undefined)

    harness.typeAndEnter("factorial")
    harness.key("[")
    harness.typeAndEnter("5")

    const javascript = javascriptCalls[javascriptCalls.length - 1]
    expect(javascript).toMatch(/function _[A-Za-z0-9_$]+\(_[A-Za-z0-9_$]+\)/)
    expect(javascript).toMatch(/_[A-Za-z0-9_$]+\(5\)/)
    expect(javascript).not.toContain("function factorial")
    expect(javascript).not.toContain("factorial(5)")
    expect(harness.container.textContent).toContain("120")

    harness.unmount()
  }), 10000)
})
