import Testing
import Foundation
@testable import progred

@Test func recordSelfDescribes() {
    let s = Schema.bootstrap()
    #expect(s.record(of: s.recordRecord, gid: s.gid) == s.recordRecord)
}

@Test func fieldDescribedByRecord() {
    let s = Schema.bootstrap()
    #expect(s.record(of: s.fieldRecord, gid: s.gid) == s.recordRecord)
    #expect(s.record(of: s.nameField, gid: s.gid) == s.fieldRecord)
}

@Test func sumDescribedByRecord() {
    let s = Schema.bootstrap()
    #expect(s.record(of: s.sumRecord, gid: s.gid) == s.recordRecord)
    #expect(s.record(of: s.listSum, gid: s.gid) == s.sumRecord)
}

@Test func recordHasThreeFields() throws {
    let s = Schema.bootstrap()
    let f = try #require(s.fields(of: s.recordRecord, gid: s.gid))
    #expect(f.count == 3)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeParametersField)
    #expect(f[2] == s.fieldsField)
}

@Test func sumHasThreeFields() throws {
    let s = Schema.bootstrap()
    let f = try #require(s.fields(of: s.sumRecord, gid: s.gid))
    #expect(f.count == 3)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeParametersField)
    #expect(f[2] == s.summandsField)
}

@Test func fieldHasTwoFields() throws {
    let s = Schema.bootstrap()
    let f = try #require(s.fields(of: s.fieldRecord, gid: s.gid))
    #expect(f.count == 2)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeExpressionField)
}

@Test func applyHasOneField() throws {
    let s = Schema.bootstrap()
    let f = try #require(s.fields(of: s.applyRecord, gid: s.gid))
    #expect(f.count == 1)
    #expect(f[0] == s.typeFunctionField)
}

@Test func listHasOneTypeParam() throws {
    let s = Schema.bootstrap()
    let tp = try #require(s.typeParams(of: s.listSum, gid: s.gid))
    #expect(tp.count == 1)
    #expect(tp[0] == s.listT)
    #expect(s.name(of: s.listT, gid: s.gid) == "T")
}

@Test func listHasTwoSummands() throws {
    let s = Schema.bootstrap()
    let sm = try #require(s.summands(of: s.listSum, gid: s.gid))
    #expect(sm.count == 2)
    #expect(sm[0] == s.consRecord)
    #expect(sm[1] == s.emptyRecord)
}

@Test func typeExpressionSumHasFourSummands() throws {
    let s = Schema.bootstrap()
    let sm = try #require(s.summands(of: s.typeExpressionSum, gid: s.gid))
    #expect(sm.count == 4)
    #expect(sm.contains(s.recordRecord))
    #expect(sm.contains(s.sumRecord))
    #expect(sm.contains(s.applyRecord))
    #expect(sm.contains(s.typeParameterRecord))
}

@Test func namesAreCorrect() {
    let s = Schema.bootstrap()
    #expect(s.name(of: s.recordRecord, gid: s.gid) == "Record")
    #expect(s.name(of: s.fieldRecord, gid: s.gid) == "Field")
    #expect(s.name(of: s.sumRecord, gid: s.gid) == "Sum")
    #expect(s.name(of: s.listSum, gid: s.gid) == "List")
    #expect(s.name(of: s.consRecord, gid: s.gid) == "Cons")
    #expect(s.name(of: s.emptyRecord, gid: s.gid) == "Empty")
    #expect(s.name(of: s.applyRecord, gid: s.gid) == "Apply")
    #expect(s.name(of: s.stringRecord, gid: s.gid) == "String")
    #expect(s.name(of: s.numberRecord, gid: s.gid) == "Number")
}

@Test func primitivesHaveNoFields() throws {
    let s = Schema.bootstrap()
    let sf = try #require(s.fields(of: s.stringRecord, gid: s.gid))
    let nf = try #require(s.fields(of: s.numberRecord, gid: s.gid))
    #expect(sf.isEmpty)
    #expect(nf.isEmpty)
}
