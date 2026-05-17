import { describe, expect, it } from "vitest"
import { sidFromString } from "../model/ID"
import { block, dList, dText, isBlock, isSingleLine, line } from "./D"
import { descend } from "./R"
import { withTestEnvironment } from "../testHelpers"

describe("D", () => {
  it("marks line and block layout in D metadata", () => {
    expect(isSingleLine(line(dText("lhs"), dText("rhs")))).toBe(true)
    expect(isSingleLine(block(line(dText("lhs"), dText("rhs"))))).toBe(false)
    expect(isBlock(line(dText("lhs"), dText("rhs")))).toBe(false)
    expect(isBlock(block(line(dText("lhs"), dText("rhs"))))).toBe(true)
  })

  it("marks list layout in D metadata", () => {
    expect(isSingleLine(dList("[", [], "]", ","))).toBe(true)
    expect(isSingleLine(dList("[", [dText("x")], "]", ","))).toBe(true)
    expect(isSingleLine(dList("[", [dText("x"), dText("y")], "]", ","))).toBe(false)
    expect(isSingleLine(dList("[", [block(dText("x"))], "]", ","))).toBe(false)
  })

  it("renders descends immediately", () => {
    withTestEnvironment(() => {
      let renders = 0
      const d = descend("guid-parent", sidFromString("child"), () => {
        renders++
        return dText("projected") })

      expect(isSingleLine(d)).toBe(true)
      expect(renders).toBe(1)
    })
  })
})
