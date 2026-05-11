import { describe, expect, it } from "vitest"
import { SourceType } from "../Environment"
import { MapIDMap } from "../model/MapIDMap"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { appendCopyResult, copyResultForID } from "./Copy"
import { GUIDMap } from "../model/GUIDMap"

describe("Copy", () => {
  it("copies atomic IDs without adding graph data", () => {
    withTestEnvironment(() => {
      const copy = copyResultForID(sidFromString("hello"))

      expect(copy.root).toBe(sidFromString("hello"))
      expect(copy.remap.size).toBe(0)
      expect(copy.guidMap.map.size).toBe(0)
    })
  })

  it("copies document GUIDs deeply and remaps document GUIDs", () => {
    withTestEnvironment(environment => {
      environment.guidMap.set("guid-a", sidFromString("child"), "guid-b")
      environment.guidMap.set("guid-b", sidFromString("name"), sidFromString("Child"))

      const copy = copyResultForID("guid-a")
      const rootCopy = copy.remap.get("guid-a")
      const childCopy = copy.remap.get("guid-b")

      expect(copy.root).toBe(rootCopy)
      expect(rootCopy).not.toBe(undefined)
      expect(rootCopy).not.toBe("guid-a")
      expect(childCopy).not.toBe(undefined)
      expect(copy.guidMap.get(rootCopy!, sidFromString("child"))).toBe(childCopy)
      expect(copy.guidMap.get(childCopy!, sidFromString("name"))).toBe(sidFromString("Child"))
    })
  })

  it("cuts off cycles by reusing already allocated copies", () => {
    withTestEnvironment(environment => {
      environment.guidMap.set("guid-a", sidFromString("self"), "guid-a")

      const copy = copyResultForID("guid-a")
      const rootCopy = copy.root

      expect(copy.remap.get("guid-a")).toBe(rootCopy)
      expect(copy.guidMap.get(rootCopy as string, sidFromString("self"))).toBe(rootCopy)
    })
  })

  it("leaves library GUIDs as references", () => {
    const libraries = new Map([[
      "library",
      {
        idMap: new MapIDMap(new Map([["guid-lib", new Map([[sidFromString("name"), sidFromString("Library")]])]])),
        root: "guid-lib" }]])

    withTestEnvironment(() => {
      const copy = copyResultForID("guid-lib")

      expect(copy.root).toBe("guid-lib")
      expect(copy.remap.size).toBe(0)
      expect(copy.guidMap.map.size).toBe(0)
    }, {libraries})
  })

  it("appends copy results without mutating either input", () => {
    const lhs = {
      root: "guid-root",
      remap: new Map([["guid-a", "guid-a-copy"]]),
      guidMap: new GUIDMap(new Map([["guid-a-copy", new Map([[sidFromString("name"), sidFromString("A")]])]])) }
    const rhs = {
      root: "guid-other",
      remap: new Map([["guid-b", "guid-b-copy"]]),
      guidMap: new GUIDMap(new Map([["guid-b-copy", new Map([[sidFromString("name"), sidFromString("B")]])]])) }

    const appended = appendCopyResult(lhs, rhs)

    expect(appended.root).toBe("guid-root")
    expect(appended.remap.get("guid-a")).toBe("guid-a-copy")
    expect(appended.remap.get("guid-b")).toBe("guid-b-copy")
    expect(appended.guidMap.get("guid-a-copy", sidFromString("name"))).toBe(sidFromString("A"))
    expect(appended.guidMap.get("guid-b-copy", sidFromString("name"))).toBe(sidFromString("B"))
    expect(lhs.remap.has("guid-b")).toBe(false)
    expect(lhs.guidMap.edges("guid-b-copy")).toBe(undefined)
  })

  it("throws when appending conflicting remaps", () => {
    expect(() => appendCopyResult(
      {root: "guid-a", remap: new Map([["guid-a", "guid-copy-a"]]), guidMap: new GUIDMap()},
      {root: "guid-a", remap: new Map([["guid-a", "guid-copy-b"]]), guidMap: new GUIDMap()}))
      .toThrow()
  })
})
