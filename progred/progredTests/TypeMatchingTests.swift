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

@Test func schemaSelfDescribes() {
    let s = Schema.bootstrap()
    let g = gid(s)
    #expect(matches(s.recordRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.fieldRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.sumRecord, s.recordRecord, [:], gid: g, schema: s) == true)
    #expect(matches(s.listSum, s.sumRecord, [:], gid: g, schema: s) == true)
}
