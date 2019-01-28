import { nothing } from "../lib/Maybe"

let ignoreFocusEventsCount = 0

export function focus(htmlElement: HTMLElement) {
  if (document.activeElement !== htmlElement) {
    ++ignoreFocusEventsCount
    htmlElement.focus()
    --ignoreFocusEventsCount }}

export function blur(htmlElement: HTMLElement) {
  if (document.activeElement === htmlElement) {
    ++ignoreFocusEventsCount
    htmlElement.blur()
    --ignoreFocusEventsCount }}

export function handleFocusEvent<A>(f: () => A) {
  return ignoreFocusEventsCount === 0 ? f() : nothing }