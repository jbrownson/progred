import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { _get } from "../Environment"
import { MapIDMap } from "../model/MapIDMap"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"
import { deleteCursor } from "./deleteCursor"

function cursor() {
  return new Cursor(undefined, "guid-node", sidFromString("label"), new SparseSpanningTree())
}

describe("deleteCursor", () => {
  it("deletes document edges at the selected cursor", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      environment.guidMap.set("guid-node", c.label, "guid-target")

      expect(deleteCursor(c)).toBe(true)
      expect(_get("guid-node", c.label)).toBe(undefined)
    })
  })

  it("does not delete library edges", () => {
    const libraries = new Map([[
      "library",
      {
        idMap: new MapIDMap(new Map([["guid-lib", new Map([[sidFromString("label"), "guid-target"]])]])),
        root: "guid-lib" }]])

    withTestEnvironment(() => {
      const c = new Cursor(undefined, "guid-lib", sidFromString("label"), new SparseSpanningTree())

      expect(deleteCursor(c)).toBe(false)
      expect(_get("guid-lib", sidFromString("label"))).toBe("guid-target")
    }, {libraries})
  })
})
