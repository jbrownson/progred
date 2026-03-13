import { bindMaybe } from "../lib/Maybe"
import { AppPlatform, GUIDAWSCredentials, GUIDPutAWS } from "./graph"

export function putAWSFromAppPlatform(appPlatform: AppPlatform) {
  return GUIDPutAWS.new()
    .setBucket("brainiumads")
    .setCredentials(GUIDAWSCredentials.new().setAccessKeyId("AKIAI76HHZJKZ5K7BJ3Q"))
    .setKey(`${bindMaybe(appPlatform.platform, platform => platform.name)}/${bindMaybe(appPlatform.app, app => app.name)}/bradparams.json`) }