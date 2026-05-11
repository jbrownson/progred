import { describe, expect, it } from "vitest"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"
import { selectedMissingLabels } from "./selectedMissingLabels"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("root"), new SparseSpanningTree())
}

describe("selectedMissingLabels", () => {
  it("includes selected child labels that are not already rendered", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const label = sidFromString("missing")
      environment.selection = {cursor: _childCursor(c, "guid-node", label)}

      expect(selectedMissingLabels(c, "guid-node", [])).toEqual([label])
      expect(selectedMissingLabels(c, "guid-node", [label])).toEqual([])
    })
  })

  it("does not include pending edge label selections", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const label = sidFromString("missing")
      environment.selection = {cursor: _childCursor(c, "guid-node", label), pendingEdgeLabel: true}

      expect(selectedMissingLabels(c, "guid-node", [])).toEqual([])
    })
  })

  it("ignores selections outside the cursor subtree", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const other = new Cursor(undefined, "guid-other", sidFromString("root"), new SparseSpanningTree())
      environment.selection = {cursor: _childCursor(other, "guid-node", sidFromString("missing"))}

      expect(selectedMissingLabels(c, "guid-node", [])).toEqual([])
    })
  })
})
