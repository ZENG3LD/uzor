//! Aviation icons — top-down silhouettes for flight tracking
//!
//! All aircraft icons show top-down view with nose pointing UP (toward y=0),
//! matching standard radar/flight tracker displays.

/// Commercial jet (top-down, nose pointing up)
/// Source: FlightAware community (FlyingPeteNZ)
/// License: Community contribution (usage rights unclear)
///
/// Design: Generic swept-wing twin-jet silhouette, suitable for A320/B737 class
pub const JET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,1 2,1 2,3 0,18 4,1 0,-4 3,0 0,5 17,6 0,3 -15,-2 -9,0 0,12 -2,6 7,3 0,2 -8,-1 -1,2 -1,-2 -8,1 0,-2 7,-3 -2,-6 0,-12 -9,0 -15,2 0,-3 17,-6 0,-5 3,0 0,4 4,-1 0,-18 2,-3 2,-1z"/></svg>"#;

/// Large twin-engine jet (top-down)
/// Source: FlightAware community (FlyingPeteNZ)
/// License: Community contribution (usage rights unclear)
///
/// Design: Wide-body twin-jet profile, suitable for B777/B787/A350 class
pub const JET_LARGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,1 2,1 1,2 0,20 4,4 0,-4 3,0 0,4 -1,2 17,12 0,2 -16,-5 -7,0 0,13 -1,5 7,5 0,2 -8,-2 -1,2 -1,-2 -8,2 0,-2 7,-5 -1,-5 0,-13 -7,0 -16,5 0,-2 17,-12 -1,-2 0,-4 3,0 0,4 4,-4 0,-20 1,-2 2,-1z"/></svg>"#;

/// Small propeller aircraft (top-down)
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Side-view plane icon (Lucide doesn't have top-down)
pub const PROP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17.8 19.2 16 11l3.5-3.5C21 6 21.5 4 21 3c-1-.5-3 0-4.5 1.5L13 8 4.8 6.2c-.5-.1-.9.1-1.1.5l-.3.5c-.2.5-.1 1 .3 1.3L9 12l-2 3H4l-1 1 3 2 2 3 1-1v-3l3-2 3.5 5.3c.3.4.8.5 1.3.3l.5-.2c.4-.3.6-.7.5-1.2z" /></svg>"#;

/// Helicopter (top-down view)
/// Source: Tabler Icons
/// License: MIT
///
/// Design: Side-view helicopter (Tabler doesn't have true top-down)
pub const HELICOPTER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 10l1 2h6" /><path d="M12 9a2 2 0 0 0 -2 2v3c0 1.1 .9 2 2 2h7a2 2 0 0 0 2 -2c0 -3.31 -3.13 -5 -7 -5h-2" /><path d="M13 9l0 -3" /><path d="M5 6l15 0" /><path d="M15 9.1v3.9h5.5" /><path d="M15 19l0 -3" /><path d="M19 19l-8 0" /></svg>"#;

/// Military jet fighter (top-down)
/// Source: FlightAware community (derived from triangle)
/// License: Community contribution
///
/// Design: Simple delta-wing fighter silhouette
pub const MILITARY_JET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 32,0 32,64 -64,0z"/></svg>"#;

/// Drone / Quadcopter
/// Source: Tabler Icons
/// License: MIT
///
/// Design: Quadcopter with four rotors in cross configuration
pub const DRONE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 10h4v4h-4l0 -4" /><path d="M10 10l-3.5 -3.5" /><path d="M9.96 6a3.5 3.5 0 1 0 -3.96 3.96" /><path d="M14 10l3.5 -3.5" /><path d="M18 9.96a3.5 3.5 0 1 0 -3.96 -3.96" /><path d="M14 14l3.5 3.5" /><path d="M14.04 18a3.5 3.5 0 1 0 3.96 -3.96" /><path d="M10 14l-3.5 3.5" /><path d="M6 14.04a3.5 3.5 0 1 0 3.96 3.96" /></svg>"#;

/// Weather balloon or airship
/// Source: FlightAware community (FlyingPeteNZ)
/// License: Community contribution
///
/// Design: Hot air balloon with envelope and basket
pub const BALLOON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="m 27,1 10,0 3,1 3,1 1,1 2,1 6,6 1,2 1,1 1,3 1,3 0,10 -1,3 -1,3 -1,1 -1,2 -6,6 -2,1 -1,1 -2,1 -2,1 -2,8 -1,0 2,-8 -3,1 -6,0 -3,-1 2,8 9,0 0,6 -10,0 0,-6 -2,-8 -2,-1 -2,-1 -1,-1 -2,-1 -6,-6 -1,-2 -1,-1 -1,-3 -1,-3 0,-10 1,-3 1,-3 1,-1 1,-2 6,-6 2,-1 1,-1 3,-1 3,-1z"/></svg>"#;
