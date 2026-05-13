import { dText, line } from "./DLayout"
import { renderListCurly, renderListParens } from "./defaultRender"
import { algebraicTypeCtor, atomicTypeCtor, ctorCtor } from "../graph"
import { dispatch } from "./R"
import { renderIfAlgebraicType, renderIfCtor, renderIfField, renderIfListType } from "../renderIfs"
import { renderNameShallow } from "./renderNameShallow"

const renderAlgebraicTypeName = renderNameShallow(algebraicTypeCtor)
const renderCtorName = renderNameShallow(ctorCtor)
const renderAtomicTypeName = renderNameShallow(atomicTypeCtor)
const renderCtorOrAlgebraicTypeName = dispatch(renderAlgebraicTypeName, renderCtorName, renderAtomicTypeName)

const renderAlgebraicType = renderIfAlgebraicType((descendName, descendCtorOrAlgebraicTypes) =>
  line(dText("data "), descendName, dText(" = "), descendCtorOrAlgebraicTypes), {ctorOrAlgebraicTypes: renderListParens(" |", dispatch(renderAlgebraicTypeName, renderAtomicTypeName))} )

function renderListType() { return renderIfListType(descendType => line(descendType, dText("[]")), {type: renderCtorOrAlgebraicTypeName}) }

const renderCtor = renderIfCtor((descendName, descendFields) => line(descendName, dText(" "), descendFields), {fields: renderListCurly()})

function _renderField() { return renderIfField((descendName, descendType) => line(descendName, dText(" ∷ "), descendType), {type: renderCtorOrAlgebraicTypeName}) }

export const renderType = dispatch(renderAlgebraicType, renderListType(), renderCtor, _renderField())
