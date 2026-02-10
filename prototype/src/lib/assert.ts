export function assert(assertion: boolean, message: string = "Assertion failed") {
  if (!assertion) { debugger; throw new Error(message) }}
