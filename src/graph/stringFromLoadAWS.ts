import * as AWS from "aws-sdk"
import { bindMaybe, Maybe, nothing } from "../lib/Maybe"
import { HasSID, LoadAWS } from "./graph"
import { sidFromString } from "./ID"

export function stringFromLoadAWS(loadAWS: LoadAWS, f: (f: () => Maybe<HasSID>) => void): void {
  bindMaybe(loadAWS.bucket, bucket => bindMaybe(loadAWS.credentials, credentials => bindMaybe(loadAWS.key, key => bindMaybe(credentials.accessKeyId, accessKeyID => bindMaybe(credentials.secretAccessKey, secretAccessKey => {
    new AWS.S3({credentials: new AWS.Credentials(accessKeyID, secretAccessKey)}).getObject({Bucket: bucket, Key: key}, (err, data) => f(() => {
      return data.Body instanceof Buffer
        ? new HasSID(sidFromString(data.Body.toString()))
        : nothing })).send() })))))}