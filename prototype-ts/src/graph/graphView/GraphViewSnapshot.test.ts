import { describe, expect, it } from "vitest"
import { rootField, nameField, ctorField, fieldCtor, GUIDRootViews } from "../graph"
import { GUIDMap } from "../model/GUIDMap"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { buildGraphViewSnapshot } from "./GraphViewSnapshot"

describe("GraphViewSnapshot", () => {
  it("builds nodes and edges from the document graph", () => {
    const rootViews = new GUIDRootViews("guid-root-views")
    const guidMap = new GUIDMap(new Map([
      [rootViews.id, new Map([[rootField.id, "guid-root"]])],
      ["guid-root", new Map([[sidFromString("child"), "guid-child"]])],
      ["guid-child", new Map([[nameField.id, sidFromString("Child")]])]]))

    withTestEnvironment(() => {
      const snapshot = buildGraphViewSnapshot(guidMap, rootViews, undefined, undefined)

      expect(snapshot.nodes.map(node => node.id).sort()).toEqual(["guid-child", "guid-root", sidFromString("Child")].sort())
      expect(snapshot.nodes.find(node => node.id === "guid-root")?.root).toBe(true)
      expect(snapshot.edges).toEqual([
        {source: "guid-root", label: sidFromString("child"), target: "guid-child", labelText: {parts: [{name: "\"child\"", guid: undefined}]}},
        {source: "guid-child", label: nameField.id, target: sidFromString("Child"), labelText: {parts: [{name: undefined, guid: nameField.id}]}}])
    }, {guidMap, rootViews})
  })

  it("uses active editor edge as secondary graph selection", () => {
    const rootViews = new GUIDRootViews("guid-root-views")
    const label = sidFromString("child")
    const guidMap = new GUIDMap(new Map([
      [rootViews.id, new Map([[rootField.id, "guid-root"]])],
      ["guid-root", new Map([[label, "guid-child"]])]]))

    withTestEnvironment(() => {
      const snapshot = buildGraphViewSnapshot(guidMap, rootViews, {parent: "guid-root", label}, undefined)

      expect(snapshot.selectedNode).toEqual({id: "guid-child", strength: "secondary"})
      expect(snapshot.selectedEdge).toEqual({source: "guid-root", label, strength: "secondary"})
    }, {guidMap, rootViews})
  })

  it("uses graph selection as primary graph selection", () => {
    const rootViews = new GUIDRootViews("guid-root-views")
    const label = sidFromString("child")
    const guidMap = new GUIDMap(new Map([
      [rootViews.id, new Map([[rootField.id, "guid-root"]])],
      ["guid-root", new Map([[label, "guid-child"]])]]))

    withTestEnvironment(() => {
      const snapshot = buildGraphViewSnapshot(guidMap, rootViews, undefined, {kind: "edge", source: "guid-root", label})

      expect(snapshot.selectedNode).toBe(undefined)
      expect(snapshot.selectedEdge).toEqual({source: "guid-root", label, strength: "primary"})
    }, {guidMap, rootViews})
  })

  it("omits Field constructor labels on graph edge labels", () => {
    const rootViews = new GUIDRootViews("guid-root-views")
    const field = "guid-field"
    const guidMap = new GUIDMap(new Map([
      [rootViews.id, new Map([[rootField.id, "guid-root"]])],
      [field, new Map([[ctorField.id, fieldCtor.id], [nameField.id, sidFromString("fieldName")]])],
      ["guid-root", new Map([[field, "guid-child"]])]]))

    withTestEnvironment(() => {
      const snapshot = buildGraphViewSnapshot(guidMap, rootViews, undefined, undefined)

      expect(snapshot.edges.find(edge => edge.label === field)?.labelText.parts).toEqual([{name: "fieldName", guid: field}])
    }, {guidMap, rootViews})
  })
})
