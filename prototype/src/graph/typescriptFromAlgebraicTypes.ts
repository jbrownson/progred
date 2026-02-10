import { concatMap, removeDupesBy } from "../lib/Array"
import { assert } from "../lib/assert"
import { maybe, Maybe, maybeMap, maybeToArray, nothing, unsafeUnwrapMaybe } from "../lib/Maybe"
import { camelCase, indent, pascalCase } from "../lib/string"
import { ctorsAtomicsFromCtorOrAlgebraicTypes } from "./ctorsAtomicsFromCtorOrAlgebraicTypes"
import { AlgebraicType, AtomicType, Ctor, Field, listAlgebraicType, matchType, nonemptyListCtor, numberAtomicType, stringAtomicType, Type } from "./graph"
import { guidFromID } from "./ID"

export function typescriptFromCtorOrAlgebraicTypes(algebraicTypes: AlgebraicType[], ctors: Ctor[], atomicTypes: AtomicType[]): string {
  return [
    [
      'import { altMaybe, bindMaybe, fromMaybe, mapMaybe, Maybe, nothing } from "../lib/Maybe"',
      'import { arrayFromList } from "./arrayFromList"',
      'import { _get, set, setOrDelete } from "./Environment"',
      'import { generateGUID, GUID, guidFromID, ID, NID, nidFromID, nidFromNumber, numberFromID, numberFromNID, SID, sidFromID, sidFromString, stringFromID, stringFromSID } from "./ID"',
      'import { listFromArray } from "./listFromArray"' ].join("\n"),
    [
      'function checkCtor(id: ID, forCtor: Ctor): boolean {',
      '  return fromMaybe(bindMaybe(_get(id, ctorField.id), ctor => ctor === forCtor.id), () => false) }',
      '',
      'function checkAlgebraicType<A>(id: ID, xs: {ctor: Ctor, f: (id: ID) => A}[]): Maybe<A> {',
      '  return mapMaybe(_get(id, ctorField.id), _ctor => mapMaybe(xs.find(({ctor}) => ctor.id === _ctor), ({f}) => f(id))) }',
      '',
      'export function checkString(id: ID): Maybe<HasSID> { return mapMaybe(sidFromID(id), sid => new HasSID(sid)) }',
      'export function checkNumber(id: ID): Maybe<HasNID> { return mapMaybe(nidFromID(id), nid => new HasNID(nid)) }',
      '',
      'function getList<A extends HasID>(_this: HasID, field: Field, f: (id: ID) => Maybe<A>) {',
      '  return bindMaybe(_get(_this.id, field.id), x => mapMaybe(listFromID(x, f), arrayFromList)) }',
      '',
      'function get<A>(_this: HasID, field: Field, f: (id: ID) => Maybe<A>) { return bindMaybe(_get(_this.id, field.id), f) }',
      'function _set<A, B extends HasGUID>(_this: B, field: Field, f: (a: A) => ID, a: Maybe<A>) { setOrDelete(_this.id, field.id, mapMaybe(a, f)); return _this }',
      'function setList<A extends HasID, B extends HasGUID>(_this: B, field: Field, f: (a: A) => ID, as: Maybe<A[]>) {',
      '  setOrDelete(_this.id, field.id, mapMaybe(as, as => listFromArray<HasID>(as, id => ({id})).id)); return _this }',
      '',
      'function getID(hasID: HasID) { return hasID.id }',
      '',
      'export type HasID = { readonly id: ID }',
      'export type HasGUID = { readonly id: GUID }',
      '',
      'export class HasSID {',
      '  constructor(public readonly id: SID) {}',
      '  get string() { return stringFromSID(this.id) } }',
      '',
      'export class HasNID {',
      '  constructor(public readonly id: NID) {}',
      '  get number() { return numberFromNID(this.id) } }',
      '',
      'export class NonemptyList<A extends HasID = HasID> {',
      '  constructor(public readonly id: ID, public f: (id: ID) => Maybe<A>) {}',
      '  static fromID<A extends HasID>(id: ID, f: (id: ID) => Maybe<A>): Maybe<NonemptyList<A>> { return checkCtor(id, nonemptyListCtor) ? new NonemptyList(id, f) : nothing }',
      '  get guidList() { return mapMaybe(guidFromID(this.id), guid => new GUIDNonemptyList(guid, this.f)) }',
      '  get head() { return get(this, headField, this.f) }',
      '  get tail() { return get(this, tailField, id => listFromID(id, this.f)) } }',
      'export class GUIDNonemptyList<A extends HasID> extends NonemptyList<A> {',
      '  constructor(public readonly id: GUID, public f: (id: ID) => Maybe<A>) { super(id, f) }',
      '  static new<A extends HasID>(f: (id: ID) => Maybe<A>, guid: GUID = generateGUID()) { set(guid, ctorField.id, nonemptyListCtor.id); return new GUIDNonemptyList(guid, f) }',
      '  get guidList() { return this }',
      '  setHead(head: Maybe<A>) { return _set(this, headField, getID, head) }',
      '  setTail(tail: Maybe<List<A>>) { return _set(this, tailField, getID, tail) } }' ].join('\n'),
    ...maybeMap(ctors.filter(ctor => ctor.id !== nonemptyListCtor.id), typescriptFromCtor),
    [
      'export type List<A extends HasID = HasID> = NonemptyList<A> | EmptyList',
      'export type GUIDList<A extends HasID = HasID> = GUIDNonemptyList<A> | GUIDEmptyList',
      'export function listFromID<A extends HasID>(id: ID, f: (id: ID) => Maybe<A>): Maybe<List<A>> { return checkAlgebraicType<List<A>>(id, [{ctor: nonemptyListCtor, f: id => new NonemptyList(id, f)}, {ctor: emptyListCtor, f: id => new EmptyList(id)}]) }',
      'export function matchList<A extends HasID, B>(x: List<A>, nonemptyListF: (x: NonemptyList<A>) => B, emptyListF: (x: EmptyList) => B) { return x instanceof NonemptyList ? nonemptyListF(x) : emptyListF(x) }',
      'export function nonemptyListFromList<A extends HasID>(x: List<A>) { return x instanceof NonemptyList ? x : nothing }',
      'export function emptyListFromList<A extends HasID>(x: List<A>) { return x instanceof EmptyList ? x : nothing }'
    ].join("\n"),
    ...maybeMap(algebraicTypes.filter(algebraicType => algebraicType.id !== listAlgebraicType.id), typescriptFromAlgebraicType),
    wrappers(algebraicTypes, ctors, atomicTypes) ].join('\n\n')}

