import { logS } from "../lib/debug"
import { bindMaybe, maybeFromException } from "../lib/Maybe"
import { JavaScriptProgram, Module } from "./graph"
import { libraries } from "./libraries/libraries"
import { toTextFromRenders } from "./toText"

function runInContext(javascript: string, sandboxObject: Record<string, unknown>): any {
  if (typeof window === "undefined" || !window.progred) throw new Error("JavaScript execution host is unavailable")
  return window.progred.runJavascript(javascript, sandboxObject)
}

export function runJavascript(javascriptProgram: JavaScriptProgram, sandboxObject: Record<string, unknown> = {}): any {
  return bindMaybe(bindMaybe(libraries.get("JavaScript"), library => bindMaybe(Module.fromID(library.root), module => bindMaybe(module.renderCtors, renderCtors =>
    toTextFromRenders(renderCtors)(javascriptProgram.id, 0) ))),
    javascript => maybeFromException(() => runInContext(logS("javascript", javascript), sandboxObject)) )}
