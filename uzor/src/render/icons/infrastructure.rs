//! Infrastructure and facility icons
//!
//! Icons for airports, ports, industrial facilities, and installations.

/// Airport
/// Source: Mapbox Maki
/// License: CC0-1.0 (Public Domain)
/// viewBox: 0 0 15 15 (Maki uses 15x15 grid, NOT 24x24!)
///
/// Design: Top-down airplane silhouette
pub const AIRPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" id="airport" xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15">
  <path id="path7712-0" d="M15,6.8182L15,8.5l-6.5-1&#xA;&#x9;l-0.3182,4.7727L11,14v1l-3.5-0.6818L4,15v-1l2.8182-1.7273L6.5,7.5L0,8.5V6.8182L6.5,4.5v-3c0,0,0-1.5,1-1.5s1,1.5,1,1.5v2.8182&#xA;&#x9;L15,6.8182z"/>
</svg>"#;

/// Seaport / harbor
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Traditional ship anchor symbol
pub const PORT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 6v16" /><path d="m19 13 2-1a9 9 0 0 1-18 0l2 1" /><path d="M9 11h6" /><circle cx="12" cy="4" r="2" /></svg>"#;

/// Heliport
/// Source: Mapbox Maki
/// License: CC0-1.0 (Public Domain)
/// viewBox: 0 0 15 15 (Maki 15x15 grid)
///
/// Design: Helicopter landing pad with H marker
pub const HELIPORT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" id="heliport" xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15">
  <path id="path10415" d="M4,2C3,2,3,3,4,3h4v1C7.723,4,7.5,4.223,7.5,4.5V5H5H3.9707H3.9316&#xA;&#x9;C3.7041,4.1201,2.9122,3.5011,2,3.5c-1.1046,0-2,0.8954-2,2s0.8954,2,2,2c0.3722-0.001,0.7368-0.1058,1.0527-0.3027L5.5,10.5&#xA;&#x9;C6.5074,11.9505,8.3182,12,9,12h5c0,0,1,0,1-1v-0.9941C15,9.2734,14.874,8.874,14.5,8.5l-3-3c0,0-0.5916-0.5-1.2734-0.5H9.5V4.5&#xA;&#x9;C9.5,4.223,9.277,4,9,4V3h4c1,0,1-1,0-1C13,2,4,2,4,2z M2,4.5c0.5523,0,1,0.4477,1,1s-0.4477,1-1,1s-1-0.4477-1-1&#xA;&#x9;C1,4.9477,1.4477,4.5,2,4.5z M10,6c0.5,0,0.7896,0.3231,1,0.5L13.5,9H10c0,0-1,0-1-1V7C9,7,9,6,10,6z"/>
</svg>"#;

/// Fuel station
/// Source: Mapbox Maki
/// License: CC0-1.0 (Public Domain)
/// viewBox: 0 0 15 15 (Maki 15x15 grid)
///
/// Design: Gas/fuel pump icon
pub const FUEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg height="15" viewBox="0 0 15 15" width="15" xmlns="http://www.w3.org/2000/svg" id="fuel">
  <path d="m14 6v5.5c0 .2761-.2239.5-.5.5s-.5-.2239-.5-.5v-2c0-.8284-.6716-1.5-1.5-1.5h-1.5v-6c0-.5523-.4477-1-1-1h-6c-.5523 0-1 .4477-1 1v11c0 .5523.4477 1 1 1h6c.5523 0 1-.4477 1-1v-4h1.5c.2761 0 .5.2239.5.5v2c0 .8284.6716 1.5 1.5 1.5s1.5-.6716 1.5-1.5v-6.5c0-.5523-.4477-1-1-1v-1.51c-.0054-.2722-.2277-.4901-.5-.49-.2816.0047-.5062.2367-.5015.5184.0002.0105.0007.0211.0015.0316v2.45c0 .5523.4477 1 1 1s1-.4477 1-1-.4477-1-1-1zm-5 .5c0 .2761-.2239.5-.5.5h-5c-.2761 0-.5-.2239-.5-.5v-3c0-.2761.2239-.5.5-.5h5c.2761 0 .5.2239.5.5z"/>
</svg>"#;

