import * as React from "react"
import { describe, expect, it } from "vitest"
import { SourceType } from "../Environment"
import { nameField } from "../graph"
import type { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { dText, dKind, type D } from "./D"
import { defaultRender, renderString } from "./defaultRender"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"

function edge(): Edge {
  return {parent: "guid-holder", label: sidFromString("root")}
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

function childDsWithoutOpeningDefaultCollapsed(d: D): D[] {
  const props = d.props as Record<string, unknown>
  return [
    ...dKind(d) === "collapsible" && !(props as any).defaultCollapsed ? [(props.render as (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => D)(false, () => {})] : [],
    ...Object.values(props).flatMap(value =>
    Array.isArray(value)
      ? value.filter(React.isValidElement) as D[]
      : React.isValidElement(value) ? [value as D] : [])]
}

function allDWithoutOpeningDefaultCollapsed(d: D): D[] {
  return [d, ...childDsWithoutOpeningDefaultCollapsed(d).flatMap(allDWithoutOpeningDefaultCollapsed)]
}

describe("defaultRender", () => {
  it("renders missing edges as placeholders", () => {
    withTestEnvironment(() => {
      const e = edge()

      const d = defaultRender(e, undefined)

      expect(dKind(d)).toBe("placeholderEditor")
      expect((d.props as any).placeholderEditor.activeState).toBe(undefined)
      expect((d.props as any).placeholderEditor.entries("").length).toBeGreaterThan(0)
    })
  })

  it("uses the edge context field name for placeholder labels", () => {
    withTestEnvironment(() => {
      const d = defaultRender(edge(), undefined, {fieldName: "root"})

      expect((d.props as any).placeholderEditor.name).toBe("root")
    })
  })

  it("wraps writable document GUIDs in a GuidEditor and SupportsUnderselection", () => {
    withTestEnvironment(() => {
      const e = edge()
      const d = renderDocumentGuidEditor(e, {id: "guid-node", source: {source: SourceType.DocumentType, guid: "guid-node"}}, dText("node"))

      expect(dKind(d)).toBe("supportsUnderselection")
      expect(dKind((d.props as any).child)).toBe("guidEditor")
    })
  })

  it("does not wrap library GUIDs in document editor commands", () => {
    withTestEnvironment(() => {
      const e = edge()
      const d = renderDocumentGuidEditor(e, {id: "guid-node", source: {source: SourceType.LibraryType}}, dText("node"))

      expect(dKind(d)).toBe("text")
    })
  })

  it("renders named labels as text in fields", () => {
    withTestEnvironment(environment => {
      const label = "guid-label"
      environment.guidMap.set(label, nameField.id, sidFromString("Label"))

      const d = renderField("guid-node", label)

      expect(findD(d, d => dKind(d) === "text" && (d.props as any).string === "Label")).not.toBe(undefined)
    })
  })

  it("renders unnamed GUID labels as identicons in fields", () => {
    withTestEnvironment(() => {
      const label = "guid-label"
      const d = renderField("guid-node", label)

      expect(findD(d, d => dKind(d) === "identicon" && (d.props as any).guid === label)).not.toBe(undefined)
    })
  })

  it("renders strings as string editors", () => {
    withTestEnvironment(() => {
      const d = renderString(edge(), sidFromString("hello"), "hello", {source: SourceType.DocumentType, guid: "guid-holder"})

      expect(dKind(d)).toBe("stringEditor")
      expect((d.props as any).editorCommands.copy).not.toBe(undefined)
    })
  })

  it("defaults indirect cycles collapsed when the same node is reached through different labels", () => {
    withTestEnvironment(environment => {
      const a = "guid-a"
      const b = "guid-b"
      environment.guidMap.set(a, sidFromString("left"), b)
      environment.guidMap.set(b, sidFromString("right"), a)
      const d = defaultRender(edge(), {id: a, source: {source: SourceType.DocumentType, guid: a}})

      expect(allDWithoutOpeningDefaultCollapsed(d).filter(d => dKind(d) === "collapsible" && (d.props as any).defaultCollapsed).length).toBe(1)
    }, {defaultRender})
  })

  it("does not treat sibling references to the same node as cycles", () => {
    withTestEnvironment(environment => {
      const root = "guid-root"
      const shared = "guid-shared"
      environment.guidMap.set(root, sidFromString("left"), shared)
      environment.guidMap.set(root, sidFromString("right"), shared)
      environment.guidMap.set(shared, nameField.id, sidFromString("Shared"))
      const d = defaultRender(edge(), {id: root, source: {source: SourceType.DocumentType, guid: root}})

      expect(allDWithoutOpeningDefaultCollapsed(d).filter(d => dKind(d) === "collapsible" && (d.props as any).defaultCollapsed).length).toBe(0)
    }, {defaultRender})
  })
})
