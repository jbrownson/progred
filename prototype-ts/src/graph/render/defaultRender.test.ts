import * as React from "react"
import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { SourceType } from "../Environment"
import { nameField } from "../graph"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { dText, dKind, type D } from "./D"
import { defaultRender, renderString } from "./defaultRender"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("root"))
}

function childDs(d: D): D[] {
  const props = d.props as Record<string, unknown>
  return [
    ...dKind(d) === "collapsible" ? [(props.render as (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D)(false, () => {})] : [],
    ...Object.values(props).flatMap(value =>
    Array.isArray(value)
      ? value.filter(React.isValidElement) as D[]
      : React.isValidElement(value) ? [value as D] : [])]
}

function findD(d: D, f: (d: D) => boolean): D | undefined {
  return f(d) ? d : childDs(d).map(child => findD(child, f)).find(d => d !== undefined)
}

describe("defaultRender", () => {
  it("renders missing edges as placeholders", () => {
    withTestEnvironment(() => {
      const c = cursor()

      const d = defaultRender(c, undefined)

      expect(dKind(d)).toBe("placeholderEditor")
      expect((d.props as any).placeholderEditor.activeState).toBe(undefined)
      expect((d.props as any).placeholderEditor.entries("").length).toBeGreaterThan(0)
    })
  })

  it("uses the edge context field name for placeholder labels", () => {
    withTestEnvironment(() => {
      const d = defaultRender(cursor(), undefined, {fieldName: "root"})

      expect((d.props as any).placeholderEditor.name).toBe("root")
    })
  })

  it("wraps writable document GUIDs in a GuidEditor and SupportsUnderselection", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const d = renderDocumentGuidEditor(c, {id: "guid-node", source: {source: SourceType.DocumentType, guid: "guid-node"}}, dText("node"))

      expect(dKind(d)).toBe("supportsUnderselection")
      expect(dKind((d.props as any).child)).toBe("guidEditor")
    })
  })

  it("does not wrap library GUIDs in document editor commands", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const d = renderDocumentGuidEditor(c, {id: "guid-node", source: {source: SourceType.LibraryType}}, dText("node"))

      expect(dKind(d)).toBe("text")
    })
  })

  it("renders named labels as text in fields", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const label = "guid-label"
      environment.guidMap.set(label, nameField.id, sidFromString("Label"))

      const d = renderField(c, "guid-node", label)

      expect(findD(d, d => dKind(d) === "text" && (d.props as any).string === "Label")).not.toBe(undefined)
    })
  })

  it("renders unnamed GUID labels as identicons in fields", () => {
    withTestEnvironment(() => {
      const c = cursor()
      const label = "guid-label"
      const d = renderField(c, "guid-node", label)

      expect(findD(d, d => dKind(d) === "identicon" && (d.props as any).guid === label)).not.toBe(undefined)
    })
  })

  it("renders strings as string editors", () => {
    withTestEnvironment(() => {
      const d = renderString(cursor(), sidFromString("hello"), "hello", {source: SourceType.DocumentType, guid: "guid-holder"})

      expect(dKind(d)).toBe("stringEditor")
      expect((d.props as any).stringEditor.editorCommands.copy).not.toBe(undefined)
    })
  })
})
