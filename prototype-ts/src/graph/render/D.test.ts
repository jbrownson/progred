import { describe, expect, it } from "vitest"
import type { Edge } from "../model/Edge"
import { sidFromString } from "../model/ID"
import { block, descendElement, dText, isSingleLine, label, line, dKind } from "./D"
import { descend } from "./R"
import { withTestEnvironment } from "../testHelpers"

function edge(): Edge {
  return {parent: "guid-root", label: sidFromString("root")}
}

describe("D", () => {
  it("marks line and block layout in React D metadata", () => {
    expect(isSingleLine(line(dText("lhs"), dText("rhs")))).toBe(true)
    expect(isSingleLine(block(line(dText("lhs"), dText("rhs"))))).toBe(false)
  })

  it("keeps D kinds on wrappers", () => {
    const e = edge()

    expect(dKind(label(e, dText("x")))).toBe("label")
    expect(dKind(descendElement(e, dText("x"), false))).toBe("descend")
  })

  it("renders descends immediately", () => {
    withTestEnvironment(() => {
      let renders = 0
      const d = descend("guid-parent", sidFromString("child"), () => {
        renders++
        return dText("projected") })

      expect(dKind(d)).toBe("descend")
      expect(renders).toBe(1)
    })
  })
})
