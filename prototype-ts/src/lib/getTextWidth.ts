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
    e.textContent = s
    const width = e.offsetWidth
    e.remove()
    getTextWidthCache.set(s, width)
    return width }, identity )}
