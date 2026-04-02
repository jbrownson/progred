import AppKit

class DIndent: FlippedView, Reconcilable {
    init(child: D, editor: Editor) {
        super.init(frame: .zero)
        let childView = createView(child, editor: editor)
        addSubview(childView)
        constrain(childView, toFill: self, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
    }

    required init?(coder: NSCoder) { fatalError() }

    func reconcile(_ d: D, editor: Editor, inCycle: Bool, commit: Commit?) -> Bool {
        guard case .indent(let child) = d, let childView = subviews.first else { return false }
        let resolved = reconcileChild(childView, child, editor: editor)
        if resolved !== childView {
            childView.removeFromSuperview()
            addSubview(resolved)
            constrain(resolved, toFill: self, insets: NSEdgeInsets(top: 0, left: indentWidth, bottom: 0, right: 0))
        }
        return true
    }
}
