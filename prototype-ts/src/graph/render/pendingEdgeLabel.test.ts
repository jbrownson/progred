import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { sidFromString } from "../model/ID"
import { Block, Line, PlaceholderEditor } from "./D"
import { pendingEdgeLabel } from "./pendingEdgeLabel"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"

function cursor() {
  return new Cursor(undefined, "guid-parent", sidFromString("root"), new SparseSpanningTree())
}

describe("pendingEdgeLabel", () => {
  it("renders nothing unless the selection is a pending edge label", () => {
    withTestEnvironment(environment => {
      const c = cursor()

      environment.selection = {cursor: c}
      expect(pendingEdgeLabel(c, "guid-parent")).toEqual([])

      environment.selection = undefined
      expect(pendingEdgeLabel(c, "guid-parent")).toEqual([])
    })
  })

  it("renders a label placeholder for pending edge labels", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      environment.selection = {cursor: c, pendingEdgeLabel: true}

      const ds = pendingEdgeLabel(c, "guid-parent")
      const placeholder = (ds[0] as Block).children[0] as Line

      expect(ds.length).toBe(1)
      expect(placeholder.children[0]).toBeInstanceOf(PlaceholderEditor)
    })
  })

  it("commits chosen labels by selecting under that label", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      environment.selection = {cursor: c, pendingEdgeLabel: true}
      const placeholder = (((pendingEdgeLabel(c, "guid-parent")[0] as Block).children[0] as Line).children[0] as PlaceholderEditor)

      placeholder.editorCommands.commitID?.(sidFromString("label"))

      expect(environment.selection?.pendingEdgeLabel).toBe(undefined)
      expect(environment.selection?.cursor.parentCursor).toBe(c)
      expect(environment.selection?.cursor.parent).toBe("guid-parent")
      expect(environment.selection?.cursor.label).toBe(sidFromString("label"))
    })
  })
})
