import SwiftUI

struct ContentView: View {
    @State private var schema = Schema.bootstrap()

    var body: some View {
        let ctx = ProjectionContext(entity: schema.library, schema: schema, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DView(d: d)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}
