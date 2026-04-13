import AppKit

enum NavigationDirection {
    case tab
    case backtab
    // Arrow directions coming later (up/down/left/right over structural tree).
}

/// Views that participate in our keyboard navigation by declaring what the user
/// can reach them via. Default implementation lands nothing — conforming views
/// opt in by overriding `isTabTarget` etc.
protocol FocusTarget: NSView {
    var isTabTarget: Bool { get }
}

extension FocusTarget {
    var isTabTarget: Bool { false }
}

extension NSView {
    /// Next view in document order (preorder traversal of the tree).
    /// First child if any; else the next sibling; else the ancestor's next sibling; else nil.
    fileprivate func nextInDocumentOrder() -> NSView? {
        if let first = subviews.first { return first }
        var view: NSView = self
        while let parent = view.superview {
            if let sibling = parent.sibling(after: view) { return sibling }
            view = parent
        }
        return nil
    }

    /// Previous view in document order.
    /// Previous sibling's deepest-last descendant if there's a previous sibling;
    /// else the parent.
    fileprivate func prevInDocumentOrder() -> NSView? {
        guard let parent = superview else { return nil }
        if let prevSib = parent.sibling(before: self) {
            return prevSib.deepestLastDescendant()
        }
        return parent
    }

    private func sibling(after view: NSView) -> NSView? {
        guard let i = subviews.firstIndex(of: view), i + 1 < subviews.count else { return nil }
        return subviews[i + 1]
    }

    private func sibling(before view: NSView) -> NSView? {
        guard let i = subviews.firstIndex(of: view), i > 0 else { return nil }
        return subviews[i - 1]
    }

    private func deepestLastDescendant() -> NSView {
        var view = self
        while let last = view.subviews.last { view = last }
        return view
    }

    /// Walk the tree to find the next FocusTarget matching the given direction.
    /// Returns nil if no target is found (e.g. end of document).
    func nextFocusTarget(_ direction: NavigationDirection) -> NSView? {
        let step: (NSView) -> NSView? = { view in
            switch direction {
            case .tab: return view.nextInDocumentOrder()
            case .backtab: return view.prevInDocumentOrder()
            }
        }
        var view = step(self)
        while let v = view {
            if let target = v as? FocusTarget, target.isTabTarget { return v }
            view = step(v)
        }
        return nil
    }
}
