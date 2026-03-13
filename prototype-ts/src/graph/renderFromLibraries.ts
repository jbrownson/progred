import { bindMaybe, mapMaybe, maybe, maybeMap } from "../lib/Maybe"
import { Module } from "./graph"
import { Library } from "./libraries/libraries"
import { dispatch, Render } from "./R"
import { renderFromRender } from "./renderFromRender"

export function renderFromLibraries(libraries: Map<string, Library>): Render {
  return dispatch(...maybeMap(Array.from(libraries.values()), ({root}) => mapMaybe(bindMaybe(root, Module.fromID), renderFromModule))) }

export function renderFromModule(module: Module): Render {
  return dispatch(...maybe(module.renderCtors, () => [], renders => maybeMap(renders, renderFromRender))) }