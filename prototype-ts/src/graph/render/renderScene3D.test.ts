import { describe, expect, test } from "vitest"
import { SourceType } from "../Environment"
import { GUIDPolyline3D, GUIDScene3D } from "../graph"
import { withTestEnvironment } from "../testHelpers"
import { renderScene3D } from "./renderScene3D"

describe("renderScene3D", () => {
  test("renders generated Scene 3D nodes", () => withTestEnvironment(() => {
    const scene = GUIDScene3D.new("guid-scene")
    const d = renderScene3D({parent: "guid-workspace", label: "guid-root"}, {id: scene.id, source: {source: SourceType.DocumentType, guid: "guid-scene"}})

    expect(d).not.toBeUndefined() }))

  test("ignores non-scene nodes", () => withTestEnvironment(() => {
    const polyline = GUIDPolyline3D.new("guid-polyline")
    const d = renderScene3D({parent: "guid-workspace", label: "guid-root"}, {id: polyline.id, source: {source: SourceType.DocumentType, guid: "guid-polyline"}})

    expect(d).toBeUndefined() }))
})
