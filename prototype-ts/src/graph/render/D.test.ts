import { describe, expect, it } from "vitest"
import { Cursor } from "../cursor/Cursor"
import { sidFromString } from "../model/ID"
import { SparseSpanningTree } from "../SparseSpanningTree"
import { Block, Descend, DText, GuidEditor, Label, Line, SupportsUnderselection, supportsUnderselection } from "./D"

function cursor() {
  return new Cursor(undefined, "guid-root", sidFromString("root"), new SparseSpanningTree())
}

describe("D", () => {
  it("links children back to their parents", () => {
    const lhs = new DText("lhs")
    const rhs = new DText("rhs")
    const line = new Line(lhs, rhs)
    const block = new Block(line)

    expect(lhs.parent).toBe(line)
    expect(rhs.parent).toBe(line)
    expect(line.parent).toBe(block)
  })

  it("keeps underselection support inside a projection boundary", () => {
    expect(supportsUnderselection(new SupportsUnderselection(new DText("x")))).toBe(true)
    expect(supportsUnderselection(new Line(new SupportsUnderselection(new DText("x"))))).toBe(true)
    expect(supportsUnderselection(new Descend(cursor(), new SupportsUnderselection(new DText("x")), undefined, false))).toBe(false)
  })

  it("exposes child lists consistently", () => {
    const c = cursor()
    const child = new DText("x")
    const label = new Label(c, child)
    const guidEditor = new GuidEditor(c, "guid-root", new DText("node"), false, {})

    expect(label.children).toEqual([child])
    expect(guidEditor.children.length).toBe(1)
    expect(new DText("leaf").children).toEqual([])
  })
})
