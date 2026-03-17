import SwiftUI

struct ContentView: View {
    @State private var schema = Schema.bootstrap()
    @State private var currentSelection: Selection?

    var body: some View {
        let ctx = ProjectionContext(entity: schema.library, schema: schema, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DView(d: d)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .environment(\.select, { newSelection in
            currentSelection?.deselect()
            currentSelection = newSelection
        })
        .frame(minWidth: 400, minHeight: 300)
    }
}
