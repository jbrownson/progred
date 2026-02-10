import { bindArray } from "../lib/Array"
import { bindMaybe, mapMaybe, Maybe, maybe, nothing } from "../lib/Maybe"
import { Cursor } from "./Cursor"
import { D, Descend } from "./D"
import { descendFromCursor } from "./descendFromCursor"
import { _get, edges, set } from "./Environment"
import { generateGUID, guidFromID, ID } from "./ID"

// TODO: tail field on last nonempty list has no matching Descend
// this is not actually a problem since empty list has no fields except type
// which i assume wouldn't change under a structural copy??
function betwixt_(cutoff: ID, cursor: Maybe<Cursor>, result: Set<ID>): void {
  mapMaybe(cursor, cursor => {
    maybe(
      _get(cursor.parent, cursor.label),
      () => betwixt_(cutoff, cursor.parentCursor, result),
      id => bindMaybe(guidFromID(id), id => {
        if (id !== cutoff) {
          result.add(id)
          betwixt_(cutoff, cursor.parentCursor, result) }}))})}

// works in range (cutoff, _get(cursor.parent, cursor.label)]
function betwixt(cutoff: ID, cursor: Cursor): Set<ID> {
  let result = new Set<ID>()
  betwixt_(cutoff, cursor, result)
  return result }

function getIDsBetweenDescends(d: D, cutoff: ID): Set<ID> {
  return d instanceof Descend
    ? (() => {
      let s = betwixt(cutoff, d.cursor)
      Array.from(getIDsBetweenDescends(d.child, d.cursor.parent)).map(x => s.add(x))
      return s })()
    : new Set(bindArray(d.children, d => Array.from(getIDsBetweenDescends(d, cutoff)))) }

type ObjectRef = string
function makeObjectRef(n: ID) { return "new:" + n }

type SerializedID = ObjectRef | ID
type StructuralCopy = Map<ObjectRef, Map<SerializedID, SerializedID>>
type SerializedCopy = [ObjectRef, [SerializedID, SerializedID][]][]

// TODO: nameField is weird with this algorithm
function structuralCopy_(id: ID, deepCopyIDs: Set<ID>, copy: StructuralCopy, visited: Set<ID>): SerializedID {
  if (!deepCopyIDs.has(id))
    return id

  if (visited.has(id))
    return makeObjectRef(id)

  const result = makeObjectRef(id)
  visited.add(id)
  let serializedLabelChild = new Map<SerializedID, SerializedID>()
  copy.set(result, serializedLabelChild)
  const e = edges(id)
  if (e) {
    const asArray = Array.from(e.edges)
    for (const [k, v] of asArray) {
      serializedLabelChild.set(structuralCopy_(k, deepCopyIDs, copy, visited), structuralCopy_(v, deepCopyIDs, copy, visited)) }}

  return result }

function structuralCopy(id: ID, deepCopyIDs: Set<ID>): SerializedCopy {
  let result: StructuralCopy = new Map
  structuralCopy_(id, deepCopyIDs, result, new Set)
  return Array.from(result).map(x => [x[0], Array.from(x[1]) as [SerializedID, SerializedID][]]) as SerializedCopy }

export function structureForCursor(cursor: Cursor, rootDescend: Descend, viewsDescend: Maybe<Descend>): Maybe<SerializedCopy> {
  return bindMaybe(descendFromCursor(rootDescend, viewsDescend, cursor), descend =>
      bindMaybe(_get(cursor.parent, cursor.label), id =>
        structuralCopy(id, getIDsBetweenDescends(descend, cursor.parent)) ))}

function idFromStructure_(serializedID: SerializedID, structure: StructuralCopy, refMap: Map<ObjectRef, ID> = new Map): ID {
  if (typeof serializedID === "string" && serializedID.startsWith("new:")) {
    if (!refMap.has(serializedID)) {
      const thisGUID = generateGUID()
      refMap.set(serializedID, thisGUID)
      const submap = structure.get(serializedID) as Map<SerializedID, SerializedID>
      const asArray = Array.from(submap)
      for (const [k, v] of asArray) {
        set(thisGUID, idFromStructure_(k, structure, refMap), idFromStructure_(v, structure, refMap)) }}
    return refMap.get(serializedID) as ID
  } else {
    return serializedID }}

export function idFromStructure(structure: SerializedCopy): Maybe<ID> {
  const structure_: StructuralCopy = new Map(structure.map(x => [x[0], new Map<SerializedID, SerializedID>(x[1])] as [ObjectRef, Map<SerializedID, SerializedID>]))
  const ids = Array.from(structure_.keys()).map(key => idFromStructure_(key, structure_))
  return ids.length === 0
    ? nothing
    : ids[0] }