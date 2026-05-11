import { describe, expect, it } from "vitest"
import { sidFromString } from "./ID"
import { garbageCollectGUIDMap, GUIDMap } from "./GUIDMap"

describe("GUIDMap", () => {
  it("sets, gets, and overwrites edges", () => {
    const guidMap = new GUIDMap()
    const label = sidFromString("label")

    guidMap.set("guid-a", label, "guid-b")
    expect(guidMap.get("guid-a", label)).toBe("guid-b")

    guidMap.set("guid-a", label, "guid-c")
    expect(guidMap.get("guid-a", label)).toBe("guid-c")
  })

  it("deletes empty source nodes", () => {
    const guidMap = new GUIDMap()
    const label = sidFromString("label")

    guidMap.set("guid-a", label, "guid-b")
    guidMap.delete("guid-a", label)

    expect(guidMap.edges("guid-a")).toBe(undefined)
  })

  it("garbage collects nodes unreachable from the root, but keeps GUID labels", () => {
    const label = "guid-label"
    const guidMap = new GUIDMap(new Map([
      ["guid-root", new Map([[label, "guid-child"]])],
      [label, new Map([[sidFromString("name"), sidFromString("Label")]])],
      ["guid-child", new Map([[sidFromString("name"), sidFromString("Child")]])],
      ["guid-orphan", new Map([[sidFromString("name"), sidFromString("Orphan")]])]]))

    const collected = garbageCollectGUIDMap(guidMap, "guid-root")

    expect(collected.edges("guid-root")).not.toBe(undefined)
    expect(collected.edges(label)).not.toBe(undefined)
    expect(collected.edges("guid-child")).not.toBe(undefined)
    expect(collected.edges("guid-orphan")).toBe(undefined)
  })
})
