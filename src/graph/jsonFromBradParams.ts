import { bindMaybe, filterMaybes, mapMaybe, Maybe, sequenceMaybe } from "../lib/Maybe"
import { arrayFromList } from "./arrayFromList"
import { BradParams, GUIDJSONArray, GUIDJSONNumber, GUIDJSONObject, GUIDJSONString, GUIDKeyValuePair, HouseAdImage, JSONArray, JSONObject, List, matchWeightedEntry, WeightedEntry } from "./graph"

function jsonFromImage(image: HouseAdImage): GUIDJSONObject {
  return GUIDJSONObject.new().setKeyValuePairs(filterMaybes([
    mapMaybe(image.width, width => GUIDKeyValuePair.new().setKey("width").setValue(GUIDJSONNumber.new().setJsonNumber(width))),
    mapMaybe(image.height, height => GUIDKeyValuePair.new().setKey("height").setValue(GUIDJSONNumber.new().setJsonNumber(height))),
    mapMaybe(image.extension, extension => GUIDKeyValuePair.new().setKey("extension").setValue(GUIDJSONString.new().setJsonString(extension))),
    mapMaybe(image.sha1, sha1 => GUIDKeyValuePair.new().setKey("sha1").setValue(GUIDJSONString.new().setJsonString(sha1)) )]))}

function jsonFromImages(images: HouseAdImage[]): JSONArray { return GUIDJSONArray.new().setJsonArray(images.map(jsonFromImage)) }

function jsonFromWeightedEntryData(weightedEntry: WeightedEntry): GUIDJSONObject {
  return GUIDJSONObject.new().setKeyValuePairs(
    filterMaybes(matchWeightedEntry(weightedEntry,
      networkEntry => [
        mapMaybe(networkEntry.weight, weight => GUIDKeyValuePair.new().setKey("weight").setValue(GUIDJSONNumber.new().setJsonNumber(weight))),
        mapMaybe(networkEntry.name, name => GUIDKeyValuePair.new().setKey("name").setValue(GUIDJSONString.new().setJsonString(name))) ],
      houseAdEntry => [
        mapMaybe(houseAdEntry.weight, weight => GUIDKeyValuePair.new().setKey("weight").setValue(GUIDJSONNumber.new().setJsonNumber(weight))),
        mapMaybe(houseAdEntry.name, name => GUIDKeyValuePair.new().setKey("name").setValue(GUIDJSONString.new().setJsonString(name))),
        mapMaybe(houseAdEntry.lifetimeCap, lifetimeCap => GUIDKeyValuePair.new().setKey("lifetimeCap").setValue(GUIDJSONNumber.new().setJsonNumber(lifetimeCap))),
        mapMaybe(houseAdEntry.actionURL, actionURL => GUIDKeyValuePair.new().setKey("actionUrl").setValue(GUIDJSONString.new().setJsonString(actionURL))),
        bindMaybe(houseAdEntry.images, images => GUIDKeyValuePair.new().setKey("images").setValue(jsonFromImages(images))) ])))}

function jsonFromTier(tier: WeightedEntry[]): JSONArray { return GUIDJSONArray.new().setJsonArray(tier.map(jsonFromWeightedEntryData)) }
function jsonFromTiers(tiers: List<WeightedEntry>[]): Maybe<JSONArray> {
  return mapMaybe(sequenceMaybe(tiers.map(tier => () => arrayFromList(tier))), tiers => GUIDJSONArray.new().setJsonArray(tiers.map(jsonFromTier))) }

export function jsonFromBradParams(bradParams: BradParams): JSONObject {
  return GUIDJSONObject.new().setKeyValuePairs(filterMaybes([
    mapMaybe(bradParams.fetchPeriod, fetchPeriod => GUIDKeyValuePair.new().setKey("fetchPeriod").setValue(GUIDJSONNumber.new().setJsonNumber(fetchPeriod))),
    mapMaybe(bradParams.adProbability, adProbability => GUIDKeyValuePair.new().setKey("adProbability").setValue(GUIDJSONNumber.new().setJsonNumber(adProbability))),
    mapMaybe(bradParams.timeIntervalPerAd, timeIntervalPerAd => GUIDKeyValuePair.new().setKey("timeIntervalPerAd").setValue(GUIDJSONNumber.new().setJsonNumber(timeIntervalPerAd))),
    mapMaybe(bradParams.minimumCheckpointsPerAd, minimumCheckpointsPerAd => GUIDKeyValuePair.new().setKey("minCheckpointsPerAd").setValue(GUIDJSONNumber.new().setJsonNumber(minimumCheckpointsPerAd))),
    bindMaybe(bradParams.tiers, tiers => mapMaybe(jsonFromTiers(tiers), tiers => GUIDKeyValuePair.new().setKey("tiers").setValue(tiers))) ]))}