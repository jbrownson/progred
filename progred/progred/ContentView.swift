import SwiftUI

struct ContentView: View {
    @State private var editor = Editor.withSampleDocument()
    @FocusState private var focus: Path?

    var body: some View {
        let ctx = ProjectionContext(entity: editor.root, path: .root(), gid: editor.gid, schema: editor.schema, editor: editor, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DView(d: d, focus: $focus)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .onDeleteCommand { editor.handleDelete(path: focus) }
        .frame(minWidth: 400, minHeight: 300)
    }
}
