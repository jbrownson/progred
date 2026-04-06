import Testing
import Foundation
@testable import progred

private func gid(_ s: Schema) -> some Gid {
    StackedGid(
        top: s.gid,
        bottom: PrimitiveGid(recordField: s.recordField, stringRecord: s.stringRecord, numberRecord: s.numberRecord))
}

@Test func primitiveRecordEdges() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(g.get(entity: .string("hi"), label: s.recordField) == s.stringRecord)
    #expect(g.get(entity: .number(42), label: s.recordField) == s.numberRecord)
}

@Test func matchesPrimitives() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(matches(.string("hi"), s.stringRecord, [:], gid: g, schema: s) == true)
    #expect(matches(.number(42), s.numberRecord, [:], gid: g, schema: s) == true)
    #expect(matches(.string("hi"), s.numberRecord, [:], gid: g, schema: s) == false)
    #expect(matches(.number(42), s.stringRecord, [:], gid: g, schema: s) == false)
}

@Test func matchesRecord() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(matches(s.nameField, s.fieldRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.nameField, s.recordRecord, [:], gid: g, schema: s) == false)
}

@Test func matchesSum() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(matches(s.stringRecord, s.typeExpressionSum, [:], gid: g, schema: s) == true)
    #expect(matches(s.nameField, s.typeExpressionSum, [:], gid: g, schema: s) == false)
}

@Test func matchesThroughApply() throws {
    let s = Schema.bootstrap()
    let g = gid(s)
    let listOfField = try #require(g.get(entity: s.fieldsField, label: s.typeExpressionField))

    let emptyList = try #require(g.get(entity: s.numberRecord, label: s.typeParametersField))
    #expect(matches(emptyList, listOfField, [:], gid: g, schema: s) == true)

    let recordFields = try #require(g.get(entity: s.recordRecord, label: s.fieldsField))
    #expect(matches(recordFields, listOfField, [:], gid: g, schema: s) == true)
}

@Test func admitsRecord() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(admits(s.stringRecord, s.stringRecord, [:], gid: g, schema: s) == true)
    #expect(admits(s.stringRecord, s.numberRecord, [:], gid: g, schema: s) == false)
}

@Test func admitsSum() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(admits(s.consRecord, s.listSum, [:], gid: g, schema: s) == true)
    #expect(admits(s.emptyRecord, s.listSum, [:], gid: g, schema: s) == true)
    #expect(admits(s.fieldRecord, s.listSum, [:], gid: g, schema: s) == false)
}

@Test func admitsThroughApply() throws {
    let s = Schema.bootstrap()
    let g = gid(s)
    let listOfField = try #require(g.get(entity: s.fieldsField, label: s.typeExpressionField))
    #expect(admits(s.consRecord, listOfField, [:], gid: g, schema: s) == true)
    #expect(admits(s.emptyRecord, listOfField, [:], gid: g, schema: s) == true)
    #expect(admits(s.recordRecord, listOfField, [:], gid: g, schema: s) == false)
}

@Test func malformedTypeReturnsNil() {
    let s = Schema.bootstrap()
    let g = gid(s)
    let bogus = Id.newUUID()
    // Type with no record edge
    #expect(matches(.string("hi"), bogus, [:], gid: g, schema: s) == nil)
    #expect(admits(s.stringRecord, bogus, [:], gid: g, schema: s) == nil)
    // Unbound type parameter
    #expect(matches(.string("hi"), s.listT, [:], gid: g, schema: s) == nil)
    #expect(admits(s.stringRecord, s.listT, [:], gid: g, schema: s) == nil)
}

// MARK: - Substitution flow

private func ctx(_ entity: Id?, _ s: Schema, substitution: Substitution = [:]) -> ProjectionContext {
    let g = gid(s)
    return ProjectionContext(entity: entity, gid: g, schema: s, editor: nil, ancestors: [],
                             substitution: substitution)
}

@Test func resolveConcreteFieldType() {
    let s = Schema.bootstrap()
    // nameField's typeExpression is stringRecord (concrete)
    let c = ctx(s.nameField, s, substitution: [:])
    #expect(c.resolveExpectedType(for: s.nameField) == s.stringRecord)
}

@Test func resolveTypeParameterThroughSubstitution() {
    let s = Schema.bootstrap()
    // headField's typeExpression is listT (a TypeParameter)
    // With T→fieldRecord in substitution, should resolve to fieldRecord
    let c = ctx(nil, s, substitution: [s.listT: s.fieldRecord])
    #expect(c.resolveExpectedType(for: s.headField) == s.fieldRecord)
}

