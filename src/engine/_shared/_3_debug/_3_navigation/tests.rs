use super::*;
fn location(line: u32) -> SourceLocation {
    SourceLocation {
        file: Some("untrusted/path.c".into()),
        line: Some(line),
        column: None,
        function: Some("function".into()),
    }
}
#[test]
fn source_navigation_uses_real_depth_and_preserves_breakpoints_and_limits() {
    let mut next = SourceNavigation::at(SourceNavigationKind::Next, 2, location(7));
    assert_eq!(
        next.decide(3, &location(8), false, &Stop::Stopped { signal: 5 }),
        None
    );
    assert_eq!(
        next.decide(2, &location(7), false, &Stop::Stopped { signal: 5 }),
        None
    );
    assert_eq!(
        next.decide(2, &location(8), false, &Stop::Stopped { signal: 5 }),
        Some("complete")
    );
    assert_eq!(
        next.decide(3, &location(7), true, &Stop::Stopped { signal: 5 }),
        Some("breakpoint")
    );
    assert_eq!(
        next.decide(2, &location(7), false, &Stop::Stopped { signal: 2 }),
        Some("signal")
    );
    let mut finish = SourceNavigation::at(SourceNavigationKind::Finish, 2, location(7));
    assert_eq!(
        finish.decide(2, &location(8), false, &Stop::Stopped { signal: 5 }),
        None
    );
    assert_eq!(
        finish.decide(1, &location(8), false, &Stop::Stopped { signal: 5 }),
        Some("complete")
    );
    finish.steps = 10_000;
    assert_eq!(
        finish.decide(2, &location(8), false, &Stop::Stopped { signal: 5 }),
        Some("limit")
    );
}
