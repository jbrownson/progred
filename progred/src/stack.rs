//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    absent, color, controls, def, editor, f32, f64, fidget, grap as grap_library, isa, layout,
    list, logic, name, notebook, png, presentation, random, section, sequence, text, toolpath,
    tree, u64,
};
use crate::library::{Libraries, Library};
use crate::projection::Partial;
use crate::projection::Projection;
use crate::workspace::Shown;
use std::borrow::Cow;
use std::rc::Rc;

/// The libraries Plain shows, for the editor and the tutorial page alike:
/// names, kinds, text, absent values and numbers read as themselves, and
/// everything else as Raw draws it.
pub const PLAIN: [gid::CellId; 7] = [
    name::ID,
    isa::ID,
    text::ID,
    absent::ID,
    f32::ID,
    f64::ID,
    u64::ID,
];

/// The sets of libraries the Projections menu offers by name. Any other set
/// is a custom one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    All,
    Plain,
    Raw,
}

impl Preset {
    pub const ALL: [Self; 3] = [Self::All, Self::Plain, Self::Raw];

    /// The libraries this preset shows.
    pub fn shown(self) -> Shown {
        match self {
            Self::All => Shown::ALL,
            Self::Plain => Shown::Only(PLAIN.to_vec()),
            Self::Raw => Shown::RAW,
        }
    }
}

#[derive(Clone)]
pub struct Stack {
    pub libraries: Libraries,
    pub projection: Projection,
    pub pane_projection: Projection,
    pub completions: crate::display::CompletionProvider,
    /// The projections, cells' projections and completion providers
    /// libraries bring, in load order, so an area can leave some of them off.
    partials: Rc<[(gid::CellId, Partial)]>,
    provided: Rc<[(gid::CellId, gid::CellId, Partial)]>,
    providers: Rc<[(gid::CellId, crate::display::CompletionProvider)]>,
}

impl Stack {
    /// Each library an area can switch off, one bringing a projection or
    /// offers, and its name, in load order.
    pub fn switchable(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        crate::libraries::name::library_names(&self.libraries).filter(|(library, _)| {
            self.partials.iter().any(|(id, _)| id == library)
                || self.provided.iter().any(|(id, _, _)| id == library)
                || self.providers.iter().any(|(id, _)| id == library)
        })
    }

    /// The libraries `shown` leaves off.
    pub fn hidden<'a>(&self, shown: &'a Shown) -> Cow<'a, [gid::CellId]> {
        match shown {
            Shown::AllBut(hidden) => Cow::Borrowed(hidden),
            Shown::Only(shown) => Cow::Owned(
                self.libraries
                    .iter()
                    .map(|(id, _)| id)
                    .filter(|id| !shown.contains(id))
                    .collect(),
            ),
        }
    }

    /// The preset showing what `shown` does of the libraries an area can
    /// switch off, if one does.
    pub fn preset(&self, shown: &Shown) -> Option<Preset> {
        Preset::ALL.into_iter().find(|preset| {
            let preset = preset.shown();
            self.switchable()
                .all(|(library, _)| preset.shows(library) == shown.shows(library))
        })
    }

    /// The completions offered with `hidden` libraries' own left off.
    pub fn completions_without(
        &self,
        hidden: &[gid::CellId],
    ) -> crate::display::CompletionProvider {
        crate::libraries::completion::combine(
            self.providers
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .cloned(),
        )
    }

    /// The document and pane projections with `hidden` libraries' own
    /// projections left off, a pane's presentation among them.
    pub fn without(&self, hidden: &[gid::CellId]) -> (Projection, Projection) {
        let document = projection(
            !hidden.contains(&def::ID),
            self.partials
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .map(|(_, partial)| partial.clone()),
            self.provided
                .iter()
                .filter(|(id, _, _)| !hidden.contains(id))
                .map(|(_, cell, partial)| (*cell, partial.clone())),
        );
        let pane = if hidden.contains(&presentation::ID) {
            document.clone()
        } else {
            pane(document.clone())
        };
        (document, pane)
    }
}

pub fn load() -> Stack {
    compose(BUILT_INS.iter().map(|(id, build)| (*id, build())))
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn load_selected(ids: &[gid::CellId]) -> Result<Stack, String> {
    ids.iter()
        .map(|id| {
            BUILT_INS
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, build)| (*id, build()))
                .ok_or_else(|| format!("Unknown built-in library: {id}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(compose)
}

fn compose(
    contributions: impl IntoIterator<Item = (gid::CellId, Library<crate::Editor, Hovered>)>,
) -> Stack {
    let (libraries, partials, provided, providers) = Libraries::from_contributions(contributions);
    let kinds: Vec<_> = provided
        .iter()
        .map(|(library, kind, _)| (*library, *kind))
        .collect();
    let libraries = crate::libraries::name::described(libraries, &kinds);
    let providers: Rc<[_]> = providers.into();
    let completions = crate::libraries::completion::combine(providers.iter().cloned());
    let partials: Rc<[_]> = partials.into();
    let provided: Rc<[_]> = provided.into();
    let projection = projection(
        true,
        partials.iter().map(|(_, partial)| partial.clone()),
        provided
            .iter()
            .map(|(_, cell, partial)| (*cell, partial.clone())),
    );
    let pane_projection = pane(projection.clone());
    Stack {
        libraries,
        pane_projection,
        projection,
        completions,
        partials,
        provided,
        providers,
    }
}

/// The libraries' projections. A record that says what kind of thing it is
/// is drawn first by its kind's def, where `defs` are drawn and the def has a
/// projection that draws it; then as a library provides for its kind.
fn projection(
    defs: bool,
    partials: impl IntoIterator<Item = Partial>,
    provided: impl IntoIterator<Item = (gid::CellId, Partial)>,
) -> Projection {
    Projection::new(
        defs.then(crate::libraries::def::by_def)
            .into_iter()
            .chain([crate::display::by_kind()])
            .chain(partials),
    )
    .with_provided(crate::display::Provided::new(provided))
}

/// `projection` for a pane, whose entry may present its value instead.
fn pane(projection: Projection) -> Projection {
    projection.with_entry(crate::display::runtime_partial(presentation::pane_entry))
}

type BuildLibrary = fn() -> Library<crate::Editor, Hovered>;

const BUILT_INS: &[(gid::CellId, BuildLibrary)] = &[
    (name::ID, name::library),
    (isa::ID, isa::library),
    (def::ID, def::library),
    (section::ID, section::library),
    (notebook::ID, notebook::library),
    (text::ID, text::library),
    (png::ID, png::library),
    (absent::ID, absent::library),
    (color::ID, color::library),
    (controls::ID, controls::library),
    (tree::ID, tree::library),
    (fidget::ID, fidget::library),
    (toolpath::ID, toolpath::library),
    (grap_library::ID, grap_library::library),
    (f32::ID, f32::library),
    (f64::ID, f64::library),
    (u64::ID, u64::library),
    (logic::ID, logic::library),
    (list::ID, list::library),
    (sequence::ID, sequence::library),
    (random::ID, random::library),
    (presentation::ID, presentation::library),
    (layout::ID, layout::library),
    (editor::ID, editor::library),
];

#[cfg(test)]
#[path = "tests/stack.rs"]
mod tests;
