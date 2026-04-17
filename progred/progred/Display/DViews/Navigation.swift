import AppKit

enum NavigationDirection {
    case tab, backtab   // Document-order, skipping to next unfilled slot.
    case left, right    // Parent / first-child over StructuralNodes.
    case up, down       // Previous / next sibling StructuralNode.
}

protocol FocusTarget: NSView {
    var isTabTarget: Bool { get }
}

extension FocusTarget {
    var isTabTarget: Bool { false }
}

protocol StructuralNode: NSView {
    var isStructural: Bool { get }
}

extension StructuralNode {
    var isStructural: Bool { true }
}

extension NSView {
    fileprivate func nextInDocumentOrder() -> NSView? {
        if let first = subviews.first { return first }
        var view: NSView = self
        while let parent = view.superview {
            if let sibling = parent.sibling(after: view) { return sibling }
            view = parent
        }
        return nil
    }

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

    func nextFocusTarget(_ direction: NavigationDirection) -> NSView? {
        switch direction {
        case .tab, .backtab:
            let step: (NSView) -> NSView? = { view in
                direction == .tab ? view.nextInDocumentOrder() : view.prevInDocumentOrder()
            }
            var view = step(self)
            while let v = view {
                if let target = v as? FocusTarget, target.isTabTarget { return v }
                view = step(v)
            }
            return nil
        case .up, .down, .left, .right:
            guard let current = enclosingStructural() else { return nil }
            switch direction {
            case .left:
                return current.superview?.enclosingStructural()
            case .right:
                return current.firstDescendantStructural()
            case .up, .down:
                guard let parent = current.superview?.enclosingStructural() else { return nil }
                let siblings = parent.childStructurals()
                guard let idx = siblings.firstIndex(where: { $0 === current }) else { return nil }
                let target = direction == .up ? idx - 1 : idx + 1
                return siblings.indices.contains(target) ? siblings[target] : nil
            default: return nil
            }
        }
    }

    func enclosingStructural() -> StructuralNode? {
        var view: NSView? = self
        while let v = view {
            if let node = v as? StructuralNode, node.isStructural { return node }
            view = v.superview
        }
        return nil
    }

    func firstDescendantStructural() -> StructuralNode? {
        for sub in subviews {
            if let node = sub as? StructuralNode, node.isStructural { return node }
            if let found = sub.firstDescendantStructural() { return found }
        }
        return nil
    }

    fileprivate func childStructurals() -> [StructuralNode] {
        var result: [StructuralNode] = []
        for sub in subviews {
            if let node = sub as? StructuralNode, node.isStructural {
                result.append(node)
            } else {
                result.append(contentsOf: sub.childStructurals())
            }
        }
        return result
    }
}
