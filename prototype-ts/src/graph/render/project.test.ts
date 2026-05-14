import { describe, expect, it } from "vitest"
import { withTestEnvironment } from "../testHelpers"
import { dKind } from "./D"
import { createProjection } from "./project"

describe("createProjection", () => {
  it("creates a root descend with no workspace view", () => {
    withTestEnvironment(() => {
      const {rootDescend, viewDescend} = createProjection()

      expect(dKind(rootDescend)).toBe("descend")
      expect(rootDescend.props.edgeContext.fieldName).toBe("root")
      expect(viewDescend).toBe(undefined)
    }, {root: "guid-root"})
  })

  it("creates a view descend from the workspace view", () => {
    withTestEnvironment(() => {
      const {rootDescend, viewDescend} = createProjection()

      expect(dKind(rootDescend)).toBe("descend")
      expect(rootDescend.props.edgeContext.fieldName).toBe("root")
      expect(viewDescend).not.toBe(undefined)
      expect(dKind(viewDescend!)).toBe("descend")
      expect(viewDescend!.props.edgeContext.fieldName).toBe("view")
    }, {root: "guid-root", view: "guid-view"})
  })
})
