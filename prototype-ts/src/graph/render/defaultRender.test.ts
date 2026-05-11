import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { SourceType } from "../Environment"
import { nameField } from "../graph"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"
import { D, DIdenticon, DText, GuidEditor, PlaceholderEditor, StringEditor, SupportsUnderselection } from "./D"
import { commitCommands, defaultRender, renderDocumentGuidEditor, renderField, renderString } from "./defaultRender"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("root"), new SparseSpanningTree())
}

function hasD(d: D, f: (d: D) => boolean): boolean {
  return f(d) || d.children.some(child => hasD(child, f))
}

describe("defaultRender", () => {
  it("renders missing edges as placeholders with commit commands", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      environment.selection = {cursor: c}

      const d = defaultRender(c, undefined)

      expect(d).toBeInstanceOf(PlaceholderEditor)
      expect((d as PlaceholderEditor).selectedState).not.toBe(undefined)
      ;(d as PlaceholderEditor).editorCommands.commit?.("guid-target")
      expect(environment.guidMap.get("guid-holder", sidFromString("root"))).toBe("guid-target")
    })
  })

  it("wraps writable document GUIDs in a GuidEditor and SupportsUnderselection", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const d = renderDocumentGuidEditor(c, {id: "guid-node", source: {source: SourceType.DocumentType, guid: "guid-node"}}, new DText("node"))

      expect(d).toBeInstanceOf(SupportsUnderselection)
      expect((d as SupportsUnderselection).child).toBeInstanceOf(GuidEditor)
    })
  })

  it("does not wrap library GUIDs in document editor commands", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const d = renderDocumentGuidEditor(c, {id: "guid-node", source: {source: SourceType.LibraryType}}, new DText("node"))

      expect(d).toBeInstanceOf(DText)
    })
  })

  it("commits IDs through cursor commands", () => {
    withTestEnvironment(environment => {
      const c = cursor()

      commitCommands(c).commit?.("guid-target")

      expect(environment.guidMap.get("guid-holder", sidFromString("root"))).toBe("guid-target")
    })
  })

  it("renders named labels as text in fields", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const label = "guid-label"
      environment.guidMap.set(label, nameField.id, sidFromString("Label"))

      const d = renderField(c, "guid-node", label)

      expect(hasD(d, d => d instanceof DText && d.string === "Label")).toBe(true)
    })
  })

  it("renders unnamed GUID labels as identicons in fields", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const label = "guid-label"
      const d = renderField(c, "guid-node", label)

      expect(hasD(d, d => d instanceof DIdenticon && d.guid === label)).toBe(true)
    })
  })

  it("renders strings as string editors", () => {
    withTestEnvironment(() => {
      const d = renderString(cursor(), sidFromString("hello"), "hello", {source: SourceType.DocumentType, guid: "guid-holder"})

      expect(d).toBeInstanceOf(StringEditor)
      expect((d as StringEditor).editorCommands.copy).not.toBe(undefined)
    })
  })
})
