import { describe, expect, it } from "vitest"
import { acceptAll, acceptAllWithEmptyNeedle, defaultFilter, fuzzyFilter, prefixFilter, substringFilter } from "./filters"

const words = ["Alpha", "Beta", "alphabet", "Gamma"]

describe("filters", () => {
  it("accepts everything with full-span matches", () => {
    expect(acceptAll(words, x => x).accepted).toEqual([
      {a: "Alpha", matches: [{start: 0, length: 5}]},
      {a: "Beta", matches: [{start: 0, length: 4}]},
      {a: "alphabet", matches: [{start: 0, length: 8}]},
      {a: "Gamma", matches: [{start: 0, length: 5}]}])
  })

  it("only accepts all for an empty needle when requested", () => {
    expect(acceptAllWithEmptyNeedle<string>()(words, x => x, "").accepted.map(({a}) => a)).toEqual(words)
    expect(acceptAllWithEmptyNeedle<string>()(words, x => x, "a").accepted).toEqual([])
  })

  it("matches prefixes before non-prefixes", () => {
    expect(prefixFilter<string>()(words, x => x, "Al").accepted.map(({a}) => a)).toEqual(["Alpha"])
  })

  it("matches substrings", () => {
    expect(substringFilter<string>()(words, x => x, "ph").accepted.map(({a}) => a)).toEqual(["Alpha", "alphabet"])
  })

  it("matches fuzzy subsequences", () => {
    expect(fuzzyFilter<string>()(words, x => x, "Aa").accepted.map(({a}) => a)).toEqual(["Alpha"])
  })

  it("uses case-insensitive matching in the default filter", () => {
    expect(defaultFilter<string>()(words, x => x, "alp").accepted.map(({a}) => a).sort()).toEqual(["Alpha", "alphabet"].sort())
  })
})
