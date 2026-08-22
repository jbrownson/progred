//! The resting pointer's answers.
//!
//! Hover mirrors painting: settled placements are asked back-to-front
//! what the pointer rests on. A direct answer always wins; an extended
//! answer may only retain the target that was already hovered. An
//! occluder is the claim analog of an opaque background fill, covering
//! whatever lies beneath without naming a target.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Claim<H> {
    Direct(H),
    Extended(H),
    Occludes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_distinguish_establishing_retaining_and_occluding() {
        assert_eq!(Claim::Direct(7), Claim::Direct(7));
        assert_eq!(Claim::Extended(7), Claim::Extended(7));
        assert_eq!(Claim::<u32>::Occludes, Claim::Occludes);
    }
}
