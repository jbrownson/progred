import { dispatch } from "./R"
import { renderEvaluate } from "./renderEvaluate"
import { renderType } from "./renderType"

export const renders = dispatch(renderType, renderEvaluate)