/// Hospital
/// Source: Mapbox Maki
/// License: CC0-1.0 (Public Domain)
/// viewBox: 0 0 15 15 (Maki 15x15 grid)
///
/// Design: Medical cross symbol
pub const HOSPITAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" id="hospital" xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 15 15">
  <path id="rect4194" d="M7,1C6.4,1,6,1.4,6,2v4H2C1.4,6,1,6.4,1,7v1&#xA;&#x9;c0,0.6,0.4,1,1,1h4v4c0,0.6,0.4,1,1,1h1c0.6,0,1-0.4,1-1V9h4c0.6,0,1-0.4,1-1V7c0-0.6-0.4-1-1-1H9V2c0-0.6-0.4-1-1-1H7z"/>
</svg>"#;

/// Factory / industrial plant
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Factory building with smokestack
pub const FACTORY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 16h.01" /><path d="M16 16h.01" /><path d="M3 19a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8.5a.5.5 0 0 0-.769-.422l-4.462 2.844A.5.5 0 0 1 15 10.5v-2a.5.5 0 0 0-.769-.422L9.77 10.922A.5.5 0 0 1 9 10.5V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2z" /><path d="M8 16h.01" /></svg>"#;

/// Satellite
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Communications satellite
pub const SATELLITE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m13.5 6.5-3.148-3.148a1.205 1.205 0 0 0-1.704 0L6.352 5.648a1.205 1.205 0 0 0 0 1.704L9.5 10.5" /><path d="M16.5 7.5 19 5" /><path d="m17.5 10.5 3.148 3.148a1.205 1.205 0 0 1 0 1.704l-2.296 2.296a1.205 1.205 0 0 1-1.704 0L13.5 14.5" /><path d="M9 21a6 6 0 0 0-6-6" /><path d="M9.352 10.648a1.205 1.205 0 0 0 0 1.704l2.296 2.296a1.205 1.205 0 0 0 1.704 0l4.296-4.296a1.205 1.205 0 0 0 0-1.704l-2.296-2.296a1.205 1.205 0 0 0-1.704 0z" /></svg>"#;

/// Radio tower / broadcast tower
/// Source: Lucide Icons
/// License: MIT (ISC)
///
/// Design: Antenna tower with radio waves
pub const RADIO_TOWER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4.9 16.1C1 12.2 1 5.8 4.9 1.9" /><path d="M7.8 4.7a6.14 6.14 0 0 0-.8 7.5" /><circle cx="12" cy="9" r="2" /><path d="M16.2 4.8c2 2 2.26 5.11.8 7.47" /><path d="M19.1 1.9a9.96 9.96 0 0 1 0 14.1" /><path d="M9.5 18h5" /><path d="m8 22 4-11 4 11" /></svg>"#;

/// Offshore oil platform (side-view)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Side-view oil rig — support legs, platform deck, drilling derrick, flare
pub const OIL_PLATFORM: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="2" y1="22" x2="22" y2="22"/><line x1="5" y1="22" x2="8" y2="14"/><line x1="19" y1="22" x2="16" y2="14"/><line x1="9" y1="22" x2="10" y2="14"/><line x1="15" y1="22" x2="14" y2="14"/><rect x="4" y="12" width="16" height="2"/><path d="M10 12 L10 6 L14 6 L14 12"/><path d="M11 6 L11 3 L13 3 L13 6"/><path d="M12 3 L12 1"/><circle cx="12" cy="1" r="0.8" fill="currentColor"/></svg>"#;

/// Offshore gas platform (side-view)
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Side-view gas platform — support legs, deck, processing towers, LNG sphere
pub const GAS_PLATFORM: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><line x1="2" y1="22" x2="22" y2="22"/><line x1="5" y1="22" x2="7" y2="14"/><line x1="19" y1="22" x2="17" y2="14"/><line x1="10" y1="22" x2="10" y2="14"/><line x1="14" y1="22" x2="14" y2="14"/><rect x="4" y="12" width="16" height="2"/><rect x="5" y="7" width="2" height="5"/><rect x="9" y="5" width="2" height="7"/><rect x="13" y="7" width="2" height="5"/><circle cx="18" cy="9" r="3"/></svg>"#;

/// Oil/gas refinery or processing facility
/// Source: Custom hand-drawn
/// License: Public domain
///
/// Design: Industrial facility with tanks and chimney/flare stack
pub const REFINERY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="8" cy="16" r="5"/><circle cx="16" cy="16" r="4"/><rect x="11" y="2" width="2" height="12"/><path d="M10 2 L14 2 L13 5 L11 5 Z"/></svg>"#;
