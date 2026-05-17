import { describe, expect, it } from "vitest"
import type { Maybe } from "../../lib/Maybe"
import type { Environment } from "../Environment"
import { withTestEnvironment } from "../testHelpers"
import { dText } from "./D"
import { createProjection } from "./project"

function recordFieldNames(fieldNames: Maybe<string>[]): Environment["defaultRender"] {
  return (_edge, _sourceID, edgeContext) => {
    fieldNames.push(edgeContext?.fieldName)
    return dText("") }
}

describe("createProjection", () => {
  it("creates a root descend with no workspace view", () => {
    const fieldNames: Maybe<string>[] = []

    withTestEnvironment(() => {
      const {rootDescend, viewDescend} = createProjection()

      expect(rootDescend).not.toBe(undefined)
      expect(viewDescend).toBe(undefined)
    }, {root: "guid-root", defaultRender: recordFieldNames(fieldNames)})

    expect(fieldNames).toEqual(["root"])
  })

  it("creates a view descend from the workspace view", () => {
    const fieldNames: Maybe<string>[] = []

    withTestEnvironment(() => {
      const {rootDescend, viewDescend} = createProjection()

      expect(rootDescend).not.toBe(undefined)
      expect(viewDescend).not.toBe(undefined)
    }, {root: "guid-root", view: "guid-view", defaultRender: recordFieldNames(fieldNames)})

    expect(fieldNames).toEqual(["root", "view"])
  })
})
