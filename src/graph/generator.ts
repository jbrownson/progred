import * as FS from "fs"
import { genRenderIfs } from "../genRenderIfs"
import { concatMap, removeDupesBy } from "../lib/Array"
import { maybeMap, nothing, unsafeUnwrapMaybe } from "../lib/Maybe"
import { ctorsAtomicsFromCtorOrAlgebraicTypes } from "./ctorsAtomicsFromCtorOrAlgebraicTypes"
import { defaultRender } from "./defaultRender"
import { noopECallbacks } from "./ECallbacks"
import { Environment, withEnvironment } from "./Environment"
import { algebraicTypeFromCtorOrAlgebraicType, AtomicType, atomicTypeFromCtorOrAlgebraicType, Ctor, ctorFromCtorOrAlgebraicType, GUIDRootViews, Module } from "./graph"
import { GUIDMap } from "./GUIDMap"
import { libraries } from "./libraries/libraries"
import { SparseSpanningTree } from "./SparseSpanningTree"
import { typescriptFromCtorOrAlgebraicTypes } from "./typescriptFromAlgebraicTypes"

withEnvironment(new Environment(libraries, new GUIDMap, new GUIDRootViews(""), new SparseSpanningTree, {selection: nothing}, defaultRender, noopECallbacks), () => {
  let unsortedCtorOrAlgebraicTypes = concatMap(Array.from(libraries.values()), ({root}) => unsafeUnwrapMaybe(unsafeUnwrapMaybe(Module.fromID(root)).ctorOrAlgebraicTypes))
  let algebraicTypes = maybeMap(unsortedCtorOrAlgebraicTypes, algebraicTypeFromCtorOrAlgebraicType).sort((a, b) => unsafeUnwrapMaybe(a.name).localeCompare(unsafeUnwrapMaybe(b.name)))
  let {ctors, atomics} = algebraicTypes.map(algebraicType => ctorsAtomicsFromCtorOrAlgebraicTypes(unsafeUnwrapMaybe(algebraicType.ctorOrAlgebraicTypes)))
    .reduce((a, {ctors, atomics}) => ({ctors: [...a.ctors, ...ctors], atomics: [...a.atomics, ...atomics]}), {ctors: [] as Ctor[], atomics: [] as AtomicType[]})
  let _ctors = removeDupesBy([...maybeMap(unsortedCtorOrAlgebraicTypes, ctorFromCtorOrAlgebraicType), ...ctors],
    ctor => ctor.id ).sort((a, b) => unsafeUnwrapMaybe(a.name).localeCompare(unsafeUnwrapMaybe(b.name)))
  let _atomics = removeDupesBy([...maybeMap(unsortedCtorOrAlgebraicTypes, atomicTypeFromCtorOrAlgebraicType), ...atomics],
    atomic => atomic.id ).sort((a, b) => unsafeUnwrapMaybe(a.name).localeCompare(unsafeUnwrapMaybe(b.name)))
  FS.writeFileSync("src/graph/graph.ts", typescriptFromCtorOrAlgebraicTypes(algebraicTypes, _ctors, _atomics))
  FS.writeFileSync("src/graph/renderIfs.ts", genRenderIfs(_ctors)) })