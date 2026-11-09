use super::*;

/// An emitter that draws nothing.
fn ignore(_: Point3, _: Point3, _: Axis, _: Option<&Tool>, _: bool) -> Result<(), InvalidPath> {
    Ok(())
}

#[test]
fn absolute_leaf_position_survives_refocusing_and_handles_disconnected_boundaries() {
    let mut program = Recording::default();
    for (x, length) in [(0.0, 2.0), (10.0, 4.0), (20.0, 8.0)] {
        let mut part = Recording::default();
        part.start_at([x, 0.0, 0.0], Axis::Z).unwrap();
        part.line_to([x + length, 0.0, 0.0]).unwrap();
        program.append_part(part);
    }
    for (position, focus, expected, completed, upcoming) in [
        (1.25, 0..3, 11.0, 3.0, 11.0),
        (1.25, 1..2, 11.0, 3.0, 3.0),
        (1.25, 1..3, 11.0, 3.0, 11.0),
        (1.25, 2..3, 20.0, 6.0, 8.0),
        (1.0, 0..3, 10.0, 2.0, 12.0),
        (2.0, 0..3, 20.0, 6.0, 8.0),
        (2.0, 1..2, 14.0, 6.0, 0.0),
        (3.0, 0..3, 28.0, 14.0, 0.0),
    ] {
        let mut lengths = [0.0; 2];
        let pose = program
            .playback_cursor::<InvalidPath>(
                Cursor::Position(position),
                Some(focus),
                |a, b, _, _, done| {
                    lengths[usize::from(!done)] += distance(a, b);
                    Ok(())
                },
            )
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(pose.tip[0], expected);
        assert_eq!(lengths, [completed, upcoming]);
    }
}

#[test]
fn focusing_uses_selected_distance_and_keeps_the_completed_prefix() {
    let mut program = Recording::default();
    for (x, length) in [(0.0, 2.0), (10.0, 4.0), (20.0, 8.0)] {
        let mut part = Recording::default();
        part.start_at([x, 0.0, 0.0], Axis::Z).unwrap();
        part.line_to([x + length, 0.0, 0.0]).unwrap();
        program.append_part(part);
    }
    for (progress, x) in [(0.0, 10.0), (0.5, 12.0), (1.0, 14.0)] {
        let mut complete = 0.0;
        let mut upcoming = 0.0;
        let pose = program
            .playback_cursor::<InvalidPath>(
                Cursor::Progress(progress),
                Some(1..2),
                |a, b, _, _, done| {
                    assert!(b[0] <= 14.0, "later groups are not visited");
                    if done {
                        complete += distance(a, b);
                    } else {
                        upcoming += distance(a, b);
                    }
                    Ok(())
                },
            )
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(pose.tip[0], x);
        assert_eq!(complete, 2.0 + progress * 4.0);
        assert_eq!(upcoming, (1.0 - progress) * 4.0);
    }
    assert!(
        program
            .playback_cursor(Cursor::Progress(0.0), Some(3..4), ignore)
            .is_err()
    );
    assert_eq!(
        program
            .playback_cursor(Cursor::Progress(0.0), Some(1..1), ignore)
            .unwrap(),
        None
    );
}

#[test]
fn seeking_is_by_distance_and_never_draws_across_path_breaks() {
    let mut path = Recording::default();
    path.start_at([0.0, 0.0, 0.0], Axis::Z).unwrap();
    path.line_to([1.0, 0.0, 0.0]).unwrap();
    path.start_at([10.0, 0.0, 0.0], Axis::Z).unwrap();
    path.line_to([13.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        path.segments().map(|(a, b, _)| distance(a, b)).sum::<f64>(),
        4.0
    );
    for (progress, expected) in [(0.0, 0.0), (0.25, 1.0), (0.5, 11.0), (1.0, 13.0)] {
        let mut completed = 0.0;
        let position = path
            .playback_cursor::<InvalidPath>(Cursor::Progress(progress), None, |a, b, _, _, done| {
                assert!(distance(a, b) <= 3.0);
                if done {
                    completed += distance(a, b);
                }
                Ok(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            position.0,
            Pose {
                tip: [expected, 0.0, 0.0],
                axis: Axis::Z
            }
        );
        assert_eq!(completed, progress * 4.0);
    }
}

#[test]
fn playback_preserves_consumer_errors_and_stops_emitting() {
    #[derive(Debug, PartialEq)]
    enum Error {
        Path(InvalidPath),
        Consumer,
    }
    impl From<InvalidPath> for Error {
        fn from(error: InvalidPath) -> Self {
            Self::Path(error)
        }
    }
    let mut path = Recording::default();
    path.start_at([0.0; 3], Axis::Z).unwrap();
    path.line_to([1.0; 3]).unwrap();
    path.line_to([2.0; 3]).unwrap();
    let mut calls = 0;
    let result = path.playback_cursor(Cursor::Progress(1.0), None, |_, _, _, _, _| {
        calls += 1;
        Err(Error::Consumer)
    });
    assert_eq!(result, Err(Error::Consumer));
    assert_eq!(calls, 1);
    assert_eq!(
        path.playback_cursor::<Error>(Cursor::Progress(f64::NAN), None, |_, _, _, _, _| panic!(
            "invalid progress must not emit"
        )),
        Err(Error::Path(InvalidPath::NonFinitePoint)),
    );
}

#[test]
fn empty_zero_length_and_overflow_are_explicit() {
    let mut path = Recording::default();
    assert_eq!(
        path.playback_cursor(Cursor::Progress(0.5), None, ignore)
            .unwrap(),
        None
    );
    path.start_at([2.0; 3], Axis::Z).unwrap();
    path.line_to([2.0; 3]).unwrap();
    assert_eq!(
        path.playback_cursor(Cursor::Progress(0.5), None, ignore)
            .unwrap(),
        Some((
            Pose {
                tip: [2.0; 3],
                axis: Axis::Z
            },
            None
        ))
    );
    path.start_at([-f64::MAX; 3], Axis::Z).unwrap();
    path.line_to([f64::MAX; 3]).unwrap();
    assert_eq!(
        path.playback_cursor(Cursor::Progress(0.5), None, ignore),
        Err(InvalidPath::CoordinateRange)
    );
}
