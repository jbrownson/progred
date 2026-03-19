import SwiftUI

struct ContentView: View {
    @State private var editor = Editor.withSampleDocument()

    var body: some View {
        let ctx = ProjectionContext(entity: editor.root, path: .root(), gid: editor.gid, schema: editor.schema, editor: editor, focus: nil, ancestors: [])
        let d = project(ctx)
        ScrollView {
            DRender(d: d, editor: editor)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .frame(minWidth: 400, minHeight: 300)
    }
}
