import { describe, expect, it } from "vitest"
import { _get } from "../Environment"
import { ctorCtor, ctorField, nameField } from "../graph"
import { GUIDMap } from "../model/GUIDMap"
import { ID, sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { buildEdgeLabelEntries, buildEntries } from "./buildEntries"

describe("buildEntries", () => {
  it("offers random node and string entries for raw placeholders", () => {
    withTestEnvironment(() => {
      const emptyEntries = buildEntries(undefined, () => {})("").map(({a}) => a.string)
      const searchEntries = buildEntries(undefined, () => {})("abc").map(({a}) => a.string)

      expect(emptyEntries).toContain("random node")
      expect(searchEntries).toContain("\"abc\"")
    })
  })

  it("creates new constructor instances through new entries", () => {
    const ctor = "guid-widget-ctor"
    const guidMap = new GUIDMap(new Map([
      [ctor, new Map([[ctorField.id, ctorCtor.id], [nameField.id, sidFromString("Widget")]])]]))

    withTestEnvironment(environment => {
      environment.rootViews.setRoot({id: ctor})
      let created: ID = "guid-unset"
      const entry = buildEntries(undefined, id => { created = id() })("new Widget").find(({a}) => a.string === "new Widget")?.a

      expect(entry).not.toBe(undefined)
      entry?.action()
      expect(_get(created, ctorField.id)).toBe(ctor)
    }, {guidMap})
  })

  it("does not offer constructor creation entries for edge labels", () => {
    withTestEnvironment(() => {
      const entries = buildEdgeLabelEntries(() => {})("").map(({a}) => a.string)

      expect(entries).toContain("random node")
      expect(entries.find(entry => entry.startsWith("new "))).toBe(undefined)
    })
  })

  it("includes loaded named things from the document and marks them document-local", () => {
    const root = "guid-root"
    const named = "guid-named"
    const guidMap = new GUIDMap(new Map([
      ["guid-root-views", new Map()],
      [root, new Map([[sidFromString("child"), named]])],
      [named, new Map([[nameField.id, sidFromString("Named Thing")]])]]))

    withTestEnvironment(environment => {
      environment.rootViews.setRoot({id: root})
      const entry = buildEntries(undefined, () => {})("Named").find(({a}) => a.string === "Named Thing")?.a

      expect(entry?.external).toBe(false)
      expect(entry?.matching).toBe(true)
    }, {guidMap})
  })

  it("marks named library roots as external", () => {
    withTestEnvironment(() => {
      const entry = buildEntries(undefined, () => {})("Library").find(({a}) => a.string === "Library")?.a

      expect(entry?.external).toBe(true)
    }, {
      libraries: new Map([[
        "Library",
        {
          idMap: {edges: () => new Map([[nameField.id, sidFromString("Library")]]), get: (_id, label) => label === nameField.id ? sidFromString("Library") : undefined},
          root: "guid-lib-root" }]])})
  })
})
