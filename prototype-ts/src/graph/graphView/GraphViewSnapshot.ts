import { bindMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { _get } from "../Environment"
import { ctorField, fieldCtor, GUIDRootViews, nameField } from "../graph"
import { _Selection } from "../editor/Selection"
import { GUIDMap } from "../model/GUIDMap"
import { GUID, guidFromID, ID, matchID, numberFromNID, stringFromID } from "../model/ID"

export type GraphSelection = {kind: "node", id: ID} | {kind: "edge", source: GUID, label: ID}
export type GraphLabelPart = {name: Maybe<string>, guid: Maybe<GUID>}
export type GraphLabel = {parts: GraphLabelPart[]}
export type GraphNode = {id: ID, label: GraphLabel, root: boolean}
export type GraphEdge = {source: GUID, label: ID, target: ID, labelText: GraphLabel}
export type GraphSelectionStrength = "primary" | "secondary"
export type SelectedGraphNode = {id: ID, strength: GraphSelectionStrength}
export type SelectedGraphEdge = {source: GUID, label: ID, strength: GraphSelectionStrength}
type SelectedGraphEdgeID = {source: GUID, label: ID}
export type GraphViewSnapshot = {
  nodes: GraphNode[],
  edges: GraphEdge[],
  selectedNode: Maybe<SelectedGraphNode>,
  selectedEdge: Maybe<SelectedGraphEdge> }

function guidLabelPart(guid: GUID): GraphLabelPart {
  return {name: bindMaybe(_get(guid, nameField.id), stringFromID), guid} }

function idLabelPart(id: ID): GraphLabelPart {
  return matchID<GraphLabelPart>(id,
    guidLabelPart,
    (_sid, string) => ({name: `"${string}"`, guid: nothing}),
    nid => ({name: String(numberFromNID(nid)), guid: nothing})) }

function guidDisplayLabel(guid: GUID, omitFieldCtor = false): GraphLabel {
  return maybe(_get(guid, ctorField.id),
    () => ({parts: [guidLabelPart(guid)]}),
    ctor => ({parts: omitFieldCtor && ctor === fieldCtor.id ? [guidLabelPart(guid)] : [idLabelPart(ctor), guidLabelPart(guid)]})) }

function idDisplayLabel(id: ID, omitFieldCtor = false): GraphLabel {
  return matchID<GraphLabel>(id,
    guid => guidDisplayLabel(guid, omitFieldCtor),
    (_sid, string) => ({parts: [{name: `"${string}"`, guid: nothing}]}),
    nid => ({parts: [{name: String(numberFromNID(nid)), guid: nothing}]})) }

function selectedNodeFromGraphSelection(graphSelection: Maybe<GraphSelection>): Maybe<ID> {
  return bindMaybe(graphSelection, graphSelection => {
    switch (graphSelection.kind) {
      case "node":
        return graphSelection.id
      case "edge":
        return nothing }})}

function selectedEdgeFromCursor(selection: Maybe<_Selection>): Maybe<SelectedGraphEdgeID> {
  return bindMaybe(selection, selection =>
    mapMaybe(guidFromID(selection.cursor.parent), source => ({source, label: selection.cursor.label}))) }

function selectedEdgeFromGraphSelection(graphSelection: Maybe<GraphSelection>): Maybe<SelectedGraphEdgeID> {
  return bindMaybe(graphSelection, graphSelection => graphSelection.kind === "edge" ? {source: graphSelection.source, label: graphSelection.label} : nothing) }

export function buildGraphViewSnapshot(guidMap: GUIDMap, rootViews: GUIDRootViews, selection: Maybe<_Selection>, graphSelection: Maybe<GraphSelection>): GraphViewSnapshot {
  let ids = new Set<ID>()
  let rootID = mapMaybe(rootViews.root, root => root.id)
  mapMaybe(rootID, id => ids.add(id))

  let edges: GraphEdge[] = []
  for (let [source, sourceEdges] of guidMap.map) {
    if (source === rootViews.id) continue
    ids.add(source)
    for (let [label, target] of sourceEdges) {
      ids.add(target)
      edges.push({source, label, target, labelText: idDisplayLabel(label, true)}) }}

  let cursorSelectedNode = bindMaybe(selection, selection => _get(selection.cursor.parent, selection.cursor.label))
  let graphSelectedNode = selectedNodeFromGraphSelection(graphSelection)
  let cursorSelectedEdge = selectedEdgeFromCursor(selection)
  let graphSelectedEdge = selectedEdgeFromGraphSelection(graphSelection)
  return {
    nodes: Array.from(ids).map(id => ({id, label: idDisplayLabel(id), root: id === rootID})),
    edges,
    selectedNode: fromMaybe(
      mapMaybe(graphSelectedNode, id => ({id, strength: "primary" as GraphSelectionStrength})),
      () => mapMaybe(cursorSelectedNode, id => ({id, strength: "secondary" as GraphSelectionStrength}))),
    selectedEdge: fromMaybe(
      mapMaybe(graphSelectedEdge, edge => ({...edge, strength: "primary" as GraphSelectionStrength})),
      () => mapMaybe(cursorSelectedEdge, edge => ({...edge, strength: "secondary" as GraphSelectionStrength}))) } }
