import { DText, Line } from "./D"
import { renderListCurly, renderListParens } from "./defaultRender"
import { algebraicTypeCtor, atomicTypeCtor, ctorCtor } from "./graph"
import { dispatch } from "./R"
import { renderIfAlgebraicType, renderIfCtor, renderIfField, renderIfListType } from "./renderIfs"
import { renderNameShallow } from "./renderNameShallow"

const renderAlgebraicTypeName = renderNameShallow(algebraicTypeCtor)
const renderCtorName = renderNameShallow(ctorCtor)
const renderAtomicTypeName = renderNameShallow(atomicTypeCtor)
const renderCtorOrAlgebraicTypeName = dispatch(renderAlgebraicTypeName, renderCtorName, renderAtomicTypeName)

const renderAlgebraicType = renderIfAlgebraicType((descendName, descendCtorOrAlgebraicTypes) =>
  new Line(new DText("data "), descendName, new DText(" = "), descendCtorOrAlgebraicTypes), {ctorOrAlgebraicTypes: renderListParens(" |", dispatch(renderAlgebraicTypeName, renderAtomicTypeName))} )

function renderListType() { return renderIfListType(descendType => new Line(descendType, new DText("[]")), {type: renderCtorOrAlgebraicTypeName}) }

const renderCtor = renderIfCtor((descendName, descendFields) => new Line(descendName, new DText(" "), descendFields), {fields: renderListCurly()})

function _renderField() { return renderIfField((descendName, descendType) => new Line(descendName, new DText(" ∷ "), descendType), {type: renderCtorOrAlgebraicTypeName}) }

export const renderType = dispatch(renderAlgebraicType, renderListType(), renderCtor, _renderField())