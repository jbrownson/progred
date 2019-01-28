import { bindMaybe, booleanFromMaybe, fromMaybe, maybe, Maybe } from "../lib/Maybe"
import { _get } from "./Environment"
import { AlgebraicType, AtomicType, Ctor, ctorField, emptyListCtor, matchCtorOrAlgebraicType, matchType, nonemptyListCtor, numberAtomicType, stringAtomicType, Type } from "./graph"
import { ID, matchID } from "./ID"

function _algebraicTypeHasCtor(algebraicType: AlgebraicType, ctor: Ctor, visited = new Set<ID>()): boolean {
  return visited.has(algebraicType.id) ? false : fromMaybe(bindMaybe(algebraicType.ctorOrAlgebraicTypes, ctorOrAlgebraicTypes =>
    booleanFromMaybe(ctorOrAlgebraicTypes.find(ctorOrAlgebraicType => matchCtorOrAlgebraicType(ctorOrAlgebraicType,
    _ctor => ctor.id === _ctor.id,
    _algebraicType => _algebraicTypeHasCtor(_algebraicType, ctor, visited.add(algebraicType.id)),
    atomicType => false )))), () => false) }

export function algebraicTypeHasCtor(algebraicType: AlgebraicType, ctor: Ctor): boolean { return _algebraicTypeHasCtor(algebraicType, ctor) }

export function typeMatches(id: ID, type: Type): Maybe<boolean> {
  return matchID(id,
    guid => matchType(type,
      algebraicType => maybe(bindMaybe(_get(id, ctorField.id), Ctor.fromID), () => false, ctor => _algebraicTypeHasCtor(algebraicType, ctor)),
      gListType => maybe(_get(id, ctorField.id), () => false, ctorID => ctorID === nonemptyListCtor.id || ctorID === emptyListCtor.id),
      ctor => _get(id, ctorField.id) === ctor.id,
      atomicType => false ),
    sid => typeIsOrHasAtomicType(type, stringAtomicType),
    nid => typeIsOrHasAtomicType(type, numberAtomicType) )}

export function typeIsOrHasAtomicType(type: Type, atomicType: AtomicType): boolean { return _typeIsOrHasAtomicType(type, atomicType) }

function _typeIsOrHasAtomicType(type: Type, atomicType: AtomicType, visited = new Set<ID>()): boolean {
  return matchType(type,
    algebraicType => !visited.has(algebraicType.id) &&
      maybe(algebraicType.ctorOrAlgebraicTypes,
        () => false,
        ctorOrAlgebraicTypes => booleanFromMaybe(ctorOrAlgebraicTypes.find(ctorOrAlgebraicType => matchCtorOrAlgebraicType(ctorOrAlgebraicType,
          ctor => false,
          __algebraicType => _typeIsOrHasAtomicType(__algebraicType, atomicType, visited.add(algebraicType.id)),
          _atomicType => atomicType.id === _atomicType.id )))),
    listType => false,
    ctor => false,
    _atomicType => atomicType.id === _atomicType.id )}