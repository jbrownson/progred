//! Compositional navigation descriptions and their frame-local resolution.
use super::{Direction, Stop};
use gid::Step;
use std::{collections::HashMap, rc::Rc};

type Path = Rc<[Step]>;

/// A component's explicit interface in one direction. Exits are deliberate
/// connection points, not a search for stops that happen to lack a link.
#[derive(Clone, Default)]
pub struct Boundary {
    pub entry: Option<Path>,
    pub exits: Vec<Path>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Port {
    pub path: Path,
    pub direction: Direction,
}

#[derive(Clone)]
pub struct Link {
    pub from: Port,
    pub to: Path,
}

impl Link {
    pub fn new(from: Path, direction: Direction, to: Path) -> Self {
        Self {
            from: Port {
                path: from,
                direction,
            },
            to,
        }
    }
}

/// A description, not a partially constructed routing table. Combining parts
/// concatenates declarations; no child connection can be overwritten.
#[derive(Clone, Default)]
pub struct Navigation {
    stops: Vec<Stop>,
    links: Vec<Link>,
    boundaries: [Boundary; 4],
}

impl Navigation {
    pub fn new(stops: Vec<Stop>, links: Vec<Link>, boundaries: [Boundary; 4]) -> Self {
        Self {
            stops,
            links,
            boundaries,
        }
    }

    pub fn stop(stop: Stop) -> Self {
        let boundaries = std::array::from_fn(|_| Boundary {
            entry: Some(stop.path.clone()),
            exits: vec![stop.path.clone()],
        });
        Self::new(vec![stop], vec![], boundaries)
    }

    pub fn boundary(&self, direction: Direction) -> &Boundary {
        &self.boundaries[direction.index()]
    }

    /// Overlay independent contributions, without introducing links.
    pub fn join(children: Vec<Self>) -> Self {
        let boundaries = std::array::from_fn(|index| {
            let direction = Direction::ALL[index];
            let entry = if matches!(direction, Direction::Left | Direction::Up) {
                children
                    .iter()
                    .rev()
                    .find_map(|n| n.boundary(direction).entry.clone())
            } else {
                children
                    .iter()
                    .find_map(|n| n.boundary(direction).entry.clone())
            };
            Boundary {
                entry,
                exits: children
                    .iter()
                    .flat_map(|n| n.boundary(direction).exits.clone())
                    .collect(),
            }
        });
        let mut result = Self::new(vec![], vec![], boundaries);
        for child in children {
            result.stops.extend(child.stops);
            result.links.extend(child.links);
        }
        result
    }

    pub(super) fn sequence(children: Vec<Self>, forward: Direction) -> Self {
        let children: Vec<_> = children
            .into_iter()
            .filter(|n| !n.stops.is_empty())
            .collect();
        let backward = forward.opposite();
        let mut connections = Vec::new();
        for pair in children.windows(2) {
            let [a, b] = pair else { unreachable!() };
            for (from, to, direction) in [(a, b, forward), (b, a, backward)] {
                if let Some(entry) = &to.boundary(direction).entry {
                    connections.extend(
                        from.boundary(direction)
                            .exits
                            .iter()
                            .map(|exit| Link::new(exit.clone(), direction, entry.clone())),
                    );
                }
            }
        }
        let start = children
            .first()
            .map(|n| n.boundary(backward).exits.clone())
            .unwrap_or_default();
        let end = children
            .last()
            .map(|n| n.boundary(forward).exits.clone())
            .unwrap_or_default();
        let mut result = Self::join(children);
        result.boundaries[backward.index()].exits = start;
        result.boundaries[forward.index()].exits = end;
        result.links.extend(connections);
        result
    }

    pub fn group(stop: Stop, mut content: Self) -> Self {
        let path = stop.path.clone();
        if !content.stops.iter().any(|s| s.path == path) {
            content.stops.push(stop);
            for direction in [Direction::Right, Direction::Down] {
                if let Some(entry) = content.boundary(direction).entry.clone() {
                    content
                        .links
                        .push(Link::new(path.clone(), direction, entry));
                } else {
                    content.boundaries[direction.index()]
                        .exits
                        .push(path.clone());
                }
                let backward = direction.opposite();
                let returns = std::mem::replace(
                    &mut content.boundaries[backward.index()].exits,
                    vec![path.clone()],
                );
                content.links.extend(
                    returns
                        .into_iter()
                        .map(|exit| Link::new(exit, backward, path.clone())),
                );
            }
        }
        for boundary in &mut content.boundaries {
            boundary.entry = Some(path.clone());
        }
        content
    }

    pub fn resolve(self) -> Graph {
        let mut graph = Graph::default();
        // Resolve all declarations before resolving any links: their encounter
        // order cannot decide whether a destination exists.
        for stop in self.stops {
            let path = stop.path.clone();
            if let Some(previous) = graph.stops.get(&path) {
                if previous.entry != stop.entry || !previous.scope.same_location(&stop.scope, &path)
                {
                    graph.issues.push(Issue::ConflictingStop(path));
                }
            } else {
                graph.stops.insert(path, stop);
            }
        }
        for issue in &graph.issues {
            if let Issue::ConflictingStop(path) = issue {
                graph.stops.remove(path);
            }
        }
        let mut candidates: HashMap<Port, Option<Path>> = HashMap::new();
        for link in self.links {
            if !graph.stops.contains_key(&link.from.path) || !graph.stops.contains_key(&link.to) {
                graph.issues.push(Issue::MissingStop(link.from));
                continue;
            }
            match candidates.entry(link.from.clone()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(Some(link.to));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    if entry.get().as_ref().is_some_and(|prior| *prior != link.to) {
                        graph.issues.push(Issue::ConflictingLink(link.from));
                        entry.insert(None);
                    }
                }
            }
        }
        graph.links = candidates
            .into_iter()
            .filter_map(|(port, to)| to.map(|to| (port, to)))
            .collect();
        graph
    }
}

#[derive(Debug)]
pub enum Issue {
    ConflictingStop(Path),
    ConflictingLink(Port),
    MissingStop(Port),
}

impl std::fmt::Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConflictingStop(path) => write!(f, "incompatible stops at {path:?}"),
            Self::ConflictingLink(port) => write!(f, "multiple destinations from {port:?}"),
            Self::MissingStop(port) => write!(f, "link from {port:?} refers to a missing stop"),
        }
    }
}

#[derive(Default)]
pub struct Graph {
    stops: HashMap<Path, Stop>,
    links: HashMap<Port, Path>,
    issues: Vec<Issue>,
}

impl Graph {
    pub fn destination(&self, from: &[Step], direction: Direction) -> Option<&Stop> {
        self.stops.get(self.links.get(&Port {
            path: Rc::from(from),
            direction,
        })?)
    }

    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }
}
