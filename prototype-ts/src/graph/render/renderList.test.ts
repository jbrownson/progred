import { act } from "react"
import { afterEach, describe, expect, it } from "vitest"
import { mapMaybe } from "../../lib/Maybe"
import { commitIDToActiveElement } from "../editor/EditorCommands"
import { SourceType } from "../Environment"
import { GUIDEmptyList, GUIDNonemptyList, headField, tailField } from "../graph"
import type { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import { withTestEnvironment } from "../testHelpers"
import { emptyCyclePath, stepCyclePath } from "./CyclePath"
import { dText } from "./D"
import { renderList } from "./renderList"
import { renderDForTest } from "./renderTestHelpers"

afterEach(() => {
  document.body.replaceChildren()
})

function edge(): Edge {
  return {parent: "guid-holder", label: sidFromString("list")}
}

describe("renderList", () => {
  it("renders empty lists as editable list UI", () => {
    withTestEnvironment(environment => {
      const list = GUIDEmptyList.new()
      const d = renderList()(edge(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)

      expect(container.querySelector(".guidEditor")).not.toBe(null)
      expect(container.textContent).toContain("[")
      expect(container.textContent).toContain("]")
      expect(container.querySelectorAll(".listInsertionPoint")).toHaveLength(1)
      unmount()
    })
  })

  it("renders nonempty list heads as children", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const d = renderList("[", "]", ",", () => dText("item"))(edge(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)

      expect(container.textContent).toContain("item")
      expect(container.querySelectorAll(".listInsertionPoint")).toHaveLength(2)
      unmount()
    })
  })

  it("fails cyclic lists so default GUID rendering can handle the cycle", () => {
    withTestEnvironment(() => {
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")})
      list.setTail(list)

      expect(renderList()(edge(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})).toBe(undefined)
    })
  })

  it("renders nonempty lists through local collapsible state", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)

      const d = renderList()(edge(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)
      const toggle = container.querySelector(".collapseToggle")
      expect(toggle?.textContent).toBe("▾")

      act(() => (toggle as HTMLElement).click())

      expect(container.querySelector(".collapseToggle")?.textContent).toBe("▸")
      expect(container.querySelector(".collapsedListContents")?.textContent).toBe("...")
      unmount()
    })
  })

  it("renders list heads with a cycle path through the list spine", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const first = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")})
      const second = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("b")})
      const third = GUIDNonemptyList.new(id => ({id})).setHead({id: second.id}).setTail(empty)
      first.setTail(second)
      second.setTail(third)
      const d = renderList("[", "]", ",", (edge, sourceID, edgeContext, cyclePath) =>
        mapMaybe(sourceID, sourceID => dText(stepCyclePath(cyclePath || emptyCyclePath(), sourceID.id).hasCycle ? "cycle" : "not")))(edge(), {id: first.id, source: {source: SourceType.DocumentType, guid: first.id}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)

      expect(container.textContent).toMatch(/not.*not.*cycle/)
      unmount()
    })
  })

  it("insertion points append to the empty tail", () => {
    withTestEnvironment(environment => {
      const empty = GUIDEmptyList.new()
      const list = GUIDNonemptyList.new(id => ({id})).setHead({id: sidFromString("a")}).setTail(empty)
      const d = renderList()(edge(), {id: list.id, source: {source: SourceType.DocumentType, guid: list.id}})
      expect(d).not.toBe(undefined)
      const {container, unmount} = renderDForTest(environment, d!)
      const insertionPoints = container.querySelectorAll(".listInsertionPoint")

      act(() => (insertionPoints[1] as HTMLElement).focus())
      act(() => commitIDToActiveElement(sidFromString("b")))

      const newTail = environment.guidMap.get(list.id, tailField.id)
      expect(newTail).not.toBe(undefined)
      expect(environment.guidMap.get(newTail as string, headField.id)).toBe(sidFromString("b"))
      expect(environment.guidMap.get(newTail as string, tailField.id)).toBe(empty.id)
      unmount()
    })
  })
})
