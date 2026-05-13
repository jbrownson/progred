import { logS } from "../../lib/debug"
import { bindMaybe, maybeFromException } from "../../lib/Maybe"
import { JavaScriptProgram } from "../graph"
import { javascriptFromGraph } from "./javascriptFromGraph"

function runInContext(javascript: string, sandboxObject: Record<string, unknown>): any {
  if (typeof window === "undefined" || !window.progred) throw new Error("JavaScript execution host is unavailable")
  return window.progred.runJavascript(javascript, sandboxObject)
}

export function runJavascript(javascriptProgram: JavaScriptProgram, sandboxObject: Record<string, unknown> = {}): any {
  return bindMaybe(javascriptFromGraph(javascriptProgram),
    javascript => maybeFromException(() => runInContext(logS("javascript", javascript), sandboxObject)) )}
