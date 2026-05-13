import { describe, expect, it } from "vitest"
import { notifyScrollListeners, registerScrollListener } from "./ScrollListeners"

describe("ScrollListeners", () => {
  it("notifies registered listeners and stops after unregistering", () => {
    let notifications = 0
    const unregister = registerScrollListener(() => notifications++)

    notifyScrollListeners()
    unregister()
    notifyScrollListeners()

    expect(notifications).toBe(1)
  })
})
