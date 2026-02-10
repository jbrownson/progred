import { join } from "../lib/Array"
import { lexCompare } from "../lib/lexCompare"
import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, maybeToArray } from "../lib/Maybe"
import { Entry } from "./Entry"
import { _get, set, Source, SourceType } from "./Environment"
import { defaultFilter, Match, sortFilter } from "./filters"
import { Ctor, ctorField, emptyListCtor, matchType, nonemptyListCtor, numberAtomicType, stringAtomicType, Type } from "./graph"
import { generateGUID, ID, nidFromNumber, sidFromString } from "./ID"
import { LoadedNamedThing, loadedNamedThings } from "./loadedNamedThings"
import { algebraicTypeHasCtor, typeIsOrHasAtomicType, typeMatches } from "./typeMatches"

function entryForID(name: string, id: ID, source: Source, cursorType: Maybe<Type>, action: (id: () => ID) => void): Entry {
  let matching = fromMaybe(mapMaybe(cursorType, cursorType => typeMatches(id, cursorType)), () => true)
  return {
    string: name,
    disambiguation: bindMaybe(bindMaybe(_get(id, ctorField.id), Ctor.fromID), ctor => ctor.name),
    matching,
    action: () => action(() => id),
    external: fromMaybe(mapMaybe(source, source => source.source === SourceType.LibraryType), () => true),
    magic: false }}

function newEntryForData(name: string, id: ID, cursorType: Maybe<Type>, action: (id: () => ID) => void): Maybe<Entry> {
  return bindMaybe(Ctor.fromID(id), ctor => ({
    string: `new ${fromMaybe(ctor.name, () => "[unnamed]")}`,
    action: () => action(() => { let guid = generateGUID(); set(guid, ctorField.id, ctor.id); return guid }),
    matching: fromMaybe(mapMaybe(cursorType, type => matchType(type,
      algebraicType => algebraicTypeHasCtor(algebraicType, ctor),
      listType => ctor.id === nonemptyListCtor.id || ctor.id === emptyListCtor.id,
      _ctor => ctor.id === _ctor.id,
      atomicType => false )), () => true),
    external: false, new: true, magic: false }))}

function numberMagicEntry(searchString: string, typeMatchesNumber: boolean, action: (id: () => ID) => void): Entry[] {
  let number = +searchString
  return isNaN(+number) || searchString === "" ? [] : [{
    string: searchString,
    action: () => action(() => nidFromNumber(number)),
    matching: typeMatchesNumber,
    external: true, magic: true }]}

function stringMagicEntry(searchString: string, typeMatchesString: boolean, action: (id: () => ID) => void): Entry[] {
  return [{
    string: `"${searchString}"`,
    action: () => action(() => sidFromString(searchString)),
    matching: typeMatchesString,
    external: true, magic: true }]}

function newEntries(loadedNamedThings: LoadedNamedThing[], cursorType: Maybe<Type>, action: (id: () => ID) => void): Entry[] {
  return join(loadedNamedThings.map(({name, id, source}) => maybeToArray(newEntryForData(name, id, cursorType, action)))) }

function dataEntries(loadedNamedThings: LoadedNamedThing[], cursorType: Maybe<Type>, action: (id: () => ID) => void): Entry[] {
  return loadedNamedThings.map(({name, id, source}) => entryForID(name, id, source, cursorType, action)) }

function compareEntries(lhs: Entry, rhs: Entry): number {
  return lexCompare(lhs, rhs, (lhs, rhs) => +rhs.matching - +lhs.matching, (lhs, rhs) => +lhs.magic - +rhs.magic) }

export function buildEntries(type: Maybe<Type>, action: (id: () => ID) => void): (needle: string) => { a: Entry, matches: Match[] }[] {
  function _defaultFilter(needle: string, entries: Entry[]) {
    return sortFilter(defaultFilter<Entry>(), ({a:lhs}, {a:rhs}) => compareEntries(lhs, rhs))(entries, entry => entry.string, needle).accepted }
  let _loadedNamedThings = loadedNamedThings().sort((loadedNamedThing0, loadedNamedThing1) => loadedNamedThing0.name.localeCompare(loadedNamedThing1.name))
  let newDataEntries = [...newEntries(_loadedNamedThings, type, action), ...dataEntries(_loadedNamedThings, type, action)]
  let typeMatchesNumber = maybe(type, () => true, type => typeIsOrHasAtomicType(type, numberAtomicType))
  let typeMatchesString = maybe(type, () => true, type => typeIsOrHasAtomicType(type, stringAtomicType))
  return needle => _defaultFilter(needle, [...newDataEntries, ...numberMagicEntry(needle, typeMatchesNumber, action), ...stringMagicEntry(needle, typeMatchesString, action)]) }