import Testing
import Foundation
@testable import progred

@Test func recordSelfDescribes() {
    let s = Schema.bootstrap()
    // Record's record edge points to itself
    #expect(s.record(of: s.recordRecord) == s.recordRecord)
}

@Test func fieldDescribedByRecord() {
    let s = Schema.bootstrap()
    #expect(s.record(of: s.fieldRecord) == s.recordRecord)
    #expect(s.record(of: s.nameField) == s.fieldRecord)
}

@Test func sumDescribedByRecord() {
    let s = Schema.bootstrap()
    // The Sum meta-type is itself a Record
    #expect(s.record(of: s.sumRecord) == s.recordRecord)
    // A Sum instance is described by the Sum Record
    #expect(s.record(of: s.listSum) == s.sumRecord)
}

@Test func recordHasThreeFields() {
    let s = Schema.bootstrap()
    let f = s.fields(of: s.recordRecord)
    #expect(f.count == 3)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeParametersField)
    #expect(f[2] == s.fieldsField)
}

@Test func sumHasThreeFields() {
    let s = Schema.bootstrap()
    let f = s.fields(of: s.sumRecord)
    #expect(f.count == 3)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeParametersField)
    #expect(f[2] == s.summandsField)
}

@Test func fieldHasTwoFields() {
    let s = Schema.bootstrap()
    let f = s.fields(of: s.fieldRecord)
    #expect(f.count == 2)
    #expect(f[0] == s.nameField)
    #expect(f[1] == s.typeExpressionField)
}

@Test func applyHasOneField() {
    let s = Schema.bootstrap()
    let f = s.fields(of: s.applyRecord)
    #expect(f.count == 1)
    #expect(f[0] == s.typeFunctionField)
}

@Test func listHasOneTypeParam() {
    let s = Schema.bootstrap()
    let tp = s.typeParams(of: s.listSum)
    #expect(tp.count == 1)
    #expect(tp[0] == s.listT)
    #expect(s.name(of: s.listT) == "T")
}

@Test func listHasTwoSummands() {
    let s = Schema.bootstrap()
    let sm = s.summands(of: s.listSum)
    #expect(sm.count == 2)
    #expect(sm[0] == s.consRecord)
    #expect(sm[1] == s.emptyRecord)
}

@Test func typeExpressionSumHasFourSummands() {
    let s = Schema.bootstrap()
    let sm = s.summands(of: s.typeExpressionSum)
    #expect(sm.count == 4)
    #expect(sm.contains(s.recordRecord))
    #expect(sm.contains(s.sumRecord))
    #expect(sm.contains(s.applyRecord))
    #expect(sm.contains(s.typeParameterRecord))
}

@Test func namesAreCorrect() {
    let s = Schema.bootstrap()
    #expect(s.name(of: s.recordRecord) == "Record")
    #expect(s.name(of: s.fieldRecord) == "Field")
    #expect(s.name(of: s.sumRecord) == "Sum")
    #expect(s.name(of: s.listSum) == "List")
    #expect(s.name(of: s.consRecord) == "Cons")
    #expect(s.name(of: s.emptyRecord) == "Empty")
    #expect(s.name(of: s.applyRecord) == "Apply")
    #expect(s.name(of: s.stringRecord) == "String")
    #expect(s.name(of: s.numberRecord) == "Number")
}

@Test func primitivesHaveNoFields() {
    let s = Schema.bootstrap()
    #expect(s.fields(of: s.stringRecord).isEmpty)
    #expect(s.fields(of: s.numberRecord).isEmpty)
}
