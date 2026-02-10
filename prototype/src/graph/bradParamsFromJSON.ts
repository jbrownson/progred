import { altMaybe, bindMaybe, mapMaybe, Maybe, sequenceMaybe } from "../lib/Maybe"
import { GUIDBradParams, GUIDHouseAdEntry, GUIDHouseAdImage, GUIDNetworkEntry, GUIDWeightedEntry, JSON, jsonArrayFromJSON, jsonNumberFromJSON, JSONObject, jsonObjectFromJSON, jsonStringFromJSON, weightedEntryFromID } from "./graph"
import { listFromArray } from "./listFromArray"

function mapFromJSON(json: JSON): Maybe<Map<string, JSON>> { return mapMaybe(jsonObjectFromJSON(json), mapFromJSONObject) }

function mapFromJSONObject(jsonObject: JSONObject): Maybe<Map<string, JSON>> {
  return mapMaybe(mapMaybe(jsonObject.keyValuePairs, keyValuePairs => sequenceMaybe(keyValuePairs.map(keyValuePair => () =>
    bindMaybe(keyValuePair.key, key => mapMaybe(keyValuePair.value, value => [key, value] as [string, JSON])) ))), x => new Map(x)) }

function numberFromJSON(json: JSON): Maybe<number> { return bindMaybe(jsonNumberFromJSON(json), jsonNumber => jsonNumber.jsonNumber) }
function stringFromJSON(json: JSON): Maybe<string> { return bindMaybe(jsonStringFromJSON(json), jsonString => jsonString.jsonString) }

function arrayFromJSON<A>(json: JSON, f: (json: JSON) => Maybe<A>): Maybe<A[]> {
  return bindMaybe(jsonArrayFromJSON(json), jsonArray => bindMaybe(jsonArray.jsonArray, jsonArray => sequenceMaybe(jsonArray.map(json => () => f(json))))) }

function houseAdImageFromJSON(json: JSON): Maybe<GUIDHouseAdImage> {
  return mapMaybe(mapFromJSON(json), map => GUIDHouseAdImage.new()
    .setWidth(bindMaybe(map.get("width"), numberFromJSON))
    .setHeight(bindMaybe(map.get("height"), numberFromJSON))
    .setExtension(bindMaybe(map.get("extension"), stringFromJSON))
    .setSha1(bindMaybe(map.get("sha1"), stringFromJSON)) )}

function houseAdFromJSONObject(jsonObject: JSONObject): Maybe<GUIDHouseAdEntry> {
  return bindMaybe(mapFromJSONObject(jsonObject), map =>
    bindMaybe(mapMaybe(map.get("name"), stringFromJSON), name =>
      bindMaybe(mapMaybe(map.get("weight"), numberFromJSON), weight =>
        bindMaybe(mapMaybe(map.get("actionUrl"), stringFromJSON), actionUrl =>
          bindMaybe(mapMaybe(map.get("images"), images => arrayFromJSON(images, houseAdImageFromJSON)), images =>
            GUIDHouseAdEntry.new().setName(name).setWeight(weight).setLifetimeCap(mapMaybe(map.get("lifetimeCap"), numberFromJSON)).setActionURL(actionUrl).setImages(images) )))))}

function adNetworkFromJSONObject(jsonObject: JSONObject): Maybe<GUIDNetworkEntry> {
  return bindMaybe(mapFromJSONObject(jsonObject), map =>
    bindMaybe(mapMaybe(map.get("name"), stringFromJSON), name =>
      mapMaybe(mapMaybe(map.get("weight"), numberFromJSON), weight =>
        GUIDNetworkEntry.new().setName(name).setWeight(weight).setLifetimeCap(mapMaybe(map.get("lifetimeCap"), numberFromJSON)) )))}

function weightedEntryFromJSON(json: JSON): Maybe<GUIDWeightedEntry> {
  return bindMaybe(jsonObjectFromJSON(json), jsonObject => altMaybe<GUIDWeightedEntry>(houseAdFromJSONObject(jsonObject), () => adNetworkFromJSONObject(jsonObject))) }

export function bradParamsFromJSON(json: JSON): Maybe<GUIDBradParams> {
  return mapMaybe(mapFromJSON(json),
    map => GUIDBradParams.new()
      .setFetchPeriod(mapMaybe(map.get("fetchPeriod"), numberFromJSON))
      .setAdProbability(mapMaybe(map.get("adProbability"), numberFromJSON))
      .setTimeIntervalPerAd(mapMaybe(map.get("timeIntervalPerAd"), numberFromJSON))
      .setMinimumCheckpointsPerAd(mapMaybe(map.get("minCheckpointsPerAd"), numberFromJSON))
      .setTiers(bindMaybe(map.get("tiers"), item => arrayFromJSON(item, item => bindMaybe(arrayFromJSON(item, weightedEntryFromJSON), array => listFromArray(array, weightedEntryFromID))))) )}