@Test func resolveTypeParameterUnbound() {
    let s = Schema.bootstrap()
    // headField's typeExpression is listT, no substitution → nil
    let c = ctx(nil, s)
    #expect(c.resolveExpectedType(for: s.headField) == nil)
}

@Test func descendApplyExtendsSubstitution() throws {
    let s = Schema.bootstrap()
    // recordRecord has fieldsField → a cons list. fieldsField's type is List<Field> (Apply).
    // Descending should extend substitution with T→fieldRecord.
    // The first cons's headField should then resolve T→fieldRecord.
    let c = ctx(s.recordRecord, s)
    let d = c.descend(s.fieldsField)
    // The Descend's expectedType is the Apply node (List<Field>)
    guard case .descend(let outer) = d else { Issue.record("Expected descend"); return }
    let listOfField = try #require(gid(s).get(entity: s.fieldsField, label: s.typeExpressionField))
    #expect(outer.expectedType == listOfField)

    // Now project the first cons — descend into its headField should give expectedType = fieldRecord
    let fieldsList = try #require(gid(s).get(entity: s.recordRecord, label: s.fieldsField))
    let firstCons = try #require(gid(s).get(entity: fieldsList, label: s.headField))
    // Simulate being inside the list with T→fieldRecord substitution
    let consCtx = ctx(fieldsList, s, substitution: [s.listT: s.fieldRecord])
    let headD = consCtx.descend(s.headField)
    guard case .descend(let inner) = headD else { Issue.record("Expected descend"); return }
    #expect(inner.expectedType == s.fieldRecord)
    // The head value (nameField) should be rendered, and it IS a Field
    #expect(firstCons == s.nameField)
}

@Test func descendTailPreservesSubstitution() throws {
    let s = Schema.bootstrap()
    // Inside a cons with T→fieldRecord, descending into tailField (type List<T>)
    // should extend substitution with T→resolveArg(T, [T→fieldRecord]) = T→fieldRecord
    // The expectedType for tail should be the Apply node for List<T>
    let fieldsList = try #require(gid(s).get(entity: s.recordRecord, label: s.fieldsField))
    let consCtx = ctx(fieldsList, s, substitution: [s.listT: s.fieldRecord])
    let tailD = consCtx.descend(s.tailField)
    guard case .descend(let descend) = tailD else { Issue.record("Expected descend"); return }
    // tailField's typeExpression is Apply{List, T: T} — an Apply node
    let tailTypeExpr = try #require(gid(s).get(entity: s.tailField, label: s.typeExpressionField))
    #expect(descend.expectedType == tailTypeExpr)
}

@Test func admitsNeedsOuterSubstitution() throws {
    let s = Schema.bootstrap()
    var doc = MutGid()

    // Build: Sum Optional<T> { summands: [T, Empty] }
    let optT = UUID()
    doc.set(entity: optT, label: s.recordField, value: s.typeParameterRecord)
    doc.set(entity: optT, label: s.nameField, value: .string("T"))

    let emptyList = UUID()
    doc.set(entity: emptyList, label: s.recordField, value: s.emptyRecord)
    let optTCons = UUID()
    doc.set(entity: optTCons, label: s.recordField, value: s.consRecord)
    doc.set(entity: optTCons, label: s.headField, value: .uuid(optT))
    doc.set(entity: optTCons, label: s.tailField, value: .uuid(emptyList))
    let summandsList = UUID()
    doc.set(entity: summandsList, label: s.recordField, value: s.consRecord)
    doc.set(entity: summandsList, label: s.headField, value: s.emptyRecord)
    doc.set(entity: summandsList, label: s.tailField, value: .uuid(optTCons))

    let optSum = UUID()
    doc.set(entity: optSum, label: s.recordField, value: s.sumRecord)
    doc.set(entity: optSum, label: s.nameField, value: .string("Optional"))
    let tpList = UUID()
    doc.set(entity: tpList, label: s.recordField, value: s.consRecord)
    doc.set(entity: tpList, label: s.headField, value: .uuid(optT))
    let tpEmpty = UUID()
    doc.set(entity: tpEmpty, label: s.recordField, value: s.emptyRecord)
    doc.set(entity: tpList, label: s.tailField, value: .uuid(tpEmpty))
    doc.set(entity: optSum, label: s.typeParametersField, value: .uuid(tpList))
    doc.set(entity: optSum, label: s.summandsField, value: .uuid(summandsList))

    // Build: Apply Optional<Field> — fully applied, T → fieldRecord
    let optField = UUID()
    doc.set(entity: optField, label: s.recordField, value: s.applyRecord)
    doc.set(entity: optField, label: s.typeFunctionField, value: .uuid(optSum))
    doc.set(entity: optField, label: .uuid(optT), value: s.fieldRecord)

    let g = StackedGid(top: doc, bottom: gid(s))

    // Fully applied: Field is admitted (T summand resolves to Field)
    #expect(admits(s.fieldRecord, .uuid(optField), [:], gid: g, schema: s) == true)
    // Empty is also admitted (direct summand)
    #expect(admits(s.emptyRecord, .uuid(optField), [:], gid: g, schema: s) == true)
    // Record is not admitted
    #expect(admits(s.recordRecord, .uuid(optField), [:], gid: g, schema: s) == false)

    // Now build: Apply Optional<T> — argument is a TypeParameter, needs outer substitution
    let optOuterT = UUID()
    doc.set(entity: optOuterT, label: s.recordField, value: s.applyRecord)
    doc.set(entity: optOuterT, label: s.typeFunctionField, value: .uuid(optSum))
    doc.set(entity: optOuterT, label: .uuid(optT), value: .uuid(optT))  // T → T (self-referential)

    let g2 = StackedGid(top: doc, bottom: gid(s))

    // Without outer substitution: T → T is self-referential, returns nil (unresolvable)
    #expect(admits(s.fieldRecord, .uuid(optOuterT), [:], gid: g2, schema: s) == nil)
    // Empty still matches (direct summand, doesn't need T)
    #expect(admits(s.emptyRecord, .uuid(optOuterT), [:], gid: g2, schema: s) == true)

    // WITH the outer substitution: T → Field, Field is admitted
    #expect(admits(s.fieldRecord, .uuid(optOuterT), [.uuid(optT): s.fieldRecord], gid: g2, schema: s) == true)
}

