import { act } from "react"
import { afterEach, describe, expect, it, vi } from "vitest"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

function setNativeValue(element: HTMLInputElement | HTMLTextAreaElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")?.set
  setter?.call(element, value)
}

function input(element: HTMLInputElement | HTMLTextAreaElement, value: string) {
  setNativeValue(element, value)
  element.dispatchEvent(new Event("input", {bubbles: true}))
}

function keyDown(element: Element, key: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options})
  element.dispatchEvent(event)
  if (!event.cancelBubble)
    window.onkeydown?.(event)
}

function click(element: Element, options: MouseEventInit = {}) {
  element.dispatchEvent(new MouseEvent("mousedown", {bubbles: true, cancelable: true, ...options}))
  element.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true, ...options}))
}

function graphClick(element: Element, options: MouseEventInit = {}) {
  element.dispatchEvent(new MouseEvent("mousedown", {bubbles: true, cancelable: true, ...options}))
  window.dispatchEvent(new MouseEvent("mouseup", {bubbles: true, cancelable: true, ...options}))
  element.dispatchEvent(new MouseEvent("click", {bubbles: true, cancelable: true, ...options}))
}

function textInput(root: HTMLElement) {
  const input = root.querySelector("input[type=text], textarea") as HTMLInputElement | HTMLTextAreaElement | null
  expect(input).not.toBe(null)
  return input!
}

async function actEvent(f: () => void) {
  await act(async () => {
    f()
    await Promise.resolve()
  })
}

async function typeAndEnter(root: HTMLElement, value: string) {
  await actEvent(() => {
    const text = textInput(root)
    input(text, value)
    keyDown(text, "Enter")
  })
}

function installProgred() {
  let menuAction: ((action: string) => void) | undefined
  let menuEnabled = new Map<string, boolean>()
  window.progred = {
    openFile: async () => undefined,
    saveFileAs: async () => undefined,
    writeFile: async () => {},
    writeClipboardText: () => {},
    readClipboardText: () => undefined,
    availableClipboardFormats: () => [],
    readPlainText: () => "",
    runJavascript: (javascript: string, sandboxObject: Record<string, unknown> = {}) =>
      Function("sandbox", "javascript", "with (sandbox) { return eval(javascript) }")(sandboxObject, javascript),
    sendActionToFirstResponder: () => {},
    setMenuItemEnabled: (id, enabled) => { menuEnabled.set(id, enabled) },
    setMenuItemChecked: () => {},
    onMenuAction: callback => {
      menuAction = callback
      return () => { if (menuAction === callback) menuAction = undefined }},
  }
  return {
    menuEnabled,
    menuAction: (action: string) => {
      expect(menuAction).not.toBe(undefined)
      menuAction!(action) }}
}

async function launchEditor() {
  document.body.innerHTML = `<div id="root"></div>`
  const progred = installProgred()
  await act(async () => {
    await import("./graphEditor")
    await Promise.resolve()
  })
  return {root: document.getElementById("root")!, progred}
}

describe("graphEditor integration", () => {
  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.resetModules()
  })

  it("does not blank the app when inserting into a root empty list", async () => {
    const {root} = await launchEditor()

    await typeAndEnter(root, "new Empty List")
    expect(root.textContent).not.toBe("")
    expect(document.activeElement).toBe(root.querySelector("input[placeholder=item]"))

    await typeAndEnter(root, "hello")

    expect(root.textContent).not.toBe("")
  })

  it("focuses the root placeholder after creating a new document", async () => {
    const {root, progred} = await launchEditor()

    await typeAndEnter(root, "random node")
    expect(root.querySelector(".guidEditor")).not.toBe(null)

    await actEvent(() => progred.menuAction("new"))

    expect(document.activeElement).toBe(root.querySelector("input[placeholder=root]"))
    expect(progred.menuEnabled.get("new-node")).toBe(true)
  })

  it("only creates a new node when an editor is focused", async () => {
    const {root, progred} = await launchEditor()

    expect(progred.menuEnabled.get("new-node")).toBe(true)
    await actEvent(() => progred.menuAction("new-node"))
    expect(root.querySelector(".guidEditor")).not.toBe(null)

    await actEvent(() => {
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur() })
    expect(progred.menuEnabled.get("new-node")).toBe(false)
    const previousText = root.textContent
    await actEvent(() => progred.menuAction("new-node"))

    expect(root.textContent).toBe(previousText)
  })

  it("only starts a new edge when an editor supports it", async () => {
    const {root, progred} = await launchEditor()

    expect(progred.menuEnabled.get("new-edge")).toBe(false)
    await actEvent(() => progred.menuAction("new-edge"))
    expect(root.querySelector("input[placeholder=label]")).toBe(null)

    await typeAndEnter(root, "random node")
    await actEvent(() => click(root.querySelector(".guidEditor")!))
    expect(progred.menuEnabled.get("new-edge")).toBe(true)
    await actEvent(() => progred.menuAction("new-edge"))
    expect(root.querySelector("input[placeholder=label]")).not.toBe(null)

    await actEvent(() => {
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur() })
    expect(progred.menuEnabled.get("new-edge")).toBe(false)
  })

  it("opens the active node in the view panel", async () => {
    const {root, progred} = await launchEditor()

    await typeAndEnter(root, "new Module")
    await actEvent(() => click(root.querySelector(".guidEditor")!))
    await actEvent(() => progred.menuAction("new-view"))

    const viewPanel = root.querySelector(".viewPanel")
    expect(viewPanel).not.toBe(null)
    expect(viewPanel!.textContent).toContain("Module")
  })

  it("opens the active node constructor in the view panel", async () => {
    const {root, progred} = await launchEditor()

    await typeAndEnter(root, "new Module")
    await actEvent(() => click(root.querySelector(".guidEditor")!))
    await actEvent(() => progred.menuAction("view-constructor"))

    const viewPanel = root.querySelector(".viewPanel")
    expect(viewPanel).not.toBe(null)
    expect(viewPanel!.textContent).toContain("Ctor")
    expect(viewPanel!.textContent).toContain("Module")
  })

  it("starts a new edge with the graph view open", async () => {
    const {root, progred} = await launchEditor()

    await typeAndEnter(root, "random node")
    await actEvent(() => click(root.querySelector(".guidEditor")!))
    await actEvent(() => progred.menuAction("toggle-graph"))
    await actEvent(() => progred.menuAction("new-edge"))

    expect(root.querySelector("input[placeholder=label]")).not.toBe(null)
  })

  it("deletes the root node from the graph view", async () => {
    const {root, progred} = await launchEditor()

    await typeAndEnter(root, "random node")
    await actEvent(() => progred.menuAction("toggle-graph"))
    await actEvent(() => graphClick(root.querySelector(".graphNode.rootGraphNode")!))
    expect(progred.menuEnabled.get("delete")).toBe(true)

    await actEvent(() => progred.menuAction("delete"))

    expect(root.querySelector(".graphNode.rootGraphNode")).toBe(null)
    expect(root.querySelector(".guidEditor")).toBe(null)
    expect(root.textContent).toContain("root")
  })

  it("closes placeholder completion when arrow navigation leaves the text input", async () => {
    const {root} = await launchEditor()

    await typeAndEnter(root, "new Module")
    const text = textInput(root)
    await actEvent(() => input(text, "a"))
    await actEvent(() => textInput(root).setSelectionRange(0, 0))
    expect(root.querySelector(".entrylist")).not.toBe(null)

    await actEvent(() => keyDown(text, "ArrowRight"))

    expect(root.querySelector(".entrylist")).toBe(null)
  })
})
