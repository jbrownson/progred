import { describe, expect, it } from "vitest"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { getCollapsed, setCollapsed } from "./setCollapsed"

describe("setCollapsed", () => {
  it("sets collapse state on cursors with an existing sparse tree", () => {
    const cursor = new Cursor(undefined, "guid-root", sidFromString("root"), new SparseSpanningTree())

    setCollapsed(cursor, true)
    expect(getCollapsed(cursor)).toBe(true)

    setCollapsed(cursor, false)
    expect(getCollapsed(cursor)).toBe(false)
  })

  it("creates sparse tree entries for child cursors", () => {
    const root = new Cursor(undefined, "guid-root", sidFromString("root"), new SparseSpanningTree())
    const child = _childCursor(root, "guid-child", sidFromString("child"))

    setCollapsed(child, true)
    const refreshedChild = _childCursor(root, "guid-child", sidFromString("child"))

    expect(getCollapsed(refreshedChild)).toBe(true)
  })
})
