import { bindMaybe, fromMaybe } from "../lib/Maybe"
import { DText } from "./D"
import { _get } from "./Environment"
import { Ctor, nameField } from "./graph"
import { stringFromID } from "./ID"
import { render0 } from "./render"

export function renderNameShallow(ctor: Ctor) {
  return render0(ctor, id => new DText(fromMaybe(bindMaybe(_get(id, nameField.id), stringFromID), () => "[unnamed]"))) }