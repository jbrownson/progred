import { describe, expect, it } from "vitest"
import { guidFromID, idFromJSON, matchID, nidFromID, nidFromNumber, numberFromID, sidFromID, sidFromString, stringFromID, stringFromSID } from "./ID"

describe("ID", () => {
  it("round-trips string IDs", () => {
    const sid = sidFromString("hello")

    expect(sid).toBe("sid:hello")
    expect(stringFromSID(sid)).toBe("hello")
    expect(stringFromID(sid)).toBe("hello")
    expect(sidFromID(sid)).toBe(sid)
  })

  it("round-trips number IDs", () => {
    const nid = nidFromNumber(42)

    expect(nid).toBe(42)
    expect(numberFromID(nid)).toBe(42)
    expect(nidFromID(nid)).toBe(42)
  })

  it("distinguishes GUIDs from strings and numbers", () => {
    expect(guidFromID("guid-a")).toBe("guid-a")
    expect(guidFromID("sid:a")).toBe(undefined)
    expect(guidFromID(1)).toBe(undefined)
  })

  it("dispatches by ID shape", () => {
    expect(matchID("guid-a", () => "guid", () => "sid", () => "nid")).toBe("guid")
    expect(matchID("sid:a", () => "guid", () => "sid", () => "nid")).toBe("sid")
    expect(matchID(1, () => "guid", () => "sid", () => "nid")).toBe("nid")
  })

  it("accepts JSON strings and numbers as IDs", () => {
    expect(idFromJSON("guid-a")).toBe("guid-a")
    expect(idFromJSON("sid:a")).toBe("sid:a")
    expect(idFromJSON(1)).toBe(1)
    expect(idFromJSON({guid: "guid-a"})).toBe(undefined)
  })
})
