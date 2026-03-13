import { altMaybe, bindMaybe, Maybe, nothing } from "../lib/Maybe"
import { D, Descend } from "./D"
import { _get } from "./Environment"

function findTabStopDownChildren(d: D, n: number): Maybe<Descend> {
  function f(children: D[]): Maybe<Descend> {
    return children.length > 0
      ? altMaybe(findTabStopDown(children[0], n), () => f(children.slice(1)))
      : nothing
  }
  return f(n > 0 ? d.children : d.children.reverse()) }

function findTabStopDown(d: D, n: number): Maybe<Descend> {
  return d instanceof Descend && _get(d.cursor.parent, d.cursor.label) === nothing ? d : findTabStopDownChildren(d, n) }

function getSiblingD(d: D, n: number): Maybe<D> {
  return bindMaybe(d.parent, parent => bindMaybe(parent.children.findIndex(child => child === d), index => parent.children[index + n])) }

function nextUp(d: D, n: number): Maybe<D> {
  return bindMaybe(d.parent, dParent => altMaybe(getSiblingD(dParent, n), () => nextUp(dParent, n))) }

function findTabStopUp(d: D, n: number): Maybe<Descend> {
  return altMaybe(
    bindMaybe(getSiblingD(d, n), siblingD => findTabStop(siblingD, n)),
    () => bindMaybe(nextUp(d, n), nextUp => findTabStop(nextUp, n)) )}

export function findTabStop(d: D, n: number): Maybe<Descend> {
  return altMaybe(findTabStopDown(d, n), () => findTabStopUp(d, n)) }

export function findNextTabStop(d: D, n: number = 1): Maybe<Descend> {
  return altMaybe(findTabStopDownChildren(d, n), () => findTabStopUp(d, n)) }