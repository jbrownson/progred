use super::*;

#[test]
fn painter_order_claim_composition_is_associative() {
    let choices = [
        None,
        Some(Claim::Direct(1)),
        Some(Claim::Direct(2)),
        Some(Claim::Extended(1)),
        Some(Claim::Extended(2)),
        Some(Claim::Occludes),
    ];
    let over = |base: Option<Claim<u8>>, above: Option<Claim<u8>>| {
        if above
            .as_ref()
            .is_some_and(|claim| claim.supersedes(base.as_ref()))
        {
            above
        } else {
            base
        }
    };
    for a in choices {
        for b in choices {
            for c in choices {
                assert_eq!(over(over(a, b), c), over(a, over(b, c)));
            }
        }
    }
    assert_eq!(
        over(Some(Claim::Direct(1)), Some(Claim::Extended(2))),
        Some(Claim::Direct(1))
    );
    assert_eq!(
        over(Some(Claim::Occludes), Some(Claim::Direct(2))),
        Some(Claim::Direct(2))
    );
}

#[test]
fn claims_distinguish_establishing_retaining_and_occluding() {
    assert_eq!(Claim::Direct(7), Claim::Direct(7));
    assert_eq!(Claim::Extended(7), Claim::Extended(7));
    assert_eq!(Claim::<u32>::Occludes, Claim::Occludes);
}

#[test]
fn probes_preserve_their_policy_when_targets_are_mapped() {
    let placement = Placement::new(
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Rect::new(0.0, 0.0, 12.0, 12.0),
    );
    for (probe, extends) in [
        (Probe::retaining(placement, 3), true),
        (Probe::exact(placement, 3), false),
        (
            Probe::dynamic(placement, |point| (point.x > 5.0).then_some(3)),
            false,
        ),
    ] {
        let probe = probe.map(|value| value + 1);
        assert_eq!(
            probe.answer(Point::new(7.0, 5.0), None, 4.0),
            Some(Claim::Direct(4))
        );
        assert_eq!(probe.answer(Point::new(11.0, 5.0), None, 4.0), None);
        assert_eq!(
            probe.answer(Point::new(11.0, 5.0), Some(&4), 4.0),
            extends.then_some(Claim::Extended(4))
        );
        assert_eq!(probe.answer(Point::new(13.0, 5.0), Some(&4), 4.0), None);
    }
    let occluder = Probe::<u32>::occludes(placement).map(|v| v + 1);
    assert_eq!(
        occluder.answer(Point::new(5.0, 5.0), Some(&4), 4.0),
        Some(Claim::Occludes)
    );
    assert_eq!(occluder.answer(Point::new(11.0, 5.0), Some(&4), 4.0), None);
}
