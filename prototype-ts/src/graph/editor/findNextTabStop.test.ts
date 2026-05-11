import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { Descend, DText, Line } from "../render/D"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { findNextTabStop, findTabStop } from "./findNextTabStop"
import { doTab } from "./keyHandler"

function rootCursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("root"), new SparseSpanningTree())
}

function tree() {
  const root = rootCursor()
  const filled = new Cursor(root, "guid-root", sidFromString("filled"), new SparseSpanningTree())
  const missing = new Cursor(root, "guid-root", sidFromString("missing"), new SparseSpanningTree())
  const rootDescend = new Descend(root, new Line(
    new Descend(filled, new DText("filled"), undefined, false),
    new Descend(missing, new DText("missing"), undefined, false)), undefined, false)
  return {root, filled, missing, rootDescend}
}

describe("findNextTabStop", () => {
  it("finds missing edges as tab stops", () => {
    withTestEnvironment(environment => {
      const {root, filled, missing, rootDescend} = tree()
      environment.guidMap.set("guid-holder", root.label, "guid-root")
      environment.guidMap.set("guid-root", filled.label, "guid-filled")

      expect(findTabStop(rootDescend, 1)?.cursor).toBe(missing)
      expect(findNextTabStop(rootDescend, 1)?.cursor).toBe(missing)
    })
  })

  it("searches backward for reverse tab stops", () => {
    withTestEnvironment(environment => {
      const {root, filled, missing, rootDescend} = tree()
      environment.guidMap.set("guid-holder", root.label, "guid-root")
      environment.guidMap.set("guid-root", filled.label, "guid-filled")

      expect(findTabStop(rootDescend, -1)?.cursor).toBe(missing)
    })
  })

  it("does not clear selection when tab has nowhere to go", () => {
    withTestEnvironment(environment => {
      const {root, filled, missing, rootDescend} = tree()
      environment.guidMap.set("guid-holder", root.label, "guid-root")
      environment.guidMap.set("guid-root", filled.label, "guid-filled")
      environment.guidMap.set("guid-root", missing.label, "guid-missing")
      environment.selection = {cursor: filled}

      expect(doTab(false, rootDescend, undefined)).toBe(false)
      expect(environment.selection?.cursor).toBe(filled)
    })
  })
})
