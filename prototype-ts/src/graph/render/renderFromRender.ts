import { bindMaybe, guardMaybe, mapMaybe, Maybe, maybeMap, sequenceMaybe } from "../../lib/Maybe"
import type { D } from "./DContext"
import { block as dBlock, dText, line as dLine } from "./DLayout"
import { label as dLabel } from "./DEditors"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { renderList } from "./renderList"
import { _get, SourceID } from "../Environment"
import * as G from "../graph"
import { Edge } from "../model/Edge"
import { descend, dispatch, Render } from "./R"
import { renderNameShallow } from "./renderNameShallow"
import type { CyclePath } from "./CyclePath"

function dConstructorFromD(d: G.D): Maybe<(edge: Edge, sourceID: SourceID, cyclePath?: CyclePath) => Maybe<D>> {
  return G.matchD(d,
    block => bindMaybe(bindMaybe(block.children, children => sequenceMaybe(children.map(child => () => dConstructorFromD(child)))), childConstructors => (edge: Edge, sourceID: SourceID, cyclePath?: CyclePath): Maybe<D> =>
      mapMaybe(sequenceMaybe(childConstructors.map(childConstructor => () => childConstructor(edge, sourceID, cyclePath))), children => dBlock(...children)) ),
    line => bindMaybe(bindMaybe(line.children, children => sequenceMaybe(children.map(child => () => dConstructorFromD(child)))), childConstructors => (edge: Edge, sourceID: SourceID, cyclePath?: CyclePath): Maybe<D> =>
      mapMaybe(sequenceMaybe(childConstructors.map(childConstructor => () => childConstructor(edge, sourceID, cyclePath))), children => dLine(...children)) ),
    _descend => mapMaybe(_descend.field, field => (edge: Edge, sourceID: SourceID, cyclePath?: CyclePath) => descend(sourceID.id, field.id, bindMaybe(_descend.contextRender, renderFromRender), undefined, cyclePath)),
    label => bindMaybe(label.field, field => bindMaybe(bindMaybe(label.child, dConstructorFromD), childConstructor => (edge: Edge, sourceID: SourceID, cyclePath?: CyclePath): Maybe<D> => mapMaybe(childConstructor({parent: sourceID.id, label: field.id}, sourceID, cyclePath), child => dLabel({parent: sourceID.id, label: field.id}, child)))),
    string => () => dText(string) )}

export function renderFromRender(render: G.Render): Maybe<Render> {
  return G.matchRender(render,
    renderCtor => bindMaybe(bindMaybe(renderCtor.d, dConstructorFromD), dConstructor => mapMaybe(renderCtor.forCtor, ctor => (edge: Edge, sourceID: Maybe<SourceID>, edgeContext, cyclePath) =>
      bindMaybe(sourceID, sourceID => bindMaybe(guardMaybe(_get(sourceID.id, G.ctorField.id) === ctor.id), x => mapMaybe(dConstructor(edge, sourceID, cyclePath), d => renderDocumentGuidEditor(edge, sourceID, d)))) )),
    _renderList => renderList(_renderList.opening, _renderList.closing, _renderList.separator, bindMaybe(_renderList.contextRender, renderFromRender)),
    _renderNameShallow => mapMaybe(_renderNameShallow.forCtor, renderNameShallow),
    _dispatch => mapMaybe(_dispatch.renders, renders => dispatch(...maybeMap(renders, renderFromRender))) )}
