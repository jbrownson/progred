import SwiftUI

struct ContentView: View {
    @State private var schema = Schema.bootstrap()

    var body: some View {
        let d = project(entity: schema.library, schema: schema)
        ScrollView {
            DView(d: d)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .environment(\.schema, schema)
        .frame(minWidth: 400, minHeight: 300)
    }
}