function typescriptFromType(type: Maybe<Type>, arraysForLists: boolean = true): string {
  return maybe(type, () => "HasID", type => matchType(type,
    symbolFromAlgebraicType,
    listType => arraysForLists ? `${typescriptFromType(listType.type, false)}[]` : `List<${typescriptFromType(listType.type, false)}>`,
    symbolFromCtor,
    atomicType => atomicType.id === numberAtomicType.id ? "number" : atomicType.id === stringAtomicType.id ? "string" : "unknown atomic type" ))}

function fromIDFromType(type: Maybe<Type>): string {
  return maybe(type, () => "id => ({id})", type => matchType(type,
    fromIDFromAlgebraicType,
    listType => `id => listFromID(id, ${fromIDFromType(listType.type)})`,
    ctor => `${symbolFromCtor(ctor)}.fromID`,
    fromIDFromAtomicType ))}

function fromIDFromAtomicType(atomicType: AtomicType) {
  return atomicType.id === numberAtomicType.id ? "numberFromID" : atomicType.id === stringAtomicType.id ? "stringFromID" : "unknown atomic type" }

function fromIDFromAlgebraicType(algebraicType: AlgebraicType) {
  return unsafeUnwrapMaybe(algebraicType.ctorOrAlgebraicTypes).length <= 1 ? `${symbolFromAlgebraicType(algebraicType)}.fromID` : `${camelCase(unsafeUnwrapMaybe(algebraicType.name))}FromID` }

function symbolFromCtor(ctor: Ctor) { return pascalCase(unsafeUnwrapMaybe(ctor.name)) }
function symbolFromAtomic(atomicType: AtomicType) { return atomicType.id === stringAtomicType.id ? "HasSID" : atomicType.id === numberAtomicType.id ? "HasNID" : "unsupported atomic type" }
function symbolFromField(field: Field) { return camelCase(unsafeUnwrapMaybe(field.name)) }
function symbolFromAlgebraicType(algebraicType: AlgebraicType) { return pascalCase(unsafeUnwrapMaybe(algebraicType.name)) }
function appendToLast(strings: string[], string: string) { return strings.length > 0 ? [...strings.slice(0, -1), strings[strings.length - 1] + string] : [string] }

