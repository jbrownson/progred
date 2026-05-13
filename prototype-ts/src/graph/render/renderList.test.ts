import * as React from "react"
import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { SourceType } from "../Environment"
import { GUIDEmptyList, GUIDNonemptyList, headField, tailField } from "../graph"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { dText, dKind, type D } from "./D"
import { renderList } from "./defaultRender"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("list"))
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

describe("renderList", () => {
  it("renders empty lists as empty D lists wrapped in document editor structure", () => {
    withTestEnvironment(() => {
      const list = GUIDEmptyList.new()
      const d = renderList()(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const listD = findD(d!, d => dKind(d) === "list")

      expect(findD(d!, d => dKind(d) === "supportsUnderselection")).not.toBe(undefined)
      expect((findD(d!, d => dKind(d) === "guidEditor")?.props as any)?.id).toBe(list.id)
      expect((listD?.props as any)?.opening).toBe("[")
      expect((listD?.props as any)?.children).toEqual([])
      expect((listD?.props as any)?.closing).toBe("]")
    })
  })

  it("renders nonempty list heads as children", () => {
    withTestEnvironment(() => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const d = renderList("[", "]", ",", () => dText("item"))(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const listD = findD(d!, d => dKind(d) === "list")
      const children = (listD?.props as any)?.children as D[]

      expect(children.length).toBe(1)
      expect((findD(children[0], d => dKind(d) === "text")?.props as any)?.string).toBe("item")
      expect((listD?.props as any)?.insertionPoints[1].requiresMeta).toBe(true)
    })
  })

  it("fails cyclic lists so default GUID rendering can handle the cycle", () => {
    withTestEnvironment(() => {
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")})
      list.setTail(list)

      expect(renderList()(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})).toBe(undefined)
    })
  })

  it("renders nonempty lists through local collapsible state", () => {
    withTestEnvironment(() => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)

      const d = renderList()(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const collapsible = findD(d!, d => dKind(d) === "collapsible")
      const collapsedList = findD((collapsible!.props as any).render(true, () => {}), d => dKind(d) === "list")

      expect((collapsible?.props as any)?.defaultCollapsed).toBe(false)
      expect((collapsedList?.props as any)?.children).toEqual([])
      expect(((collapsedList?.props as any)?.collapseToggle.props as any).collapsed).toBe(true)
    })
  })

  it("insertion points append to the empty tail", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const c = cursor()
      const d = renderList()(c, {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const listD = findD(d!, d => dKind(d) === "list")

      ;(listD?.props as any)?.insertionPoints[1].editorCommands.commit?.(sidFromString("b"))

      const newTail = environment.guidMap.get(list.id, tailField.id)
      expect(newTail).not.toBe(undefined)
      expect(environment.guidMap.get(newTail as string, headField.id)).toBe(sidFromString("b"))
      expect(environment.guidMap.get(newTail as string, tailField.id)).toBe(empty.id)
    })
  })
})
