import { bindMaybe, guardMaybe, mapMaybe, Maybe, maybeMap, sequenceMaybe } from "../lib/Maybe"
import { childCursor } from "./childCursor"
import { Cursor } from "./Cursor"
import { Block, D, DText, Label, Line } from "./D"
import { renderList } from "./defaultRender"
import { _get, SourceID } from "./Environment"
import * as G from "./graph"
import { descend, dispatch, Render } from "./R"
import { renderNameShallow } from "./renderNameShallow"

function dConstructorFromD(d: G.D): Maybe<(cursor: Cursor, sourceID: SourceID) => Maybe<D>> {
  return G.matchD(d,
    block => bindMaybe(bindMaybe(block.children, children => sequenceMaybe(children.map(child => () => dConstructorFromD(child)))), childConstructors => (cursor: Cursor, sourceID: SourceID): Maybe<D> =>
      mapMaybe(sequenceMaybe(childConstructors.map(childConstructor => () => childConstructor(cursor, sourceID))), children => new Block(...children)) ),
    line => bindMaybe(bindMaybe(line.children, children => sequenceMaybe(children.map(child => () => dConstructorFromD(child)))), childConstructors => (cursor: Cursor, sourceID: SourceID): Maybe<D> =>
      mapMaybe(sequenceMaybe(childConstructors.map(childConstructor => () => childConstructor(cursor, sourceID))), children => new Line(...children)) ),
    _descend => mapMaybe(_descend.field, field => (cursor: Cursor, sourceID: SourceID) => descend(cursor, sourceID.id, field.id, bindMaybe(_descend.contextRender, renderFromRender))),
    label => bindMaybe(label.field, field => bindMaybe(bindMaybe(label.child, dConstructorFromD), childConstructor => (cursor: Cursor, sourceID: SourceID): Maybe<D> => bindMaybe(childConstructor(cursor, sourceID), child => mapMaybe(childCursor(cursor, field.id), _childCursor => new Label(_childCursor, child))))),
    string => () => new DText(string) )}

export function renderFromRender(render: G.Render): Maybe<Render> {
  return G.matchRender(render,
    renderCtor => bindMaybe(bindMaybe(renderCtor.d, dConstructorFromD), dConstructor => mapMaybe(renderCtor.forCtor, ctor => (cursor: Cursor, sourceID: Maybe<SourceID>) =>
      bindMaybe(sourceID, sourceID => bindMaybe(guardMaybe(_get(sourceID.id, G.ctorField.id) === ctor.id), x => dConstructor(cursor, sourceID))) )),
    _renderList => renderList(_renderList.opening, _renderList.closing, _renderList.separator, bindMaybe(_renderList.contextRender, renderFromRender)),
    _renderNameShallow => mapMaybe(_renderNameShallow.forCtor, renderNameShallow),
    _dispatch => mapMaybe(_dispatch.renders, renders => dispatch(...maybeMap(renders, renderFromRender))) )}