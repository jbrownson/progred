import { D, matchD } from "./D"

function indent(depth: number) { return Array((depth + 1) * 2 + 1).join(" ") }

export function stringFromD(d: D, depth = 0): string {
  return matchD(d,
    block => block.children.map(d => `\n${indent(depth)}${stringFromD(d, depth + 1)}`).join(""),
    line => line.children.map(d => stringFromD(d, depth)).join(""),
    dText => dText.string,
    dList => dList.children.length === 0
      ? `${dList.opening}${dList.closing}`
      : dList.children.length === 1
        ? `${dList.opening}${stringFromD(dList.children[0], depth)}${dList.closing}`
        : `${dList.opening}${dList.children.map(child => `\n${indent(depth)}${stringFromD(child, depth + 1)}`).join(dList.separator)} ${dList.closing}`,
    descend => stringFromD(descend.child, depth),
    label => stringFromD(label.child, depth),
    button => `[${button.text}]`,
    placeholder => "[…]",
    stringEditor => stringEditor.string,
    numberEditor => `${numberEditor.number}` )}