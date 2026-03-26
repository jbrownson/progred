import AppKit

class DIndent: FlippedView, Reconcilable {
    var parentReadOnly: Bool

    init(child: D, editor: Editor, parentReadOnly: Bool) {
        self.parentReadOnly = parentReadOnly
        super.init(frame: .zero)
        let childView = createView(child, editor: editor, parentReadOnly: parentReadOnly)
        addSubview(childView)
        constrain(childView, toFill: self, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, parentReadOnly: Bool, editPath: Path?) -> Bool {
        guard case .indent(let child) = d, let childView = subviews.first else { return false }
        let resolved = reconcileChild(childView, child, editor: editor, parentReadOnly: parentReadOnly)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        }
        return true
    }
}
