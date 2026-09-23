use super::*;
use crate::libraries::f64;
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::{CellId, Value};

pub mod vocabulary {
    use gid::CellId;
    pub const TOOL: CellId = CellId::from_u128(0xfbe28409798777a59a474570ec600c9a);
    pub const CUTTING: CellId = CellId::from_u128(0xd22a92c1020be6f690b5cb738b728fd3);
    pub const NON_CUTTING: CellId = CellId::from_u128(0x32ec688b8b412514e5a4ee4789b0e5d8);
    pub const RADIUS: CellId = CellId::from_u128(0x0c43e0deeecda93da913261dc2786878);
    pub const AXIAL: CellId = CellId::from_u128(0x1e87f4d476a2d51539d4c221b068b9f4);
    pub const CONVEX_ARC: CellId = CellId::from_u128(0xf37b32e135dc0170157a33f832a5773d);
    pub const CONCAVE_ARC: CellId = CellId::from_u128(0xae836a45a572d83608097e2e14726a07);
    pub const BALL_MILL: CellId = CellId::from_u128(0x79c14e901b08feb6c3ae78f1edb50ab9);
    pub const SQUARE_MILL: CellId = CellId::from_u128(0xfe82f5a06ac17b08b4c42cc1a0a9f374);
    pub const BULL_MILL: CellId = CellId::from_u128(0xc614260c7754cd2d311d437fd6e22a19);
    pub const CORNER_RADIUS: CellId = CellId::from_u128(0x16528b8b5e24da34f153e36997b206d8);
}
use vocabulary::*;

pub(crate) fn names() -> impl Iterator<Item = (CellId, &'static str)> {
    [
        (TOOL, "tool"),
        (CUTTING, "cutting"),
        (NON_CUTTING, "non-cutting"),
        (RADIUS, "radius"),
        (AXIAL, "axial"),
        (CONVEX_ARC, "convex arc"),
        (CONCAVE_ARC, "concave arc"),
        (BALL_MILL, "ball mill"),
        (SQUARE_MILL, "square mill"),
        (BULL_MILL, "bull mill"),
        (CORNER_RADIUS, "corner radius"),
    ]
    .into_iter()
}

/// Retain explicit numeric inputs: absent coordinates inherit, malformed ones
/// fail. Unrelated metadata is not part of the geometric convention.
fn coordinate(fields: &gid::Record, id: CellId, previous: f64) -> Option<f64> {
    match fields.get(&id) {
        Some(value) => f64::read(value).filter(|v| v.is_finite() && *v >= 0.0),
        None => Some(previous),
    }
}

/// Lower moves to the connected bands consumed by the sweep. The caller owns
/// the current point across cutting/non-cutting boundaries.
/// Travel on the axis has no volume; radial edges at the ends are implicit caps.
fn sections(value: &Value, kind: SectionKind, current: &mut Point) -> Option<Vec<Section>> {
    fn finish(section: &mut Section, result: &mut Vec<Section>, next: Point) {
        if matches!(section.profile.last(), Some(Segment::Shoulder(_))) {
            section.profile.pop();
        }
        let previous = std::mem::replace(
            section,
            Section {
                kind: section.kind,
                start: next,
                profile: Vec::new(),
            },
        );
        if !previous.profile.is_empty() {
            result.push(previous);
        }
    }
    let mut section = Section {
        kind,
        start: *current,
        profile: Vec::new(),
    };
    let mut result = Vec::new();
    for value in value.as_list()?.values() {
        let fields = value.as_record()?;
        if !fields.contains_key(&RADIUS) && !fields.contains_key(&AXIAL) {
            return None;
        }
        let end = Point::new(
            coordinate(fields, RADIUS, current.radius)?,
            coordinate(fields, AXIAL, current.axial)?,
        );
        if !end.valid() || end.axial < current.axial {
            return None;
        }
        let arc = match (fields.get(&CONVEX_ARC), fields.get(&CONCAVE_ARC)) {
            (None, None) => None,
            (Some(r), None) => Some((f64::read(r)?, Bend::Convex)),
            (None, Some(r)) => Some((f64::read(r)?, Bend::Concave)),
            _ => return None,
        };
        if let Some((radius, bend)) = arc {
            section.profile.push(Segment::Arc { end, radius, bend });
        } else if end.axial == current.axial {
            if section.profile.is_empty() {
                section.start = end;
            } else if let Some(Segment::Shoulder(radius)) = section.profile.last_mut() {
                *radius = end.radius;
            } else if end.radius != current.radius {
                section.profile.push(Segment::Shoulder(end.radius));
            }
        } else if current.radius == 0.0 && end.radius == 0.0 {
            finish(&mut section, &mut result, end);
        } else {
            section.profile.push(Segment::Line(end));
        }
        *current = end;
    }
    finish(&mut section, &mut result, *current);
    (!result.is_empty()).then_some(result)
}

