import { dispatch } from "./R"
import { renderEvaluate } from "./renderEvaluate"
import { renderLoadJSON } from "./renderLoadJSON"
import { renderType } from "./renderType"

export const renders = dispatch(renderType, renderLoadJSON, renderEvaluate)