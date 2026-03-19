import SwiftUI

struct ContentView: View {
    @State private var editor = Editor.withSampleDocument()

    var body: some View {
        let ctx = ProjectionContext(entity: editor.root, path: .root(), gid: editor.gid, schema: editor.schema, editor: editor, focus: nil, ancestors: [])
        let d = project(ctx)
        DRender(d: d, editor: editor)
            .frame(minWidth: 400, minHeight: 300)
    }
}