@Test func admitsSumReversedOrder() throws {
    let s = Schema.bootstrap()
    var doc = MutGid()

    // Same Optional<T> as admitsNeedsOuterSubstitution, but summands reversed: [T, Empty]
    let optT = UUID()
    doc.set(entity: optT, label: s.recordField, value: s.typeParameterRecord)
    doc.set(entity: optT, label: s.nameField, value: .string("T"))

    let emptyList = UUID()
    doc.set(entity: emptyList, label: s.recordField, value: s.emptyRecord)

    // summands list: [T, Empty] — T comes first
    let emptyCons = UUID()
    doc.set(entity: emptyCons, label: s.recordField, value: s.consRecord)
    doc.set(entity: emptyCons, label: s.headField, value: s.emptyRecord)
    doc.set(entity: emptyCons, label: s.tailField, value: .uuid(emptyList))
    let tCons = UUID()
    doc.set(entity: tCons, label: s.recordField, value: s.consRecord)
    doc.set(entity: tCons, label: s.headField, value: .uuid(optT))
    doc.set(entity: tCons, label: s.tailField, value: .uuid(emptyCons))

    let optSum = UUID()
    doc.set(entity: optSum, label: s.recordField, value: s.sumRecord)
    doc.set(entity: optSum, label: s.nameField, value: .string("Optional"))
    let tpList = UUID()
    doc.set(entity: tpList, label: s.recordField, value: s.consRecord)
    doc.set(entity: tpList, label: s.headField, value: .uuid(optT))
    let tpEmpty = UUID()
    doc.set(entity: tpEmpty, label: s.recordField, value: s.emptyRecord)
    doc.set(entity: tpList, label: s.tailField, value: .uuid(tpEmpty))
    doc.set(entity: optSum, label: s.typeParametersField, value: .uuid(tpList))
    doc.set(entity: optSum, label: s.summandsField, value: .uuid(tCons))

    // Apply Optional<T> with T → T (unresolvable)
    let optOuterT = UUID()
    doc.set(entity: optOuterT, label: s.recordField, value: s.applyRecord)
    doc.set(entity: optOuterT, label: s.typeFunctionField, value: .uuid(optSum))
    doc.set(entity: optOuterT, label: .uuid(optT), value: .uuid(optT))

    let g = StackedGid(top: doc, bottom: gid(s))

    // T is first and unresolvable, but Empty should still be admitted
    #expect(admits(s.emptyRecord, .uuid(optOuterT), [:], gid: g, schema: s) == true)
    // Field should return nil (T unresolvable, Empty doesn't match)
    #expect(admits(s.fieldRecord, .uuid(optOuterT), [:], gid: g, schema: s) == nil)
}

