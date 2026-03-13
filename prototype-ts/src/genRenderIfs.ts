import { Ctor, nonemptyListCtor } from "./graph/graph"
import { unsafeUnwrapMaybe } from "./lib/Maybe"
import { camelCase, pascalCase } from "./lib/string"

export function genRenderIfs(ctors: Ctor[]): string { return [
  [
    'import { bindMaybe, mapMaybe, Maybe } from "../lib/Maybe"',
    'import { Cursor } from "./Cursor"',
    'import { D } from "./D"',
    `import * as G from "./graph"`,
    'import { descend, Render } from "./R"',
    'import { renderOtherFields } from "./renderOtherFields"' ].join("\n"),
  ctors.filter(ctor => ctor.id !== nonemptyListCtor.id).map(ctor => [
    `export function renderIf${pascalCase(unsafeUnwrapMaybe(ctor.name))}(f: (${[...unsafeUnwrapMaybe(ctor.fields).map(field => `_${camelCase(unsafeUnwrapMaybe(field.name))}: D`), `__${camelCase(unsafeUnwrapMaybe(ctor.name))}: G.${pascalCase(unsafeUnwrapMaybe(ctor.name))}`, "cursor: Cursor"].join(', ')}) => Maybe<D>, rs: {${unsafeUnwrapMaybe(ctor.fields).map(field => `${camelCase(unsafeUnwrapMaybe(field.name))}?: Render`).join(', ')}} = {}): Render {`,
    `return (cursor, id) => bindMaybe(bindMaybe(id, G.${pascalCase(unsafeUnwrapMaybe(ctor.name))}.fromID), x => mapMaybe(f(${[...unsafeUnwrapMaybe(ctor.fields).map(field => `descend(cursor, x.id, G.${camelCase(unsafeUnwrapMaybe(field.name))}Field.id, rs.${camelCase(unsafeUnwrapMaybe(field.name))})`), "x", "cursor"].join(', ')}), d => renderOtherFields(cursor, id, d, [${unsafeUnwrapMaybe(ctor.fields).map(field => `G.${camelCase(unsafeUnwrapMaybe(field.name))}Field`)}]))) }` ].join(" ")).join("\n")].join("\n\n")}