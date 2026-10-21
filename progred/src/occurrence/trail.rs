//! An occurrence path as projection builds it: each occurrence's own step,
//! linked to the trail of the occurrence it's drawn inside. Descending adds
//! one link and copies nothing, and two occurrences under one parent share
//! everything above them. What wants the whole path walks the parents.

use super::{Path, Step};
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct Trail(Option<Rc<Link>>);

struct Link {
    step: Step,
    parent: Trail,
    len: usize,
    /// How long the trail is through its last step other than a document
    /// path's, 0 without one: a jump, a computed value, or a pane's start.
    marker: usize,
    /// Whether what's here is computed: the last jump or computed step is a
    /// computed one.
    computed: bool,
}

impl Trail {
    /// The trail of a view's root, which has no steps.
    pub const ROOT: &'static Self = &Self(None);

    pub fn root() -> Self {
        Self(None)
    }

    pub fn of(path: &[Step]) -> Self {
        path.iter()
            .fold(Self::root(), |trail, step| trail.push(step.clone()))
    }

    /// This trail with `step` after it.
    pub fn push(&self, step: Step) -> Self {
        let len = self.len() + 1;
        let (marker, computed) = self
            .0
            .as_ref()
            .map_or((0, false), |link| (link.marker, link.computed));
        let (marker, computed) = match step {
            Step::Jump(_) => (len, false),
            Step::Computed => (len, true),
            Step::Pane => (len, computed),
            _ => (marker, computed),
        };
        Self(Some(Rc::new(Link {
            step,
            parent: self.clone(),
            len,
            marker,
            computed,
        })))
    }

    /// This trail with a document path's `steps` after it.
    pub fn below(&self, steps: &[gid::Step]) -> Self {
        steps
            .iter()
            .fold(self.clone(), |trail, step| trail.push(step.into()))
    }

    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |link| link.len)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// The step that reached this occurrence, none at the root.
    #[cfg(test)]
    pub fn last(&self) -> Option<&Step> {
        self.0.as_ref().map(|link| &link.step)
    }

    /// The trail of the occurrence this one is drawn inside, none at the root.
    pub fn parent(&self) -> Option<&Trail> {
        self.0.as_ref().map(|link| &link.parent)
    }

    /// The steps from this occurrence back to the root, the last one first.
    pub fn steps(&self) -> impl Iterator<Item = &Step> {
        std::iter::successors(self.0.as_deref(), |link| link.parent.0.as_deref())
            .map(|link| &link.step)
    }

    /// The whole path, from the root.
    pub fn to_vec(&self) -> Path {
        let mut path: Path = self.steps().cloned().collect();
        path.reverse();
        path
    }

    /// Where the data here is, as [`super::location`] finds it from the path:
    /// walking back only as far as the last jump or computed step.
    pub fn location(&self) -> Option<crate::location::Location<'static>> {
        let mut data = Vec::new();
        let mut from = None;
        for step in self.steps() {
            match step {
                Step::Jump(location) => {
                    from = Some(location);
                    break;
                }
                Step::Computed => return None,
                Step::Pane => {}
                step => data.push(step.data().expect("a data step")),
            }
        }
        data.reverse();
        Some(match from {
            None => crate::location::Location::root(data),
            Some(location) => {
                let mut location = (**location).clone();
                location.path.to_mut().extend(data);
                location
            }
        })
    }

    /// Whether what's here is computed, as [`super::computed`] says of the
    /// path.
    pub fn computed(&self) -> bool {
        self.0.as_ref().is_some_and(|link| link.computed)
    }

    /// Whether every step after the first `start` is a document path's, with
    /// no jump, computed value or pane start among them.
    pub fn data_from(&self, start: usize) -> bool {
        self.0.as_ref().is_none_or(|link| link.marker <= start)
    }

    /// Whether this trail's steps start with `prefix`.
    #[cfg(test)]
    pub fn starts_with<S>(&self, prefix: &[S]) -> bool
    where
        Step: PartialEq<S>,
    {
        self.len().checked_sub(prefix.len()).is_some_and(|below| {
            self.steps()
                .skip(below)
                .zip(prefix.iter().rev())
                .all(|(a, b)| a == b)
        })
    }

    /// Whether `path` starts with this trail's steps.
    pub fn leads(&self, path: &[Step]) -> bool {
        path.get(..self.len()).is_some_and(|prefix| self == prefix)
    }

    /// Whether this trail's steps after the first `from` are `other`'s after
    /// its first `other_from`.
    pub fn same_after(&self, from: usize, other: &Trail, other_from: usize) -> bool {
        let (len, other_len) = (
            self.len().checked_sub(from),
            other.len().checked_sub(other_from),
        );
        len.is_some()
            && len == other_len
            && self
                .steps()
                .zip(other.steps())
                .take(len.unwrap_or(0))
                .all(|(a, b)| a == b)
    }
}

/// Two trails are equal when their steps are. Siblings differ in their last
/// step, so comparing from the end usually answers at once, and trails that
/// share an ancestor stop comparing there.
impl PartialEq for Trail {
    fn eq(&self, other: &Self) -> bool {
        let (mut left, mut right) = (&self.0, &other.0);
        if self.len() != other.len() {
            return false;
        }
        loop {
            match (left, right) {
                (None, None) => return true,
                (Some(a), Some(b)) if Rc::ptr_eq(a, b) => return true,
                (Some(a), Some(b)) if a.step == b.step => {
                    (left, right) = (&a.parent.0, &b.parent.0);
                }
                _ => return false,
            }
        }
    }
}

impl Eq for Trail {}

impl<S> PartialEq<[S]> for Trail
where
    Step: PartialEq<S>,
{
    fn eq(&self, other: &[S]) -> bool {
        self.len() == other.len() && self.steps().zip(other.iter().rev()).all(|(a, b)| a == b)
    }
}

impl<S> PartialEq<Vec<S>> for Trail
where
    Step: PartialEq<S>,
{
    fn eq(&self, other: &Vec<S>) -> bool {
        self == other.as_slice()
    }
}

impl<S> PartialEq<&[S]> for Trail
where
    Step: PartialEq<S>,
{
    fn eq(&self, other: &&[S]) -> bool {
        self == *other
    }
}

impl From<&[Step]> for Trail {
    fn from(path: &[Step]) -> Self {
        Self::of(path)
    }
}

impl<const N: usize> From<[Step; N]> for Trail {
    fn from(path: [Step; N]) -> Self {
        Self::of(&path)
    }
}

impl From<&Path> for Trail {
    fn from(path: &Path) -> Self {
        Self::of(path)
    }
}

impl From<Path> for Trail {
    fn from(path: Path) -> Self {
        Self::of(&path)
    }
}

impl std::fmt::Debug for Trail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.to_vec()).finish()
    }
}

#[cfg(test)]
mod tests;
