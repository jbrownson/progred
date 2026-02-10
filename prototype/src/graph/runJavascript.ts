import * as VM from "vm"
import { logS } from "../lib/debug"
import { bindMaybe, maybeFromException } from "../lib/Maybe"
import { JavaScriptProgram, Module } from "./graph"
import { libraries } from "./libraries/libraries"
import { toTextFromRenders } from "./toText"

export function runJavascript(javascriptProgram: JavaScriptProgram, sandboxObject = {}): any {
  return bindMaybe(bindMaybe(libraries.get("JavaScript"), library => bindMaybe(Module.fromID(library.root), module => bindMaybe(module.renderCtors, renderCtors =>
    toTextFromRenders(renderCtors)(javascriptProgram.id, 0) ))),
    javascript => maybeFromException(() => VM.runInNewContext(logS("javascript", javascript), VM.createContext(sandboxObject))) )}