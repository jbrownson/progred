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
  element.dispatchEvent(new KeyboardEvent("keydown", {key, bubbles: true, cancelable: true, ...options}))
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
    onMenuAction: () => () => {},
  }
}

describe("graphEditor integration", () => {
  afterEach(() => {
    document.body.replaceChildren()
    vi.restoreAllMocks()
    vi.resetModules()
  })

  it("does not blank the app when inserting into a root empty list", async () => {
    document.body.innerHTML = `<div id="root"></div>`
    installProgred()

    await act(async () => {
      await import("./graphEditor")
      await Promise.resolve()
    })
    const root = document.getElementById("root")!

    await typeAndEnter(root, "new Empty List")
    expect(root.textContent).not.toBe("")
    expect(root.querySelector(".listInsertionPoint")).not.toBe(null)

    await actEvent(() => click(root.querySelector(".listInsertionPoint")!))
    await typeAndEnter(root, "hello")

    expect(root.textContent).not.toBe("")
  })
})
