import {identity} from "../lib/identity"
import {maybe} from "../lib/Maybe"

// TODO we will eventually have to specify more style information here, font/size/etc

let getTextWidthCache = new Map<string, number>()

export function getTextWidth(s: string) {
  return maybe(getTextWidthCache.get(s), () => {
    const e = document.createElement('div')
    e.style.position = 'absolute'
    e.style.visibility = 'hidden'
    e.style.whiteSpace = 'pre'
    document.body.appendChild(e)
    e.innerHTML = s
    const width = e.offsetWidth; // if I don't put this semicolon here Typescript 2.1.4 thinks I'm trying to call a number
    (e.parentElement as HTMLElement).removeChild(e)
    getTextWidthCache.set(s, width)
    return width }, identity )}