import { bindMaybe, booleanFromMaybe, fromMaybe, mapMaybe, Maybe, maybe, nothing } from "../../lib/Maybe"
import { buildEntries } from "../editor/buildEntries"
import { type D } from "./DContext"
import { dIdenticon, dText, line } from "./DLayout"
import { collapsible, collapseToggle } from "./DControls"
import { guidEditor, numberEditor, placeholderEditor, stringEditor, supportsUnderselection } from "./DEditors"
import { _get, edges, Source, SourceID, SourceType } from "../Environment"
import { Ctor, ctorField, nameField } from "../graph"
import { Edge } from "../model/Edge"
import { GUID, matchID, SID } from "../model/ID"
import { stringFromID } from "../model/ID"
import { descend, Render } from "./R"
import type { EdgeContext } from "../editor/EditorCommands"
import { edgeContextForEdge } from "../editor/edgeContext"
import { editorCommands } from "./renderDocumentGuidEditor"
import { renderField } from "./renderField"
import { listCreationEditorCommands, renderList } from "./renderList"
import { emptyCyclePath, stepCyclePath, type CyclePath } from "./CyclePath"

export function tryFirst(render: Render, defaultRender: (edge: Edge, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext, cyclePath?: CyclePath) => D): (edge: Edge, id: Maybe<SourceID>, edgeContext?: EdgeContext, cyclePath?: CyclePath) => D {
  return (edge, sourceID, edgeContext, cyclePath = emptyCyclePath()) => fromMaybe(render(edge, sourceID, edgeContext, cyclePath), () => defaultRender(edge, sourceID, edgeContext, cyclePath)) }

function _defaultRender(edge: Edge, sourceID: Maybe<SourceID>, edgeContext?: EdgeContext, cyclePath: CyclePath = emptyCyclePath()): D {
  edgeContext = fromMaybe(edgeContext, () => edgeContextForEdge(edge))
  return maybe(sourceID, () => renderNothing(edgeContext), sourceID => matchID(sourceID.id,
    guid => renderGUID(edge, guid, sourceID.source, cyclePath),
    (sid, string) => renderString(edge, sid, string, sourceID.source),
    number => renderNumber(edge, number, sourceID.source) ))}

export const defaultRender = tryFirst(renderList(), _defaultRender)

function renderNothing(edgeContext: EdgeContext): D {
  let entries = buildEntries(edgeContext.expectedType, id => mapMaybe(edgeContext.commit, commit => commit(id())))
  return placeholderEditor(fromMaybe(edgeContext.fieldName, () => "[unnamed]"), entries, nothing, listCreationEditorCommands()) }

function renderGUID(edge: Edge, guid: GUID, source: Source, cyclePath: CyclePath): D {
  let ctor = bindMaybe(_get(guid, ctorField.id), Ctor.fromID)
  let ctorFields = fromMaybe(bindMaybe(ctor, ctor => ctor.fields), () => [])
  let guidEdges = edges(guid)
  let writable = maybe(guidEdges, () => sourceIsWritable(source), ({source}) => sourceIsWritable(source))
  let extraLabels = maybe(guidEdges, () => [], ({edges}) => Array.from(edges.keys()).filter(edge => ctorFields.find(field => field.id === edge) === undefined))
  let hasName = booleanFromMaybe(ctorFields.find(field => field.id === nameField.id)) || booleanFromMaybe(extraLabels.find(label => label === nameField.id))
  let labels = [
    ...ctorFields.filter(field => field.id !== nameField.id && field.id !== ctorField.id).map(field => field.id),
    ...extraLabels.filter(label => label !== nameField.id && label !== ctorField.id) ]
  let cycleStep = stepCyclePath(cyclePath, guid)
  let defaultCollapsed = cycleStep.hasCycle
  let render = (collapsed: boolean, setCollapsed: (collapsed: boolean) => void) => {
    let nameDs = hasName
      ? collapsed
        ? maybe(bindMaybe(_get(guid, nameField.id), stringFromID), () => [], name => [dText(" "), dText(name)])
        : [dText(" "), descend(guid, nameField.id, undefined, undefined, cyclePath)]
      : []
    let fieldDs = collapsed ? [] : labels.map(label => renderField(guid, label, undefined, cyclePath))
    const d = line(
      guidEditor(edge, guid, fromMaybe<D>(bindMaybe(ctor, ctor => mapMaybe(ctor.name, name => dText(name))), () => dIdenticon(guid)),
        true,
        editorCommands(guid)),
      ...nameDs,
      ...(hasName || labels.length > 0 ? [collapseToggle(collapsed, () => setCollapsed(!collapsed))] : []),
      ...fieldDs )
    return writable ? supportsUnderselection(edge, guid, d, missingLabel => renderField(guid, missingLabel, undefined, cyclePath)) : d }
  return defaultCollapsed || hasName || labels.length > 0 ? collapsible(defaultCollapsed, defaultCollapsed || labels.length === 0, render) : render(false, () => {}) }

function sourceIsWritable(source: Source) { return source.source === SourceType.DocumentType }

export function renderNumber(edge: Edge, number: number, source: Source): D {
  return numberEditor(number, sourceIsWritable(source), editorCommands(number)) }
export function renderString(edge: Edge, sid: SID, string: string, source: Source): D {
  return stringEditor(string, sourceIsWritable(source), editorCommands(sid)) }
