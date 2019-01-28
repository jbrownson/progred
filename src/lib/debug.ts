export function logS<A>(s: string, a: A): A {
  console.log(s, a)
  return a }

export function log<A>(a: A): A {
  console.log(a)
  return a }

export function debug<A>(a: A): A {
  debugger
  return a }