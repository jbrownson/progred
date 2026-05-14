import { bindMaybe, fromMaybe, mapMaybe } from "../../lib/Maybe"
import { _get } from "../Environment"
import { nameField } from "../graph"
import type { EdgeContext } from "../editor/EditorCommands"
import { Edge } from "../model/Edge"
import { ID, matchID, numberFromNID } from "../model/ID"
import { stringFromID } from "../model/ID"
import { block, dIdenticon, dText, line } from "./DLayout"
import { label as dLabel } from "./DEditors"
import { isSingleLine, type D } from "./DContext"
import { alwaysFail, descend } from "./R"
import { emptyCyclePath, type CyclePath } from "./CyclePath"

function renderIDLabel(id: ID): D {
  return matchID<D>(id,
    guid => fromMaybe<D>(mapMaybe(bindMaybe(_get(guid, nameField.id), stringFromID), name => dText(name)), () => dIdenticon(guid)),
    (sid, string) => dText(`"${string}"`),
    nid => dText(`${numberFromNID(nid)}`)) }

export function renderField(id: ID, label: ID, edgeContext?: EdgeContext, cyclePath: CyclePath = emptyCyclePath()): D {
  let edge: Edge = {parent: id, label}
  let childD = descend(id, label, alwaysFail, edgeContext, cyclePath)
  let labelD = dLabel(edge, line(renderIDLabel(label), dText(" →")) )
  return isSingleLine(childD)
    ? block(line(labelD, dText(" "), childD))
    : block(labelD, block(childD)) }
