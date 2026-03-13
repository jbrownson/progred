import { mapMaybe, maybeToArray } from "../lib/Maybe"
import { Block, DText, Line } from "./D"
import { renderIfEvaluate } from "./renderIfs"
import { runJavascript } from "./runJavascript"

export const renderEvaluate = renderIfEvaluate((statements, evaluate) => new Line(new DText("Evaluate"), new Block(
  statements,
  ...maybeToArray(mapMaybe(evaluate.javascriptProgram, javascriptProgram => new DText(`${runJavascript(javascriptProgram)}`))) )))