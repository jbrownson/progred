import SwiftUI

struct ContentView: View {
    @State private var editor = Editor.withSampleDocument()
    @FocusState private var focus: Path?

    var body: some View {
        let ctx = ProjectionContext(entity: editor.root, path: .root(), gid: editor.gid, schema: editor.schema, editor: editor, focus: focus, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DView(d: d, focus: $focus)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .focusable()
        .focused($focus, equals: nil)
        .focusEffectDisabled()
        .onDeleteCommand { editor.handleDelete(path: focus) }
        .onExitCommand { focus = nil }
        .frame(minWidth: 400, minHeight: 300)
    }
}
