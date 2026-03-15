import SwiftUI

struct ContentView: View {
    @State private var schema = Schema.bootstrap()

    var body: some View {
        ScrollView {
            EntityView(entity: schema.library, schema: schema, expanded: true)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}
