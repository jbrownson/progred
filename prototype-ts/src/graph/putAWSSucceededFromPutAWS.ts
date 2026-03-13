import * as AWS from "aws-sdk"
import { bindMaybe, Maybe, nothing } from "../lib/Maybe"
import { GUIDPutAWSSucceeded, PutAWS, PutAWSSucceeded } from "./graph"

export function putAWSSucceededFromPutAWS(putAWS: PutAWS, f: (f: () => Maybe<PutAWSSucceeded>) => void): void {
  bindMaybe(putAWS.bucket, bucket => bindMaybe(putAWS.credentials, credentials => bindMaybe(putAWS.key, key => bindMaybe(credentials.accessKeyId, accessKeyID => bindMaybe(credentials.secretAccessKey, secretAccessKey => bindMaybe(putAWS.string, string => {
    new AWS.S3({credentials: new AWS.Credentials(accessKeyID, secretAccessKey)}).putObject({Bucket: bucket, Key: key, Body: string, ACL: 'public-read'}, (err, data) => f(() => err ? nothing : GUIDPutAWSSucceeded.new() )).send() }))))))}