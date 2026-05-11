import { describe, expect, it } from "vitest"
import { _childCursor } from "../cursor/childCursor"
import { Cursor } from "../cursor/Cursor"
import { _get } from "../Environment"
import { ctorField, emptyListCtor, GUIDEmptyList, headField, nonemptyListCtor, tailField } from "../graph"
import { listFromArray } from "../list"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { withTestEnvironment } from "../testHelpers"
import { appendToListCursor, deleteListElemCursor, insertAfterListElemCursor, setCursorToEmptyList } from "./listCursorActions"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("items"), new SparseSpanningTree())
}

describe("listCursorActions", () => {
  it("sets a missing edge to an empty list", () => {
    withTestEnvironment(() => {
      const c = cursor()

      expect(setCursorToEmptyList(c)).toBe(c)
      const listID = _get(c.parent, c.label)

      expect(listID).not.toBe(undefined)
      expect(_get(listID!, ctorField.id)).toBe(emptyListCtor.id)
    })
  })

  it("does not replace an existing edge with an empty list", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      environment.guidMap.set("guid-holder", c.label, "guid-existing")

      expect(setCursorToEmptyList(c)).toBe(undefined)
      expect(_get(c.parent, c.label)).toBe("guid-existing")
    })
  })

  it("appends to an empty list by replacing it with a nonempty list", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const empty = GUIDEmptyList.new()
      environment.guidMap.set("guid-holder", c.label, empty.id)

      const inserted = appendToListCursor(c)
      const newListID = _get(c.parent, c.label)

      expect(inserted?.parent).toBe(newListID)
      expect(inserted?.label).toBe(headField.id)
      expect(_get(newListID!, ctorField.id)).toBe(nonemptyListCtor.id)
      expect(_get(newListID!, tailField.id)).toBe(empty.id)
    })
  })

  it("inserts after a list element", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const list = listFromArray([{id: sidFromString("a")}], id => ({id}))
      environment.guidMap.set("guid-holder", c.label, list.id)
      const headCursor = _childCursor(c, list.id, headField.id)
      const oldTail = _get(list.id, tailField.id)

      const inserted = insertAfterListElemCursor(headCursor)

      expect(inserted?.label).toBe(headField.id)
      expect(_get(inserted!.parent, ctorField.id)).toBe(nonemptyListCtor.id)
      expect(_get(inserted!.parent, tailField.id)).toBe(oldTail)
    })
  })

  it("deletes a list element by replacing the parent tail", () => {
    withTestEnvironment(environment => {
      const c = cursor()
      const list = listFromArray([{id: sidFromString("a")}, {id: sidFromString("b")}], id => ({id}))
      environment.guidMap.set("guid-holder", c.label, list.id)
      const firstHeadCursor = _childCursor(c, list.id, headField.id)
      const oldTail = _get(list.id, tailField.id)

      expect(deleteListElemCursor(firstHeadCursor)).toBe(true)
      expect(_get(c.parent, c.label)).toBe(oldTail)
    })
  })
})
