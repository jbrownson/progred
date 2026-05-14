import { describe, expect, it } from "vitest"
import { emptyCyclePath, stepCyclePath } from "./CyclePath"
import { sidFromString } from "../model/ID"

describe("CyclePath", () => {
  it("records IDs as the render path is stepped", () => {
    const step = stepCyclePath(emptyCyclePath(), "guid-node")

    expect(step.hasCycle).toBe(false)
    expect(step.path.has("guid-node")).toBe(true)
  })

  it("detects an ID already in the render path", () => {
    const first = stepCyclePath(emptyCyclePath(), "guid-node")
    const second = stepCyclePath(first.path, "guid-node")

    expect(second.hasCycle).toBe(true)
    expect(second.path).toBe(first.path)
  })

  it("uses JavaScript value equality for all ID cases", () => {
    const path = stepCyclePath(stepCyclePath(emptyCyclePath(), sidFromString("name")).path, 42).path

    expect(stepCyclePath(path, sidFromString("name")).hasCycle).toBe(true)
    expect(stepCyclePath(path, 42).hasCycle).toBe(true)
  })
})
