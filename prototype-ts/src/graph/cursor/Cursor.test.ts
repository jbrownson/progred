import { describe, expect, it } from "vitest"
import { Descend, DText, Label, Line } from "../render/D"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { childCursor, _childCursor } from "./childCursor"
import { Cursor, cursorsEqual } from "./Cursor"
import { cursorFromD } from "./cursorFromD"
import { cursorHasCycle } from "./cursorHasCycle"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"

function rootCursor() {
  return new Cursor(undefined, "guid-root", sidFromString("root"), new SparseSpanningTree())
}

describe("Cursor", () => {
  it("compares cursor paths by parent and label", () => {
    const a = rootCursor()
    const b = rootCursor()

    expect(cursorsEqual(a, b)).toBe(true)
    expect(cursorsEqual(_childCursor(a, "guid-child", sidFromString("name")), _childCursor(b, "guid-child", sidFromString("name")))).toBe(true)
    expect(cursorsEqual(_childCursor(a, "guid-child", sidFromString("name")), _childCursor(b, "guid-child", sidFromString("other")))).toBe(false)
  })

  it("builds child cursors from graph edges", () => {
    withTestEnvironment(environment => {
      const cursor = rootCursor()
      environment.guidMap.set("guid-root", sidFromString("child"), "guid-child")

      const child = childCursor(cursor, sidFromString("child"))

      expect(child?.parentCursor).toBe(cursor)
      expect(child?.parent).toBe("guid-child")
      expect(child?.label).toBe(sidFromString("child"))
    })
  })

  it("detects repeated edges in a cursor path as cycles", () => {
    const root = rootCursor()
    const child = _childCursor(root, "guid-child", sidFromString("child"))
    const cycle = _childCursor(child, "guid-child", sidFromString("child"))

    expect(cursorHasCycle(root)).toBe(false)
    expect(cursorHasCycle(child)).toBe(false)
    expect(cursorHasCycle(cycle)).toBe(true)
  })

  it("finds the nearest cursor-bearing D node", () => {
    const cursor = rootCursor()
    const dText = new DText("x")
    const descend = new Descend(cursor, new Line(new Label(cursor, dText)), false)

    expect(cursorFromD(dText)).toBe(cursor)
    expect(cursorFromD(descend)).toBe(cursor)
  })

})
