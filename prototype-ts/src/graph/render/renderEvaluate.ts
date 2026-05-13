import { mapMaybe, maybeToArray } from "../../lib/Maybe"
import { block, dText, line } from "./Projection"
import { renderIfEvaluate } from "../renderIfs"
import { runJavascript } from "../transforms/runJavascript"

export const renderEvaluate = renderIfEvaluate((statements, evaluate) => line(dText("Evaluate"), block(
  statements,
  ...maybeToArray(mapMaybe(evaluate.javascriptProgram, javascriptProgram => dText(`${runJavascript(javascriptProgram)}`))) )))
