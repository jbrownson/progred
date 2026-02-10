export function lowerFirst(string: string) { return string.charAt(0).toLowerCase() + string.slice(1) }
export function upperFirst(string: string) { return string.charAt(0).toUpperCase() + string.slice(1) }
export function indent(lines: string[]): string[] { return lines.map(line => "  " + line) }
export function camelCase(string: string): string {
  let _words = string.split(" ").map(s => s.trim())
  return _words.length >= 1 ? [_words[0].toLowerCase(), ..._words.slice(1).map(upperFirst)].join("") : string }
export function pascalCase(string: string): string { return string.split(" ").map(s => s.trim()).map(upperFirst).join("") }