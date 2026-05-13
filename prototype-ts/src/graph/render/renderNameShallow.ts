import { bindMaybe, fromMaybe, mapMaybe } from "../../lib/Maybe"
import { dText } from "./Projection"
import { _get } from "../Environment"
import { Ctor, nameField } from "../graph"
import { stringFromID } from "../model/ID"
import { renderDocumentGuidEditor } from "./defaultRender"
import { render0 } from "./render"

export function renderNameShallow(ctor: Ctor) {
  let render = render0(ctor, id => dText(fromMaybe(bindMaybe(_get(id, nameField.id), stringFromID), () => "[unnamed]")))
  return (cursor, sourceID, edgeContext) => bindMaybe(render(cursor, sourceID, edgeContext), d => mapMaybe(sourceID, sourceID => renderDocumentGuidEditor(cursor, sourceID, d))) }