@Test func matchesVisitedSetDistinguishesInstantiations() throws {
    let s = Schema.bootstrap()
    var doc = MutGid()

    // Record Box { content: T }
    let boxT = UUID()
    doc.set(entity: boxT, label: s.recordField, value: s.typeParameterRecord)
    doc.set(entity: boxT, label: s.nameField, value: .string("T"))

    let contentField = UUID()
    doc.set(entity: contentField, label: s.recordField, value: s.fieldRecord)
    doc.set(entity: contentField, label: s.nameField, value: .string("content"))
    doc.set(entity: contentField, label: s.typeExpressionField, value: .uuid(boxT))

    let boxRecord = UUID()
    doc.set(entity: boxRecord, label: s.recordField, value: s.recordRecord)
    doc.set(entity: boxRecord, label: s.nameField, value: .string("Box"))
    let contentList = UUID()
    doc.set(entity: contentList, label: s.recordField, value: s.consRecord)
    doc.set(entity: contentList, label: s.headField, value: .uuid(contentField))
    let contentEmpty = UUID()
    doc.set(entity: contentEmpty, label: s.recordField, value: s.emptyRecord)
    doc.set(entity: contentList, label: s.tailField, value: .uuid(contentEmpty))
    doc.set(entity: boxRecord, label: s.fieldsField, value: .uuid(contentList))
    let boxTpList = UUID()
    doc.set(entity: boxTpList, label: s.recordField, value: s.consRecord)
    doc.set(entity: boxTpList, label: s.headField, value: .uuid(boxT))
    let boxTpEmpty = UUID()
    doc.set(entity: boxTpEmpty, label: s.recordField, value: s.emptyRecord)
    doc.set(entity: boxTpList, label: s.tailField, value: .uuid(boxTpEmpty))
    doc.set(entity: boxRecord, label: s.typeParametersField, value: .uuid(boxTpList))

    // Sum Wrapper { summands: [Box<String>, Box<Number>] }
    let boxOfString = UUID()
    doc.set(entity: boxOfString, label: s.recordField, value: s.applyRecord)
    doc.set(entity: boxOfString, label: s.typeFunctionField, value: .uuid(boxRecord))
    doc.set(entity: boxOfString, label: .uuid(boxT), value: s.stringRecord)

    let boxOfNumber = UUID()
    doc.set(entity: boxOfNumber, label: s.recordField, value: s.applyRecord)
    doc.set(entity: boxOfNumber, label: s.typeFunctionField, value: .uuid(boxRecord))
    doc.set(entity: boxOfNumber, label: .uuid(boxT), value: s.numberRecord)

    let wrapperEmpty = UUID()
    doc.set(entity: wrapperEmpty, label: s.recordField, value: s.emptyRecord)
    let boxNumCons = UUID()
    doc.set(entity: boxNumCons, label: s.recordField, value: s.consRecord)
    doc.set(entity: boxNumCons, label: s.headField, value: .uuid(boxOfNumber))
    doc.set(entity: boxNumCons, label: s.tailField, value: .uuid(wrapperEmpty))
    let boxStrCons = UUID()
    doc.set(entity: boxStrCons, label: s.recordField, value: s.consRecord)
    doc.set(entity: boxStrCons, label: s.headField, value: .uuid(boxOfString))
    doc.set(entity: boxStrCons, label: s.tailField, value: .uuid(boxNumCons))

    let wrapperSum = UUID()
    doc.set(entity: wrapperSum, label: s.recordField, value: s.sumRecord)
    doc.set(entity: wrapperSum, label: s.nameField, value: .string("Wrapper"))
    let wrapperTpEmpty = UUID()
    doc.set(entity: wrapperTpEmpty, label: s.recordField, value: s.emptyRecord)
    doc.set(entity: wrapperSum, label: s.typeParametersField, value: .uuid(wrapperTpEmpty))
    doc.set(entity: wrapperSum, label: s.summandsField, value: .uuid(boxStrCons))

    // A Box value with content = nameField (a Field — neither String nor Number)
    let myBox = UUID()
    doc.set(entity: myBox, label: s.recordField, value: .uuid(boxRecord))
    doc.set(entity: myBox, label: .uuid(contentField), value: s.nameField)

    let g = StackedGid(top: doc, bottom: gid(s))

    // myBox has a Field as content — should NOT match Wrapper (neither Box<String> nor Box<Number>)
    #expect(matches(.uuid(myBox), .uuid(wrapperSum), [:], gid: g, schema: s) == false)

    // Sanity: a Box with a string content SHOULD match
    let goodBox = UUID()
    doc.set(entity: goodBox, label: s.recordField, value: .uuid(boxRecord))
    doc.set(entity: goodBox, label: .uuid(contentField), value: .string("hello"))
    let g2 = StackedGid(top: doc, bottom: gid(s))
    #expect(matches(.uuid(goodBox), .uuid(wrapperSum), [:], gid: g2, schema: s) == true)
}

@Test func schemaSelfDescribes() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(matches(s.recordRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.fieldRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.sumRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.listSum, s.sumRecord, [:], gid: g, schema: s) == true)
}
