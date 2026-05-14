import { bindMaybe, mapMaybe } from "../../lib/Maybe"
import { button } from "./DControls"
import { dText, line } from "./DLayout"
import { LoadJSON, urlField } from "../graph"
import { jsonFromJSON } from "../transforms/jsonFromJSON"
import { renderDocumentGuidEditor } from "./renderDocumentGuidEditor"
import { descend, Render } from "./R"

export const renderLoadJSON: Render = (edge, sourceID, edgeContext, cyclePath) => bindMaybe(sourceID, sourceID => mapMaybe(LoadJSON.fromID(sourceID.id), loadJSON => renderDocumentGuidEditor(edge, sourceID, line(
  descend(sourceID.id, urlField.id, undefined, undefined, cyclePath),
  dText(" "),
  button("Load", () =>
    bindMaybe(loadJSON.url, jsonURL => mapMaybe(edgeContext?.commit, commit => {
      let request = new XMLHttpRequest()
      request.open('GET', jsonURL, false)
      request.onload = () => {
        if (request.status >= 200 && request.status < 400) {
          try {
            mapMaybe(jsonFromJSON(JSON.parse(request.responseText)), json => commit(json.id))
          } catch {} }}
      request.send() })))))))
