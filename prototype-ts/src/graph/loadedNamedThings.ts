import { join, removeDupesBy } from "../lib/Array"
import { bindMaybe, booleanFromMaybe, mapMaybe, maybeMap, maybeToArray } from "../lib/Maybe"
import { edges, environment, get, Source, SourceType } from "./Environment"
import { nameField } from "./graph"
import { guidFromID, ID, stringFromID } from "./ID"

export type LoadedNamedThing = {name: string, id: ID, source: Source}

export function loadedNamedThings(): LoadedNamedThing[] {
  return removeDupesBy([...join([
    ...Array.from(environment().libraries.values()).map(({root}) => root), ...maybeToArray(mapMaybe(environment().rootViews.root, root => root.id))].map(namedThings)),
    ...Array.from(environment().libraries).map(([name, {root}]) => ({name, id: root, source: { source: SourceType.LibraryType } as Source}) )], ({id}) => id) }

function namedThings(id: ID): LoadedNamedThing[] { return _namedThings(new Set([id]), new Set<ID>(), []) }

function _namedThings(ids: Set<ID>, visited: Set<ID>, accumulator: LoadedNamedThing[]): LoadedNamedThing[] {
  let toProcess = Array.from(ids).filter(id => booleanFromMaybe(guidFromID(id)) && !visited.has(id))
  let children = Array.from(combineChildren(toProcess)).filter(id => !visited.has(id) && !ids.has(id))
  let namedThingsToProcess = maybeMap(toProcess, id => bindMaybe(get(id, nameField.id), ({id: nameID, source}) => mapMaybe(stringFromID(nameID), name => ({id, name, source}))))
  let newAccumulator = [...accumulator, ...namedThingsToProcess]
  return toProcess.length === 0 ? accumulator : _namedThings(new Set(children), new Set([...visited, ...ids]), newAccumulator) }

function combineChildren(ids: ID[]): Set<ID> {
  return new Set(join(maybeMap(ids, id => mapMaybe(edges(id), ({edges}) => Array.from(edges.values()))))) }