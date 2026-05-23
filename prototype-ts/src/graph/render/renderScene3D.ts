import { mapMaybe } from "../../lib/Maybe"
import { Scene3DComponent } from "../components/Scene3DComponent"
import { Scene3D } from "../graph"
import { dElement } from "./DContext"
import type { Render } from "./R"

export const renderScene3D: Render = (_edge, sourceID) =>
  mapMaybe(sourceID, ({id}) => mapMaybe(Scene3D.fromID(id), scene =>
    dElement(Scene3DComponent, {scene}, {singleLine: false, block: true})))
