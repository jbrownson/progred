import { bindMaybe, mapMaybe, Maybe } from "../../lib/Maybe"
import { assert } from "../../lib/assert"
import { edges, set, SourceType } from "../Environment"
import { generateGUID, GUID, ID, matchID } from "../model/ID"
import { GUIDMap } from "../model/GUIDMap"
import { load } from "../model/load"
import { save, SerializedGraph } from "../model/save"

export type CopyResult = {root: ID, remap: Map<GUID, GUID>, guidMap: GUIDMap}

function copyResultWithRoot(root: ID): CopyResult {
  return {root, remap: new Map, guidMap: new GUIDMap} }

function copyResultWithGUID(from: GUID, to: GUID): CopyResult {
  return {root: to, remap: new Map([[from, to]]), guidMap: new GUIDMap(new Map([[to, new Map]]))} }

function copyResultWithEdge(from: GUID, label: ID, to: ID): CopyResult {
  return {root: from, remap: new Map, guidMap: new GUIDMap(new Map([[from, new Map([[label, to]])]]))} }

function copyID(copyResult: CopyResult, id: ID): {id: ID, copyResult: CopyResult} {
  return matchID<{id: ID, copyResult: CopyResult}>(id, guid => {
    const guidEdges = edges(guid)
    if (guidEdges && guidEdges.source.source === SourceType.LibraryType) return {id: guid, copyResult}

    const existing = copyResult.remap.get(guid)
    if (existing) return {id: existing, copyResult}

    const copy = generateGUID()
    const result = appendCopyResult(copyResult, copyResultWithGUID(guid, copy))
    return {id: copy, copyResult: guidEdges ? Array.from(guidEdges.edges).reduce((copyResult, [label, to]) => {
      const labelCopy = copyID(copyResult, label)
      const toCopy = copyID(labelCopy.copyResult, to)
      return appendCopyResult(toCopy.copyResult, copyResultWithEdge(copy, labelCopy.id, toCopy.id)) }, result) : result} },
    sid => ({id: sid, copyResult}),
    nid => ({id: nid, copyResult})) }

export function copyResultForID(id: ID): CopyResult {
  const result = copyID(copyResultWithRoot(id), id)
  return {...result.copyResult, root: result.id} }

export function appendCopyResult(copyResult: CopyResult, other: CopyResult): CopyResult {
  const result = {root: copyResult.root, remap: new Map(copyResult.remap), guidMap: new GUIDMap(new Map(Array.from(copyResult.guidMap.map).map(([guid, edges]) => [guid, new Map(edges)])))}
  Array.from(other.remap).forEach(([from, to]) => {
    const existing = result.remap.get(from)
    assert(existing === undefined || existing === to)
    result.remap.set(from, to) })
  Array.from(other.guidMap.map).forEach(([guid, edges]) => {
    if (!result.guidMap.map.has(guid)) result.guidMap.map.set(guid, new Map)
    Array.from(edges).forEach(([label, to]) => {
      const existing = result.guidMap.map.get(guid)?.get(label)
      assert(existing === undefined || existing === to)
      result.guidMap.set(guid, label, to) })})
  return result }

export function copyResultToJSON(copyResult: CopyResult) {
  return save({root: copyResult.root, guidMap: copyResult.guidMap}) }

function remappedID(id: ID, remap: Map<GUID, GUID>): ID {
  return matchID<ID>(id, guid => remap.get(guid) || guid, sid => sid, nid => nid) }

export function idFromCopyJSON(json: SerializedGraph): Maybe<ID> {
  try {
    let {root, guidMap} = load(json)
    const remap = new Map<GUID, GUID>()
    Array.from(guidMap.map.keys()).forEach(guid => remap.set(guid, generateGUID()))
    Array.from(guidMap.map).forEach(([guid, edges]) =>
      Array.from(edges).forEach(([label, to]) =>
        set(remap.get(guid) as GUID, remappedID(label, remap), remappedID(to, remap))))
    return mapMaybe(root, root => remappedID(root, remap))
  } catch {
    return undefined }}
