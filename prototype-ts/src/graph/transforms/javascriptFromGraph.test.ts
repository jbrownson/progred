import { describe, expect, it } from "vitest"
import { GUIDBinaryInline, GUIDExternFunction, GUIDFunctionCall, GUIDFunctionDeclaration, GUIDJavaScriptProgram, GUIDParameter, GUIDProduct, GUIDReturn, HasNID, HasSID } from "../graph"
import { withTestEnvironment } from "../testHelpers"
import { sidFromString } from "../model/ID"
import { javascriptFromGraph } from "./javascriptFromGraph"

function evalJavascript(javascript: string) {
  return Function("javascript", "return eval(javascript)")(javascript)
}

function evalJavascriptWithExtern(javascript: string, __extern: Record<string, unknown>) {
  return Function("__extern", "javascript", "return eval(javascript)")(__extern, javascript)
}

describe("javascriptFromGraph", () => {
  it("uses GUID-based identifiers for declarations, parameters, and references", () => {
    withTestEnvironment(() => {
      const parameter = GUIDParameter.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").setName("bad parameter name")
      const body = GUIDReturn.new("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").setExpression(parameter)
      const declaration = GUIDFunctionDeclaration.new("cccccccccccccccccccccccccccccccc")
        .setName("bad function name")
        .setParameters([parameter])
        .setStatements([body])
      const call = GUIDFunctionCall.new("dddddddddddddddddddddddddddddddd")
        .setFunction(declaration)
        .setArguments([new HasNID(42)])
      const program = GUIDJavaScriptProgram.new("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
        .setStatements([declaration, call])
      const javascript = javascriptFromGraph(program)

      expect(javascript).toContain("function _cccccccccccccccccccccccccccccccc(_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)")
      expect(javascript).toContain("_cccccccccccccccccccccccccccccccc(42)")
      expect(javascript).not.toContain("bad function name")
      expect(javascript).not.toContain("bad parameter name")
      expect(evalJavascript(javascript!)).toBe(42)
    })
  })

  it("escapes plain strings as string literals", () => {
    withTestEnvironment(() => {
      const string = "\"); globalThis.progredInjected = true; (\""
      const program = GUIDJavaScriptProgram.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .setStatements([new HasSID(sidFromString(string))])
      const javascript = javascriptFromGraph(program)

      expect(javascript).toContain(JSON.stringify(string))
      expect(javascript).not.toContain("globalThis.progredInjected = true; (\"")
      expect(evalJavascript(javascript!)).toBe(string)
      expect((globalThis as unknown as {progredInjected?: boolean}).progredInjected).toBe(undefined)
    })
  })

  it("resolves matching display-name strings to scoped GUID identifiers", () => {
    withTestEnvironment(() => {
      const parameter = GUIDParameter.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").setName("n")
      const product = GUIDBinaryInline.new("dddddddddddddddddddddddddddddddd")
        .setLeft(new HasSID(sidFromString("n")))
        .setBinaryOperator(GUIDProduct.new("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"))
        .setRight(new HasNID(2))
      const body = GUIDReturn.new("ffffffffffffffffffffffffffffffff").setExpression(product)
      const declaration = GUIDFunctionDeclaration.new("11111111111111111111111111111111")
        .setName("double")
        .setParameters([parameter])
        .setStatements([body])
      const call = GUIDFunctionCall.new("22222222222222222222222222222222")
        .setFunction(new HasSID(sidFromString("double")))
        .setArguments([new HasNID(5)])
      const program = GUIDJavaScriptProgram.new("33333333333333333333333333333333")
        .setStatements([declaration, call])
      const javascript = javascriptFromGraph(program)

      expect(javascript).toContain("(_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa * 2)")
      expect(javascript).toContain("_11111111111111111111111111111111")
      expect(javascript).not.toContain("double(")
      expect(javascript).not.toContain("\"n\"")
      expect(evalJavascript(javascript!)).toBe(10)
    })
  })

  it("emits extern functions through the extern namespace", () => {
    withTestEnvironment(() => {
      const add = GUIDExternFunction.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").setName("add")
      const call = GUIDFunctionCall.new("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .setFunction(add)
        .setArguments([new HasNID(2), new HasNID(3)])
      const program = GUIDJavaScriptProgram.new("cccccccccccccccccccccccccccccccc")
        .setStatements([call])
      const javascript = javascriptFromGraph(program)

      expect(javascript).toContain("__extern[\"add\"](2, 3)")
      expect(evalJavascriptWithExtern(javascript!, {add: (a: number, b: number) => a + b})).toBe(5)
    })
  })

  it("emits arbitrary extern function names as string keys", () => {
    withTestEnvironment(() => {
      const max = GUIDExternFunction.new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").setName("Math max")
      const call = GUIDFunctionCall.new("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .setFunction(max)
        .setArguments([new HasNID(2), new HasNID(3)])
      const program = GUIDJavaScriptProgram.new("cccccccccccccccccccccccccccccccc")
        .setStatements([call])
      const javascript = javascriptFromGraph(program)

      expect(javascript).toContain("__extern[\"Math max\"](2, 3)")
      expect(evalJavascriptWithExtern(javascript!, {"Math max": Math.max})).toBe(3)
    })
  })
})
