import { bindMaybe, mapMaybe } from "../lib/Maybe"
import { Button, DText, Line } from "./D"
import { set } from "./Environment"
import { LoadJSON, urlField } from "./graph"
import { guidFromID } from "./ID"
import { jsonFromJSON } from "./jsonFromJSON"
import { descend, Render } from "./R"

export const renderLoadJSON: Render = (cursor, sourceID) => bindMaybe(sourceID, sourceID => mapMaybe(LoadJSON.fromID(sourceID.id), loadJSON => new Line(
  descend(cursor, sourceID.id, urlField.id),
  new DText(" "),
  new Button("Load", () =>
    bindMaybe(loadJSON.url, jsonURL => mapMaybe(guidFromID(cursor.parent), parentGUID => {
      let request = new XMLHttpRequest()
      request.open('GET', jsonURL, false)
      request.onload = () => {
        if (request.status >= 200 && request.status < 400)
          mapMaybe(jsonFromJSON(JSON.parse(request.responseText)), json => set(parentGUID, cursor.label, json.id) )}
      request.send() }))))))