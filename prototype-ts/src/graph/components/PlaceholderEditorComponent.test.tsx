import * as React from "react"
import { act } from "react"
import { flushSync } from "react-dom"
import { createRoot } from "react-dom/client"
import { describe, expect, it } from "vitest"
import { nothing } from "../../lib/Maybe"
import { PlaceholderEditorComponent } from "./PlaceholderEditorComponent"

(globalThis as unknown as {IS_REACT_ACT_ENVIRONMENT: boolean}).IS_REACT_ACT_ENVIRONMENT = true

describe("PlaceholderEditorComponent", () => {
  it("builds entries when activated, not when rendered", () => {
    document.body.replaceChildren()
    const container = document.createElement("div")
    document.body.appendChild(container)
    const root = createRoot(container)
    let builds = 0
    const render = () => root.render(<PlaceholderEditorComponent
      placeholderEditor={{
        name: "placeholder",
        entries: () => {
          builds++
          return () => [] },
        activeState: nothing }}
      editorCommands={{}}
      runE={f => f()} />)

    act(() => flushSync(render))
    expect(builds).toBe(0)

    act(() => (container.querySelector(".uneditable") as HTMLElement).focus())
    expect(builds).toBe(1)

    act(() => flushSync(render))
    expect(builds).toBe(1)

    act(() => root.unmount())
    container.remove()
  })
})
