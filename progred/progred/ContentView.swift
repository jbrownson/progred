import SwiftUI

struct ContentView: View {
    @State private var editor = Editor.withSampleDocument()
    @State private var currentSelection: Selection?
    @FocusState private var isFocused: Bool

    var body: some View {
        let ctx = ProjectionContext(entity: editor.root, gid: editor.gid, schema: editor.schema, editor: editor, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DView(d: d)
                .padding()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .environment(\.select, { newSelection in
            currentSelection?.deselect()
            currentSelection = newSelection
            isFocused = true
        })
        .focusable()
        .focused($isFocused)
        .onDeleteCommand {
            currentSelection?.handleDelete()
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}
