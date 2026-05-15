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
    setMenuItemEnabled: () => {},
    setMenuItemChecked: () => {},
    onMenuAction: callback => {
      menuAction = callback
      return () => { if (menuAction === callback) menuAction = undefined }},
  }
  return {
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
    expect(root.querySelector(".listInsertionPoint")).not.toBe(null)

    await actEvent(() => click(root.querySelector(".listInsertionPoint")!))
    await typeAndEnter(root, "hello")

    expect(root.textContent).not.toBe("")
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

  it("closes placeholder completion when arrow navigation leaves the text input", async () => {
    const {root} = await launchEditor()

    await typeAndEnter(root, "new Module")
    const text = textInput(root)
    await actEvent(() => input(text, "a"))
    await actEvent(() => textInput(root).setSelectionRange(0, 0))
    expect(root.querySelector(".entrylist")).not.toBe(null)

    await actEvent(() => keyDown(text, "ArrowLeft"))

    expect(root.querySelector(".entrylist")).toBe(null)
  })
})
