import AppKit

protocol DView: NSView {
    func reconcile(_ d: D, editor: Editor) -> Bool
}

func reconcileChild(_ existing: NSView?, _ d: D, editor: Editor) -> NSView {
    if let node = existing as? (any DView), node.reconcile(d, editor: editor) {
        return node
    }
    return createView(d, editor: editor)
}

func reconcileList<T: AnyObject, Ts>(
    _ existing: [T],
    with ts: [Ts],
    reconcile: (T?, Ts) -> T,
    replace: (Int, T, T) -> Void,
    append: (T) -> Void,
    remove: (T) -> Void
) {
    zip(existing, ts).enumerated().forEach { (i, pair) in
        let reconciled = reconcile(pair.0, pair.1)
        if reconciled !== pair.0 { replace(i, pair.0, reconciled) }
    }
    ts.dropFirst(existing.count).forEach { append(reconcile(nil, $0)) }
    existing.dropFirst(ts.count).forEach { remove($0) }
}

func reconcileChildren(stack: NSStackView, children: [D], editor: Editor) {
    reconcileList(
        stack.arrangedSubviews,
        with: children,
        reconcile: { reconcileChild($0, $1, editor: editor) },
        replace: { i, old, new in
            stack.removeArrangedSubview(old)
            old.removeFromSuperview()
            stack.insertArrangedSubview(new, at: i)
        },
        append: { stack.addArrangedSubview($0) },
        remove: { stack.removeArrangedSubview($0); $0.removeFromSuperview() }
    )
}
