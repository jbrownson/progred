import { describe, expect, it } from "vitest"
import { GUIDEmptyList, listFromID } from "./graph"
import { arrayFromList, listFromArray } from "./list"
import { sidFromString } from "./model/ID"
import { withTestEnvironment } from "./testHelpers"

describe("list", () => {
  it("creates an empty list from an empty array", () => {
    withTestEnvironment(() => {
      const list = listFromArray([], id => ({id}))

      expect(list).toBeInstanceOf(GUIDEmptyList)
      expect(arrayFromList(list)).toEqual([])
    })
  })

  it("round-trips arrays through graph lists", () => {
    withTestEnvironment(() => {
      const list = listFromArray([{id: sidFromString("a")}, {id: sidFromString("b")}], id => ({id}))

      expect(arrayFromList(list)?.map(({id}) => id)).toEqual([sidFromString("a"), sidFromString("b")])
      expect(listFromID(list.id, id => ({id}))).not.toBe(undefined)
    })
  })
})
