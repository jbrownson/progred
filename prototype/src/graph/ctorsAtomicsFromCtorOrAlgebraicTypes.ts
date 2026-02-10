
import { join } from "../lib/Array"
import { mapMaybe, maybeMap } from "../lib/Maybe"
import { AtomicType, Ctor, CtorOrAlgebraicType, matchCtorOrAlgebraicType } from "./graph"
import { ID } from "./ID"

function _ctorsAtomicsFromCtorOrAlgebraicTypes(ctorsOrAlgebraicTypes: CtorOrAlgebraicType[], visited = new Set<ID>()): {ctors: Ctor[], atomics: AtomicType[]} {
  let x = maybeMap(ctorsOrAlgebraicTypes, ctorsOrAlgebraicType => matchCtorOrAlgebraicType(ctorsOrAlgebraicType,
    ctor => ({ctors: [ctor], atomics: []}),
    algebraicType => visited.has(algebraicType.id) ? {ctors: [], atomics: []}
      : mapMaybe(algebraicType.ctorOrAlgebraicTypes, ctorOrAlgebraicTypes => _ctorsAtomicsFromCtorOrAlgebraicTypes(ctorOrAlgebraicTypes, visited.add(algebraicType.id))),
    atomicType => ({ctors: [], atomics: [atomicType]}) ))
  return {ctors: join(x.map(({ctors}) => ctors)), atomics: join(x.map(({atomics}) => atomics))} }

export function ctorsAtomicsFromCtorOrAlgebraicTypes(ctorsOrAlgebraicTypes: CtorOrAlgebraicType[]): {ctors: Ctor[], atomics: AtomicType[]} { return _ctorsAtomicsFromCtorOrAlgebraicTypes(ctorsOrAlgebraicTypes) }