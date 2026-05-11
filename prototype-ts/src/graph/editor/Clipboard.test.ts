import { describe, expect, it } from "vitest"
import { GUIDMap } from "../model/GUIDMap"
import { copyIDFromClipboardText, clipboardStringForCopyResult, idFromClipboardText } from "./Clipboard"

describe("Clipboard", () => {
  it("reads the reference ID from clipboard JSON", () => {
    expect(idFromClipboardText(JSON.stringify({id: "guid-a"}))).toBe("guid-a")
    expect(idFromClipboardText(JSON.stringify({id: 42}))).toBe(42)
  })

  it("ignores missing or invalid clipboard JSON", () => {
    expect(idFromClipboardText(undefined)).toBe(undefined)
    expect(idFromClipboardText("{")).toBe(undefined)
    expect(idFromClipboardText(JSON.stringify({id: {guid: "guid-a"}}))).toBe(undefined)
  })

  it("round-trips a copy result for atomic IDs", () => {
    const text = clipboardStringForCopyResult(42, {root: 42, remap: new Map(), guidMap: new GUIDMap()})

    expect(idFromClipboardText(text)).toBe(42)
    expect(copyIDFromClipboardText(text)).toBe(42)
  })
})
