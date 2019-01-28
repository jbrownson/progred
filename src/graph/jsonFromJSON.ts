import { bindMaybe, mapMaybe, Maybe, sequenceMaybe } from "../lib/Maybe"
import { GUIDJSONArray, GUIDJSONNumber, GUIDJSONObject, GUIDJSONString, GUIDKeyValuePair, JSON, matchJSON } from "./graph"

export function jsonFromJSON(json: any): Maybe<JSON> {
  return typeof json === "string"
    ? GUIDJSONString.new().setJsonString(json)
    : typeof json === "number"
      ? GUIDJSONNumber.new().setJsonNumber(json)
      : json instanceof Array
        ? mapMaybe(sequenceMaybe(json.map(json => () => jsonFromJSON(json))), array => GUIDJSONArray.new().setJsonArray(array))
        : mapMaybe(sequenceMaybe(Object.keys(json).map(key => () => mapMaybe(jsonFromJSON(json[key]), json =>
          GUIDKeyValuePair.new().setKey(key).setValue(json)))), keyValuePairs => GUIDJSONObject.new().setKeyValuePairs(keyValuePairs)) }

export function jsonToJSON(json: JSON): any {
  return matchJSON<any>(json,
    jsonString => jsonString.jsonString,
    jsonNumber => jsonNumber.jsonNumber,
    jsonArray => bindMaybe(jsonArray.jsonArray, jsonArray => jsonArray.map(jsonToJSON)),
    jsonObject => bindMaybe(jsonObject.keyValuePairs, keyValuePairs =>
      mapMaybe(sequenceMaybe(keyValuePairs.map(keyValuePair => () => bindMaybe(keyValuePair.key, key => bindMaybe(bindMaybe(keyValuePair.value, jsonToJSON),
      json => <[string, any]>[key, json])))), kvps => kvps.reduce((a, [k, v]) => { a[k] = v; return a }, <{[id: string]: any}>{})) ))}