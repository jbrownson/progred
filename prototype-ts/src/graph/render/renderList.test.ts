import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { SourceType } from "../Environment"
import { GUIDEmptyList, GUIDNonemptyList, headField, tailField } from "../graph"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"
import { D, DList, DText, GuidEditor, SupportsUnderselection } from "./D"
import { renderList } from "./defaultRender"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("list"), new SparseSpanningTree())
}

function findD<A extends D>(d: D, f: (d: D) => d is A): A | undefined {
  return f(d) ? d : d.children.map(child => findD(child, f)).find(d => d !== undefined)
}

describe("renderList", () => {
  it("renders empty lists as empty D lists wrapped in document editor structure", () => {
    withTestEnvironment(() => {
      const list = GUIDEmptyList.new()
      const d = renderList()(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const dList = findD(d!, (d): d is DList => d instanceof DList)

      expect(d).toBeInstanceOf(SupportsUnderselection)
      expect(findD(d!, (d): d is GuidEditor => d instanceof GuidEditor)?.id).toBe(list.id)
      expect(dList?.opening).toBe("[")
      expect(dList?.children).toEqual([])
      expect(dList?.closing).toBe("]")
    })
  })

  it("renders nonempty list heads as children", () => {
    withTestEnvironment(() => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const d = renderList("[", "]", ",", () => new DText("item"))(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const dList = findD(d!, (d): d is DList => d instanceof DList)

      expect(dList?.children.length).toBe(1)
      expect(dList?.children[0].children[0]?.constructor.name).toBe("DText")
    })
  })

  it("fails cyclic lists so default GUID rendering can handle the cycle", () => {
    withTestEnvironment(() => {
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")})
      list.setTail(list)

      expect(renderList()(cursor(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})).toBe(undefined)
    })
  })

  it("renders collapsed nonempty lists without item children", () => {
    withTestEnvironment(() => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const c = cursor()
      c.sparseSpanningTree!.collapsed = true

      const d = renderList()(c, {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const dList = findD(d!, (d): d is DList => d instanceof DList)

      expect(dList?.children).toEqual([])
      expect(dList?.collapseToggle?.collapsed).toBe(true)
    })
  })

  it("clickBefore appends to the empty tail and selects the new head", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const c = cursor()
      const d = renderList()(c, {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      const dList = findD(d!, (d): d is DList => d instanceof DList)

      dList?.clickBefore(1)

      const newTail = environment.selection?.cursor.parent
      expect(environment.selection?.cursor.label).toBe(headField.id)
      expect(newTail).not.toBe(undefined)
      expect(environment.guidMap.get(newTail as string, tailField.id)).toBe(empty.id)
    })
  })
})
