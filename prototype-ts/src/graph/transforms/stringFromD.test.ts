import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { sidFromString } from "../model/ID"
import { Block, CollapseToggle, DIdenticon, DList, DText, GuidEditor, Line, NumberEditor, PlaceholderEditor, StringEditor } from "../render/D"
import { stringFromD } from "./stringFromD"

function cursor() {
  return new Cursor(undefined, "guid-holder", sidFromString("root"))
}

describe("stringFromD", () => {
  it("omits collapse toggles from exported text", () => {
    expect(stringFromD(new Line(new DText("a"), new CollapseToggle(false, () => {}), new DText("b")))).toBe("ab")
  })

  it("prints placeholders and identicons explicitly", () => {
    expect(stringFromD(new Line(new PlaceholderEditor("field", () => [], undefined, {}), new DText(" "), new DIdenticon("1234567890abcdef")))).toBe("[…] [12345678]")
  })

  it("prints editor wrappers as their contents", () => {
    expect(stringFromD(new GuidEditor(cursor(), "guid-node", new DText("Node"), false, {}))).toBe("Node")
    expect(stringFromD(new StringEditor(sidFromString("hello"), "hello", true, {}))).toBe("hello")
    expect(stringFromD(new NumberEditor(42, 42, true, {}))).toBe("42")
  })

  it("does not add an extra blank line for nested blocks", () => {
    expect(stringFromD(new Block(new DText("a"), new Block(new DText("b"))))).toBe("\n  a\n    b")
  })

  it("renders multiline lists with separators", () => {
    expect(stringFromD(new DList("[", [new DText("a"), new DText("b")], "]", ","))).toBe("[\n  a,\n  b ]")
  })
})