function typescriptFromCtor(ctor: Ctor): string {
  return [
    `export class ${symbolFromCtor(ctor)} {`,
    ...indent([
      "constructor(public readonly id: ID) {}",
      `static fromID(id: ID) { return checkCtor(id, ${camelCase(unsafeUnwrapMaybe(ctor.name))}Ctor) ? new ${symbolFromCtor(ctor)}(id) : nothing }`,
      `get guid${symbolFromCtor(ctor)}() { return mapMaybe(guidFromID(this.id), guid => new GUID${symbolFromCtor(ctor)}(guid)) }`,
      ...appendToLast(unsafeUnwrapMaybe(ctor.fields).map(field => `get ${symbolFromField(field)}(): Maybe<${typescriptFromType(field.type)}> { return ${
        maybe(field.type, () => `get(this, ${symbolFromField(field)}Field, id => ({id}))`, type => matchType(type,
          algebraicType => `get(this, ${symbolFromField(field)}Field, ${fromIDFromAlgebraicType(algebraicType)})`,
          listType => `getList(this, ${symbolFromField(field)}Field, ${fromIDFromType(listType.type)})`,
          ctor => `get(this, ${symbolFromField(field)}Field, ${symbolFromCtor(ctor)}.fromID)`,
          atomicType => `get(this, ${symbolFromField(field)}Field, ${fromIDFromAtomicType(atomicType)})` ))} }` ), " }" )]),
    `export class GUID${symbolFromCtor(ctor)} extends ${symbolFromCtor(ctor)} {`,
    ...indent([
      "constructor(public readonly id: GUID) { super(id) }",
      `static new(guid: GUID = generateGUID()) { set(guid, ctorField.id, ${camelCase(unsafeUnwrapMaybe(ctor.name))}Ctor.id); return new GUID${symbolFromCtor(ctor)}(guid) }`,
      `get guid${symbolFromCtor(ctor)}() { return this }`,
      ...appendToLast(unsafeUnwrapMaybe(ctor.fields).map(field => `${camelCase(`set ${unsafeUnwrapMaybe(field.name)}`)}(x: Maybe<${typescriptFromType(field.type)}>) { return ${
        maybe(field.type,
          () => `_set(this, ${symbolFromField(field)}Field, getID, x)`,
          type => matchType(type,
            algebraicType => `_set(this, ${symbolFromField(field)}Field, getID, x)`,
            listType => `setList(this, ${symbolFromField(field)}Field, getID, x)`,
            ctor => `_set(this, ${symbolFromField(field)}Field, getID, x)`,
            atomicType => `_set(this, ${symbolFromField(field)}Field, ${atomicType.id === numberAtomicType.id ? "nidFromNumber" : atomicType.id === stringAtomicType.id ? "sidFromString" : "unknown atomic type"}, x)` ))} }`), " }" )])].join("\n")}

