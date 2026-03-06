//! Integration tests for transitions module

use uzor_animation::recipes::transitions::*;
use uzor_animation::Easing;

#[test]
fn test_material_shared_axis_x_preset() {
    let transition = material_shared_axis_x();
    assert_eq!(transition.combined_duration_ms(), 300);

    let exit_timeline = transition.exit_timeline();
    let enter_timeline = transition.enter_timeline();

    assert!(exit_timeline.total_duration().as_millis() > 0);
    assert!(enter_timeline.total_duration().as_millis() > 0);
}

#[test]
fn test_cross_fade_preset() {
    let transition = cross_fade();
    assert_eq!(transition.combined_duration_ms(), 300);
}

#[test]
fn test_ios_push_preset() {
    let transition = ios_push();
    assert_eq!(transition.combined_duration_ms(), 350);
}

#[test]
fn test_zoom_in_preset() {
    let transition = zoom_in();
    assert_eq!(transition.combined_duration_ms(), 400);
}

#[test]
fn test_circle_reveal_preset() {
    let transition = circle_reveal();
    assert_eq!(transition.combined_duration_ms(), 600);
}

#[test]
fn test_stair_cascade_preset() {
    let transition = stair_cascade();
    let duration = transition.combined_duration_ms();
    assert!(duration >= 200);
    assert!(duration <= 1500);
}

#[test]
fn test_shared_axis_x_builder() {
    let transition = SharedAxisXBuilder::new()
        .enter_duration_ms(250)
        .exit_duration_ms(350)
        .distance(60.0)
        .easing(Easing::Linear)
        .build();

    if let TransitionAnimation::SharedAxisX {
        enter_duration_ms,
        exit_duration_ms,
        distance,
        easing,
        ..
    } = transition
    {
        assert_eq!(enter_duration_ms, 250);
        assert_eq!(exit_duration_ms, 350);
        assert_eq!(distance, 60.0);
        assert_eq!(easing, Easing::Linear);
    } else {
        panic!("Expected SharedAxisX variant");
    }
}

#[test]
fn test_circle_reveal_builder() {
    let transition = CircleRevealBuilder::new()
        .origin(100.0, 200.0)
        .duration_ms(800)
        .build();

    if let TransitionAnimation::CircleReveal {
        origin_x,
        origin_y,
        duration_ms,
        ..
    } = transition
    {
        assert_eq!(origin_x, 100.0);
        assert_eq!(origin_y, 200.0);
        assert_eq!(duration_ms, 800);
    } else {
        panic!("Expected CircleReveal variant");
    }
}

#[test]
fn test_crossfade_builder() {
    let transition = CrossFadeBuilder::new()
        .duration_ms(500)
        .easing(Easing::EaseInOutQuad)
        .build();

    if let TransitionAnimation::CrossFade {
        duration_ms,
        easing,
    } = transition
    {
        assert_eq!(duration_ms, 500);
        assert_eq!(easing, Easing::EaseInOutQuad);
    } else {
        panic!("Expected CrossFade variant");
    }
}

#[test]
fn test_slide_over_variants() {
    let right = slide_over();
    let left = slide_over_left();
    let top = slide_over_top();

    if let TransitionAnimation::SlideOver { direction, .. } = right {
        assert_eq!(direction, SlideDirection::Right);
    }

    if let TransitionAnimation::SlideOver { direction, .. } = left {
        assert_eq!(direction, SlideDirection::Left);
    }

    if let TransitionAnimation::SlideOver { direction, .. } = top {
        assert_eq!(direction, SlideDirection::Up);
    }
}
