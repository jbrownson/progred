import { afterEach, beforeEach, describe, expect, it } from "vitest"
import { GUIDExternFunction, GUIDFunctionCall, GUIDJavaScriptProgram, HasNID } from "../graph"
import { withTestEnvironment } from "../testHelpers"
import { runJavascript } from "./runJavascript"

function installJavascriptHost() {
  window.progred = {
    runJavascript: (javascript: string, sandboxObject: Record<string, unknown> = {}) =>
      Function("sandbox", "javascript", "with (sandbox) { return eval(javascript) }")(sandboxObject, javascript),
  } as typeof window.progred
}

describe("runJavascript", () => {
  let oldProgred: typeof window.progred | undefined

  beforeEach(() => {
    oldProgred = window.progred
  })

  afterEach(() => {
    if (oldProgred) window.progred = oldProgred
    else delete (window as unknown as {progred?: typeof window.progred}).progred
  })

  it("passes extern functions through the sandbox object", () => {
    installJavascriptHost()
    withTestEnvironment(() => {
      const add = GUIDExternFunction.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").setName("add")
      const call = GUIDFunctionCall.new("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .setFunction(add)
        .setArguments([new HasNID(2), new HasNID(3)])
      const program = GUIDJavaScriptProgram.new("cccccccccccccccccccccccccccccccc")
        .setStatements([call])

      expect(runJavascript(program, {__extern: {add: (a: number, b: number) => a + b}})).toBe(5)
    })
  })
})
