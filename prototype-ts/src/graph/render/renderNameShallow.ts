import { bindMaybe, fromMaybe, mapMaybe } from "../../lib/Maybe"
import { dText } from "./DLayout"
import { _get } from "../Environment"
import { Ctor, nameField } from "../graph"
import { stringFromID } from "../model/ID"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { render0 } from "./render"
import type { Render } from "./R"

export function renderNameShallow(ctor: Ctor): Render {
  let render = render0(ctor, id => dText(fromMaybe(bindMaybe(_get(id, nameField.id), stringFromID), () => "[unnamed]")))
  return (cursor, sourceID, edgeContext) => bindMaybe(render(cursor, sourceID, edgeContext), d => mapMaybe(sourceID, sourceID => renderDocumentGuidEditor(cursor, sourceID, d))) }
