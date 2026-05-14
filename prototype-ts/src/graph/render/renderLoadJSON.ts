import { bindMaybe, mapMaybe } from "../../lib/Maybe"
import { button } from "./DControls"
import { dText, line } from "./DLayout"
import { LoadJSON, urlField } from "../graph"
import { jsonFromJSON } from "../transforms/jsonFromJSON"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { descend, Render } from "./R"

export const renderLoadJSON: Render = (cursor, sourceID, edgeContext) => bindMaybe(sourceID, sourceID => mapMaybe(LoadJSON.fromID(sourceID.id), loadJSON => renderDocumentGuidEditor(cursor, sourceID, line(
  descend(cursor, sourceID.id, urlField.id),
  dText(" "),
  button("Load", () =>
    bindMaybe(loadJSON.url, jsonURL => mapMaybe(edgeContext?.commit, commit => {
      let request = new XMLHttpRequest()
      request.open('GET', jsonURL, false)
      request.onload = () => {
        if (request.status >= 200 && request.status < 400)
          mapMaybe(jsonFromJSON(JSON.parse(request.responseText)), json => commit(json.id)) }
      request.send() })))))))
