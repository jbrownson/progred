import { altMaybe, bindMaybe, firstMaybe, fromMaybe, guardMaybe, mapMaybe, Maybe, sequenceMaybe } from "../lib/Maybe"
import { arrayFromList } from "./arrayFromList"
import { _get } from "./Environment"
import { ctorField, D, listFromID, matchD, matchRender, nameField, Render } from "./graph"
import { ID, matchID, numberFromNID, stringFromID } from "./ID"

export type ToText = (id: ID, depth: number) => Maybe<string>

function indent(depth: number) { return Array((depth + 1) * 2 + 1).join(" ") }

function fFromD(d: D, toText: () => ToText): Maybe<(id: ID, depth: number) => Maybe<string>> {
  return matchD(d,
    block => mapMaybe(bindMaybe(block.children, children => sequenceMaybe(children.map(child => () => fFromD(child, toText)))), x =>
      (id: ID, depth: number) => mapMaybe(sequenceMaybe(x.map(x => () => x(id, depth + 1))), x => x.map(x => `${indent(depth)}${x}`).join('\n')) ),
    line => mapMaybe(bindMaybe(line.children, children => sequenceMaybe(children.map(child => () => fFromD(child, toText)))), x =>
      (id: ID, depth: number) => mapMaybe(sequenceMaybe(x.map(x => () => x(id, depth))), x => x.join('')) ),
    descend => {
      let contextToText = mapMaybe(descend.contextRender, render => toTextFromRender(render, toText))
      return bindMaybe(descend.field, field => (id: ID, depth: number): Maybe<string> =>
        bindMaybe(_get(id, field.id), newID => altMaybe(bindMaybe(contextToText, contextToText => contextToText(newID, depth)), () => toText()(newID, depth))) )},
    label => bindMaybe(label.child, child => fFromD(child, toText)),
    string => () => string )}

export function toTextFromRenders(renders: Render[]): ToText {
  let toText = (id: ID, depth: number) => firstMaybe(renders.map(render => toTextFromRender(render, () => toTextWithNumberAndString)).map(f => () => bindMaybe(f, f => f(id, depth))))
  let toTextWithNumberAndString = (id: ID, depth: number): Maybe<string> => matchID(id, guid => bindMaybe(toText, toText => toText(id, depth)), (sid, string) => string, nid => `${numberFromNID(nid)}`)
  return toTextWithNumberAndString }

function toTextFromRender(render: Render, toText: () => ToText): Maybe<ToText> {
  return matchRender(render,
    renderCtor => bindMaybe(renderCtor.forCtor, ctor => bindMaybe(bindMaybe(renderCtor.d, d => fFromD(d, toText)),
      x => (id: ID, depth: number): Maybe<string> => bindMaybe(bindMaybe(_get(id, ctorField.id), _ctor => guardMaybe(_ctor === ctor.id)), () => x(id, depth) ))),
    renderList => {
      let opening = fromMaybe(renderList.opening, () => "[")
      let closing = fromMaybe(renderList.closing, () => "]")
      let separator = fromMaybe(renderList.separator, () => ",")
      let contextToText = mapMaybe(renderList.contextRender, render => toTextFromRender(render, toText))
      return (id: ID, depth: number): Maybe<string> => {
        let array = bindMaybe(listFromID(id, id => ({id})), arrayFromList)
        let x = bindMaybe(array, array => sequenceMaybe(array.map(({id}) => () => altMaybe(
          bindMaybe(contextToText, contextToText => contextToText(id, depth)),
          () => toText()(id, depth) ))))
        return mapMaybe(x, x => `${opening}${x.join(`${separator} `)}${closing}`) }},
    renderNameShallow => (id: ID) => bindMaybe(_get(id, nameField.id), stringFromID),
    dispatch => mapMaybe(dispatch.renders, renders => {
      let x = sequenceMaybe(renders.map(render => () => toTextFromRender(render, toText)))
      return mapMaybe(x, x => (id: ID, depth: number) => firstMaybe(x.map(x => () => x(id, depth))))} ))}