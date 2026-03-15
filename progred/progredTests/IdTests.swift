import Testing
import Foundation
@testable import progred

@Test func uuidEquality() {
    let uuid = UUID()
    #expect(Id.uuid(uuid) == Id.uuid(uuid))
    #expect(Id.uuid(UUID()) != Id.uuid(UUID()))
}

@Test func stringEquality() {
    #expect(Id.string("abc") == Id.string("abc"))
    #expect(Id.string("abc") != Id.string("def"))
}

@Test func crossTypeInequality() {
    #expect(Id.string("abc") != Id.number(123))
}

@Test func hashConsistency() {
    let uuid = UUID()
    var set: Set<Id> = [.uuid(uuid), .string("abc"), .number(123)]
    #expect(set.contains(.uuid(uuid)))
    #expect(set.contains(.string("abc")))
    #expect(set.contains(.number(123)))
}

@Test func comparable() {
    #expect(Id.number(1) < Id.number(2))
    #expect(Id.number(1) < Id.string("a"))
    #expect(Id.string("a") < Id.uuid(UUID()))
}
