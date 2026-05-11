import { describe, expect, it } from "vitest"
import { load } from "./load"
import { save } from "./save"
import { GUIDMap } from "./GUIDMap"
import { ID, sidFromString } from "./ID"

describe("save/load", () => {
  it("serializes GUID, string, and number IDs", () => {
    const guidMap = new GUIDMap(new Map([
      ["guid-root", new Map<ID, ID>([
        ["guid-label", "guid-target"],
        [sidFromString("string-label"), sidFromString("string-target")],
        [7, 9]])]]))

    expect(save({root: "guid-root", guidMap})).toEqual({
      root: "guid-root",
      guidMap: {
        "guid-root": [
          {label: {guid: "guid-label"}, to: {guid: "guid-target"}},
          {label: {string: "string-label"}, to: {string: "string-target"}},
          {label: {number: 7}, to: {number: 9}}] } })
  })

  it("loads serialized graph edges back into IDs", () => {
    const {root, guidMap} = load({
      root: "guid-root",
      guidMap: {
        "guid-root": [
          {label: {guid: "guid-label"}, to: {guid: "guid-target"}},
          {label: {string: "name"}, to: {string: "Root"}},
          {label: {number: 1}, to: {number: 2}}] } })

    expect(root).toBe("guid-root")
    expect(guidMap.get("guid-root", "guid-label")).toBe("guid-target")
    expect(guidMap.get("guid-root", sidFromString("name"))).toBe(sidFromString("Root"))
    expect(guidMap.get("guid-root", 1)).toBe(2)
  })

  it("round-trips saved graphs", () => {
    const guidMap = new GUIDMap(new Map([
      ["guid-root", new Map([[sidFromString("name"), sidFromString("Root")]])]]))

    const loaded = load(save({root: "guid-root", guidMap}))

    expect(loaded.root).toBe("guid-root")
    expect(loaded.guidMap.get("guid-root", sidFromString("name"))).toBe(sidFromString("Root"))
  })
})