fn profile_value(section: &Section, current: &mut Point) -> Value {
    let mut moves = Vec::new();
    // Connected sections continue directly. A gap needs explicit travel along
    // the axis, rather than an unintended cylinder or taper between the bands.
    if section.start.axial != current.axial {
        if current.radius != 0.0 {
            moves.push(Value::record([(RADIUS, f64::value(0.0))]));
            current.radius = 0.0;
        }
        moves.push(Value::record([(AXIAL, f64::value(section.start.axial))]));
    }
    if section.start.radius != current.radius {
        moves.push(Value::record([(RADIUS, f64::value(section.start.radius))]));
    }
    *current = section.start;
    for segment in &section.profile {
        let (end, arc) = match *segment {
            Segment::Line(end) => (end, None),
            Segment::Shoulder(radius) => (Point::new(radius, current.axial), None),
            Segment::Arc { end, radius, bend } => (
                end,
                Some((
                    match bend {
                        Bend::Convex => CONVEX_ARC,
                        Bend::Concave => CONCAVE_ARC,
                    },
                    f64::value(radius),
                )),
            ),
        };
        let mut fields = Vec::new();
        if end.radius != current.radius || end == *current {
            fields.push((RADIUS, f64::value(end.radius)));
        }
        if end.axial != current.axial {
            fields.push((AXIAL, f64::value(end.axial)));
        }
        fields.extend(arc);
        moves.push(Value::record(fields));
        *current = end;
    }
    Value::list(moves)
}

impl Tool {
    pub fn value(&self) -> Value {
        let mut current = Point::new(0.0, 0.0);
        Value::record([(
            TOOL,
            Value::list(self.sections.iter().map(|s| {
                let kind = match s.kind {
                    SectionKind::Cutting => CUTTING,
                    SectionKind::NonCutting => NON_CUTTING,
                };
                Value::record([(kind, profile_value(s, &mut current))])
            })),
        )])
    }

    pub fn read(value: &Value) -> Option<Self> {
        let fields = value.as_record()?;
        let mut current = Point::new(0.0, 0.0);
        let sections = fields
            .get(&TOOL)?
            .as_list()?
            .values()
            .map(|s| {
                let s = s.as_record()?;
                let (kind, shape) = match (s.get(&CUTTING), s.get(&NON_CUTTING)) {
                    (Some(shape), None) => (SectionKind::Cutting, shape),
                    (None, Some(shape)) => (SectionKind::NonCutting, shape),
                    _ => return None,
                };
                sections(shape, kind, &mut current)
            })
            .collect::<Option<Vec<_>>>()?;
        Self::new(sections.into_iter().flatten().collect())
    }
}

pub(crate) fn functions(mut functions: ForeignFunctions) -> ForeignFunctions {
    use super::super::{
        invalid, number, result,
        vocabulary::{TOOL_DIAMETER, TOOL_LENGTH},
    };
    for id in [BALL_MILL, SQUARE_MILL, BULL_MILL] {
        functions = functions.register(
            id,
            ForeignFunction::from_value(move |context, call, env| {
                result((|| {
                    let diameter = number(context, call, env, TOOL_DIAMETER)?;
                    let length = number(context, call, env, TOOL_LENGTH)?;
                    let tool = match id {
                        BALL_MILL => Tool::ball(diameter, length),
                        SQUARE_MILL => Tool::square(diameter, length),
                        BULL_MILL => {
                            Tool::bull(diameter, number(context, call, env, CORNER_RADIUS)?, length)
                        }
                        _ => unreachable!(),
                    };
                    tool.map(|t| t.value()).ok_or_else(invalid)
                })())
            })
            .tracked(),
        );
    }
    functions
}
