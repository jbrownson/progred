import { describe, expect, it } from "vitest"
import { _delete, _get, get, set, setOrDelete, SourceType } from "./Environment"
import { workspaceRootField, workspaceViewField } from "./workspace"
import { MapIDMap } from "./model/MapIDMap"
import { sidFromString } from "./model/ID"
import { withTestEnvironment } from "./testHelpers"

describe("Environment", () => {
  it("reads document edges through _get and get", () => {
    withTestEnvironment(() => {
      set("guid-node", sidFromString("label"), "guid-target")

      expect(_get("guid-node", sidFromString("label"))).toBe("guid-target")
      expect(get("guid-node", sidFromString("label"))).toEqual({
        id: "guid-target",
        source: {source: SourceType.DocumentType, guid: "guid-node"} })
    })
  })

  it("reads library edges when document edges are absent", () => {
    const libraries = new Map([[
      "library",
      {
        idMap: new MapIDMap(new Map([["guid-lib", new Map([[sidFromString("label"), "guid-target"]])]])),
        root: "guid-lib" }]])

    withTestEnvironment(() => {
      expect(_get("guid-lib", sidFromString("label"))).toBe("guid-target")
      expect(get("guid-lib", sidFromString("label"))?.source).toEqual({source: SourceType.LibraryType})
    }, {libraries})
  })

  it("prefers document edges over library edges", () => {
    const libraries = new Map([[
      "library",
      {
        idMap: new MapIDMap(new Map([["guid-node", new Map([[sidFromString("label"), "guid-library-target"]])]])),
        root: "guid-node" }]])

    withTestEnvironment(() => {
      set("guid-node", sidFromString("label"), "guid-document-target")

      expect(_get("guid-node", sidFromString("label"))).toBe("guid-document-target")
      expect(get("guid-node", sidFromString("label"))?.source).toEqual({source: SourceType.DocumentType, guid: "guid-node"})
    }, {libraries})
  })

  it("deletes document edges", () => {
    withTestEnvironment(() => {
      set("guid-node", sidFromString("label"), "guid-target")
      _delete("guid-node", sidFromString("label"))

      expect(_get("guid-node", sidFromString("label"))).toBe(undefined)
    })
  })

  it("sets or deletes document edges from maybe values", () => {
    withTestEnvironment(() => {
      setOrDelete("guid-node", sidFromString("label"), "guid-target")
      expect(_get("guid-node", sidFromString("label"))).toBe("guid-target")

      setOrDelete("guid-node", sidFromString("label"), undefined)
      expect(_get("guid-node", sidFromString("label"))).toBe(undefined)
    })
  })

  it("stores workspace root and view outside the document graph", () => {
    withTestEnvironment(environment => {
      set(environment.workspace.id, workspaceRootField.id, "guid-root")
      set(environment.workspace.id, workspaceViewField.id, "guid-view")

      expect(_get(environment.workspace.id, workspaceRootField.id)).toBe("guid-root")
      expect(_get(environment.workspace.id, workspaceViewField.id)).toBe("guid-view")
      expect(environment.guidMap.edges(environment.workspace.id)).toBe(undefined)

      _delete(environment.workspace.id, workspaceViewField.id)

      expect(_get(environment.workspace.id, workspaceViewField.id)).toBe(undefined)
    })
  })
})