function typescriptFromAlgebraicType(algebraicType: AlgebraicType): Maybe<string> {
  let {ctors, atomics} = ctorsAtomicsFromCtorOrAlgebraicTypes(unsafeUnwrapMaybe(algebraicType.ctorOrAlgebraicTypes)) // TODO also capture atomics here and use them
  let name = unsafeUnwrapMaybe(algebraicType.name)
  function genMatches(ctors: Ctor[], atomics: AtomicType[]): string {
    function _genMatches(...xs: {fCall: string, predicate: string}[]): string {
      assert(xs.length >= 1)
      return xs.length === 1
        ? xs[0].fCall
        : `${xs[0].predicate} ? ${xs[0].fCall} : ${_genMatches(...xs.slice(1))}` }
    return _genMatches(
      ...ctors.map(ctor => ({fCall: `${camelCase(unsafeUnwrapMaybe(ctor.name))}F(x)`, predicate: `x instanceof ${symbolFromCtor(ctor)}`})),
      ...atomics.map(atomic => ({
        fCall: atomic.id === stringAtomicType.id ? "stringF(x.string)" : atomic.id === numberAtomicType.id ? "numberF(x.number)" : "unsupported atomic type",
        predicate: atomic.id === stringAtomicType.id ? "x instanceof HasSID" : atomic.id === numberAtomicType.id ? "x instanceof HasNID" : "unsupported atomic type"})) )}
  let checkAlgebraicType = `checkAlgebraicType<${symbolFromAlgebraicType(algebraicType)}>(id, [${
    ctors.map(ctor => `{ctor: ${camelCase(unsafeUnwrapMaybe(ctor.name))}Ctor, f: id => new ${symbolFromCtor(ctor)}(id)}`).join(", ")}])`
  let check = atomics.length > 0
    ? `altMaybe(${[checkAlgebraicType, ...atomics.map(atomic => `() => check${pascalCase(unsafeUnwrapMaybe(atomic.name))}(id)`)].join(', ')})`
    : checkAlgebraicType
  return ctors.length < 2 ? nothing : [
    `export type ${symbolFromAlgebraicType(algebraicType)} = ${[...ctors.map(symbolFromCtor), ...atomics.map(symbolFromAtomic)].join(" | ")}`,
    ...atomics.length === 0 ? [`export type GUID${symbolFromAlgebraicType(algebraicType)} = ${ctors.map(ctor => `GUID${symbolFromCtor(ctor)}`).join(" | ")}`] : [],
    `export function ${camelCase(name)}FromID(id: ID): Maybe<${symbolFromAlgebraicType(algebraicType)}> { return ${check} }`,
    ...maybeToArray(ctors.length < 1 ? nothing : `export function match${symbolFromAlgebraicType(algebraicType)}<A>(x: ${symbolFromAlgebraicType(algebraicType)}${[
      ...ctors.map(ctor => `, ${camelCase(unsafeUnwrapMaybe(ctor.name))}F: (x: ${symbolFromCtor(ctor)}) => A`),
      ...atomics.map(atomic => `, ${camelCase(unsafeUnwrapMaybe(atomic.name))}F: (x: ${unsafeUnwrapMaybe(atomic.name)}) => A`)].join("")}) { return ${genMatches(ctors, atomics)} }`),
    ...ctors.map(ctor => `export function ${camelCase(unsafeUnwrapMaybe(ctor.name))}From${symbolFromAlgebraicType(algebraicType)}(x: ${symbolFromAlgebraicType(algebraicType)}) { return x instanceof ${symbolFromCtor(ctor)} ? x : nothing }`),
    ...atomics.map(atomic => `export function ${camelCase(unsafeUnwrapMaybe(atomic.name))}From${symbolFromAlgebraicType(algebraicType)}(x: ${symbolFromAlgebraicType(algebraicType)}) { return ${
      atomic.id === stringAtomicType.id ? 'stringFromID' : atomic.id === numberAtomicType.id ? "numberFromID" : "unknown atomic type"}(x.id) }`) ].join('\n') }

function wrappers(algebraicTypes: AlgebraicType[], ctors: Ctor[], atomicTypes: AtomicType[]): string {
  let fields = removeDupesBy(concatMap(ctors, ctor => unsafeUnwrapMaybe(ctor.fields)), field => field.id).sort((a, b) => unsafeUnwrapMaybe(a.name).localeCompare(unsafeUnwrapMaybe(b.name)))
  return [
    `export const`,
    [...[
      ...fields.map(field => ({x: "Field", name: unsafeUnwrapMaybe(field.name), id: field.id})),
      ...ctors.map(ctor => ({x: "Ctor", name: unsafeUnwrapMaybe(ctor.name), id: ctor.id})),
      ...algebraicTypes.map(algebraicType => ({x: "AlgebraicType", name: unsafeUnwrapMaybe(algebraicType.name), id: algebraicType.id})),
      ...atomicTypes.map(atomicType => ({x: "AtomicType", name: unsafeUnwrapMaybe(atomicType.name), id: atomicType.id})) ]
      .map(({x, name, id}) => `  ${camelCase(name)}${x} = new GUID${x}("${unsafeUnwrapMaybe(guidFromID(id))}")`)
    ].join(",\n")].join('\n') }