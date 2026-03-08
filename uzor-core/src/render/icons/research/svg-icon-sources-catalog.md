# SVG Icon Sources Catalog for Real-Time World Monitoring Map

Research compiled: 2026-02-16

This catalog evaluates free/open-source SVG icon collections for map-related and transport icons suitable for a real-time world monitoring application tracking aircraft, ships, weather, military activity, and infrastructure.

## Requirements Summary

**Target use case**: Real-time world monitoring map displaying:
- Aviation: commercial jets, propeller planes, helicopters, military aircraft, drones
- Maritime: cargo ships, tankers, container ships, fishing boats, naval vessels, submarines
- Vehicles: cars, trucks, trains, buses
- Weather: sun, clouds, rain, storms, tornadoes, snow, wind
- Natural disasters: earthquakes, volcanoes, tsunamis, fires, floods
- Military: bases, missiles, radar, vehicles, installations
- Infrastructure: power plants, nuclear facilities, oil rigs, pipelines, ports, airports
- Communication: satellites, antennas, cables, servers
- Markers: pins, dots, flags, warning symbols
- Geopolitical: protests, conflicts, borders, refugee movements

**Technical requirements**:
- SVG format with actual `<path d="..."/>` data (not icon fonts or PNGs)
- Permissive license for commercial use (MIT, Apache 2.0, CC0, CC-BY acceptable)
- Clean, consistent style suitable for small sizes (16-32px)
- Good contrast on dark map backgrounds

---

## Top Tier Sources (Highly Recommended)

### 1. Lucide Icons
**URL**: https://lucide.dev/icons
**GitHub**: https://github.com/lucide-icons/lucide
**License**: MIT
**Icon Count**: 1,653+ icons
**Format**: SVG files with clean path data

**Coverage for our needs**:
- **Aviation** ✓✓✓: plane, plane-landing, plane-takeoff, helicopter, drone (5 icons)
- **Maritime** ✓✓: ship, ship-wheel, sailboat, anchor (4 icons)
- **Vehicles** ✓✓✓: car, car-front, truck, bus, train-front, motorcycle, bicycle, scooter (15+ icons)
- **Weather** ✓✓✓: cloud variants (rain, snow, lightning, drizzle), sun, moon, wind, tornado, snowflake, thermometer (15+ icons)
- **Natural Disasters** ✓✓: flame, fire-extinguisher, biohazard, radiation, siren (5 icons)
- **Military** ✓: shield, sword, swords, bomb (4 icons)
- **Infrastructure** ✓✓: building, factory, landmark, bridge, dam, power (6+ icons)
- **Communication/Satellite** ✓✓✓: satellite, antenna, radio-tower, signal, wifi, bluetooth (10+ icons)
- **Map Markers** ✓✓✓: map-pin (multiple variants), map, navigation, compass, location (10+ icons)

**Quality**: Excellent. 24x24 grid, consistent 2px stroke, clean paths, designed for UI. Very readable at small sizes.

**Pros**: Fork of Feather Icons with active development, very clean and modern design, excellent for dark backgrounds.

**Cons**: Limited military-specific icons, no dedicated maritime vessel types (cargo ship, tanker, etc).

---

### 2. Tabler Icons
**URL**: https://tabler.io/icons
**GitHub**: https://github.com/tabler/tabler-icons
**License**: MIT
**Icon Count**: 4,985+ icons
**Format**: SVG files with path data

**Coverage for our needs**:
- **Aviation** ✓: plane, helicopter icons confirmed
- **Maritime** ✓: ship-related icons present
- **Vehicles** ✓✓✓: Extensive vehicle category (exact count TBD)
- **Weather** ✓✓✓: Dedicated weather category (exact count TBD)
- **Natural Disasters** ✓: Limited coverage
- **Military** ?: Not confirmed in main categories
- **Infrastructure** ✓✓: building, bridge icons
- **Communication/Satellite** ✓✓: antenna, wifi, signal icons
- **Map** ✓✓✓: Dedicated map category with markers

**Quality**: Excellent. 24x24 grid, 2px stroke, consistent design system.

**Pros**: Largest collection in this tier (nearly 5000 icons), very comprehensive, excellent for general UI needs.

**Cons**: Need to explore GitHub repo for exact military/maritime coverage. Sheer size may make finding specific icons challenging.

---

### 3. Bootstrap Icons
**URL**: https://icons.getbootstrap.com
**GitHub**: https://github.com/twbs/icons
**License**: MIT
**Icon Count**: 2,000+ icons (v1.13.1)
**Format**: SVG files with path data

**Coverage for our needs**:
- **Aviation** ✓✓: airplane, airplane-engines, airplane-engines-fill, airplane-fill (4 icons)
- **Maritime** ✓: ship, ship-fill (2 icons)
- **Vehicles** ✓✓: car-front, bus-front, bicycle, minecart (6+ icons with variants)
- **Weather** ✓✓✓: cloud (many variants), sun, moon, wind, hurricane (20+ icons) - v1.4.0 added 60+ weather icons
- **Natural Disasters** ✓✓: fire, flood, tornado/hurricane (3+ icons)
- **Military** ?: Not confirmed
- **Infrastructure** ✓✓: building (multiple variants), bridge, house, factory (10+ icons)
- **Communication/Satellites** ✓✓: broadcast, broadcast-pin, wifi, signal (6+ icons)
- **Map Markers** ✓✓✓: pin, pin-angle, pin-map, geo, geo-alt (10+ icons)

**Quality**: Excellent. Official Bootstrap design language, clean and consistent, works well at small sizes.

**Pros**: Strong weather category (60+ icons), good general coverage, widely used and tested.

**Cons**: Limited military icons, maritime coverage basic (only generic ship icons).

---

### 4. Phosphor Icons
**URL**: https://phosphoricons.com
**GitHub**: https://github.com/phosphor-icons/core
**License**: MIT
**Icon Count**: 1,240+ icons (across 6 weights)
**Format**: SVG files with path data, multiple weights

**Coverage for our needs**:
- **Aviation** ✓✓: airplane, airplane-in-flight, airplane-landing, airplane-takeoff, airplane-tilt, air-traffic-control (6+ icons)
- **Maritime** ?: Limited confirmation
- **Vehicles** ✓✓: car, truck, bus, bicycle, etc (count TBD)
- **Weather** ✓✓: cloud-lightning and other weather icons confirmed
- **Natural Disasters** ?: TBD
- **Military** ?: TBD
- **Infrastructure** ✓: building, factory icons
- **Communication/Satellite** ✓: antenna, satellite icons
- **Map Markers** ✓✓: pin, location markers

**Quality**: Excellent. Multiple weights available (thin, light, regular, bold, fill, duotone) - very flexible.

**Pros**: Six different weight options provide maximum flexibility. Good aviation coverage. Duotone style available.

**Cons**: Less comprehensive transport/maritime coverage than Tabler. Need to verify military icons.

---

### 5. Remix Icon
**URL**: https://remixicon.com
**GitHub**: https://github.com/Remix-Design/RemixIcon
**License**: Apache 2.0
**Icon Count**: 3,200+ icons
**Format**: SVG files with path data, outlined + filled styles

**Coverage for our needs**:
- **Aviation** ✓: plane and related icons (exact count TBD)
- **Maritime** ?: TBD
- **Vehicles** ✓✓: transport category exists
- **Weather** ✓✓: weather category exists
- **Natural Disasters** ?: TBD
- **Military** ?: TBD
- **Infrastructure** ✓: building, factory icons
- **Communication/Satellite** ✓: communication icons
- **Map Markers** ✓✓: map, pin icons

**Quality**: Excellent. Neutral style, 24x24 grid, both outlined and filled variants.

**Pros**: Large collection (3200+), two styles (outline + filled), neutral design works everywhere.

**Cons**: Need to verify specific category coverage. Apache 2.0 license (still permissive but requires attribution).

---

### 6. Iconoir
**URL**: https://iconoir.com
**GitHub**: https://github.com/iconoir-icons/iconoir
**License**: MIT
**Icon Count**: 1,671 icons
**Format**: SVG files with path data

**Coverage for our needs**:
- **Aviation** ✓✓: airplane, airplane-helix, airplane-rotation, drone (multiple variants), helicopter-adjacent (7+ icons)
- **Maritime** ✓: delivery, delivery truck (2 icons - limited maritime)
- **Vehicles** ✓✓✓: car, bicycle, bus, bus stop, EV charge/station/plug variants, gas tank variants (15+ icons)
- **Weather** ✓✓: cloud, cloud-sunny, fog, dew-point, lightning/flash variants (8+ icons)
- **Natural Disasters** ?: Limited
- **Military** ?: Not confirmed
- **Infrastructure** ✓✓✓: building, city, bank, church, farm, garage, bridge-3d, elevator (12+ icons)
- **Communication/Satellite** ✓✓✓: antenna, antenna-signal, antenna-signal-tag, airplay, bluetooth, wifi variants, data transfer (12+ icons)
- **Map Markers** ✓✓: compass, navigation arrows (4+ icons)

**Quality**: Excellent. Clean, modern design, 24x24 grid, good at small sizes.

**Pros**: Strong infrastructure and communication coverage. Good drone/aviation variety. Active development.

**Cons**: Very limited maritime coverage (no ships). No military icons confirmed.

---

## Specialized Map Icon Collections

### 7. Maki Icons (Mapbox)
**URL**: https://labs.mapbox.com/maki-icons
**GitHub**: https://github.com/mapbox/maki
**License**: CC0-1.0 (Public Domain)
**Icon Count**: 215+ POI icons
**Format**: SVG files, 15x15px

**Coverage for our needs**:
- **Aviation** ✓: airport, heliport, airfield icons
- **Maritime** ✓✓: harbor, ferry, marina, dock icons
- **Vehicles** ✓✓: bus, rail, bicycle, scooter, parking icons
- **Weather** ✗: Not designed for weather
- **Natural Disasters** ✗: Not included
- **Military** ✓: Limited (referenced in other projects)
- **Infrastructure** ✓✓✓: power plant, fuel, water, dam, pipeline, etc (excellent coverage)
- **Communication/Satellite** ✓: communications tower, cell tower
- **Map Markers** ✓✓✓: Designed specifically as map markers (excellent)

**Quality**: Excellent. Specifically designed for cartography at 15x15px size. Clean, readable, optimized for maps.

**Pros**: CC0 public domain license (most permissive). Designed FOR maps. Strong POI/infrastructure coverage.

**Cons**: Only 15x15px size (one size available). Not comprehensive (only "most common" POI). No weather icons. Limited military.

---

### 8. Temaki Icons
**URL**: https://rapideditor.github.io/temaki/docs
**GitHub**: https://github.com/ideditor/temaki
**License**: CC0-1.0 (Public Domain)
**Icon Count**: 300+ icons (estimated from 53 releases)
**Format**: SVG files, 15px for pins, 40px for labels

**Coverage for our needs**:
- **Aviation** ✓: Likely includes airport-related POI
- **Maritime** ✓: Likely includes port/marina POI
- **Vehicles** ✓✓: Extends Maki with additional transport
- **Weather** ✗: Not designed for weather
- **Natural Disasters** ?: TBD
- **Military** ?: TBD
- **Infrastructure** ✓✓: Extends Maki with specialized POI
- **Communication/Satellite** ✓: Antenna/tower icons
- **Map Markers** ✓✓✓: Designed for map POI markers

**Quality**: Good. Larger than Maki (15px/40px), accepts "messier" designs for specialized use cases.

**Pros**: CC0 public domain. Complements Maki with niche POI. Designed for mapping applications.

**Cons**: Less polished than Maki (intentionally). Need to browse gallery for full inventory. Limited weather/military.

---

### 9. SJJB Map Icons (OpenStreetMap)
**URL**: https://www.sjjb.co.uk/mapicons
**GitHub**: https://github.com/twain47/Open-SVG-Map-Icons
**License**: CC0 (Public Domain)
**Icon Count**: Unknown (website required for full inventory)
**Format**: SVG files, PNG generation tools available

**Coverage for our needs**:
- **Aviation** ✓: Referenced in changelog
- **Maritime** ✓: slipway, transport icons
- **Vehicles** ✓✓: transport parking, miniroundabout, speedbump, subway (confirmed)
- **Weather** ?: TBD
- **Natural Disasters** ?: TBD
- **Military** ✓: military bunker confirmed in changelog
- **Infrastructure** ✓✓: Various OSM tag-based POI
- **Communication/Satellite** ?: TBD
- **Map Markers** ✓✓✓: Designed for OSM cartography

**Quality**: Good. Simple, consistent vector icons for cartographic use with OSM/Mapnik.

**Pros**: CC0 public domain. OSM tag naming conventions. Military icons confirmed.

**Cons**: Requires browsing website for full catalog. Older project (less active updates).

---

### 10. OSMIC (OSM Map Icons Collection)
**URL**: https://gitlab.com/gmgeo/osmic
**GitHub Mirror**: https://github.com/gmgeo/osmic
**License**: CC0 (Public Domain)
**Icon Count**: 150+ clean SVG map icons
**Format**: SVG files

**Coverage for our needs**:
- **Aviation** ✓: OSM airport/airfield icons
- **Maritime** ✓: OSM harbor/marina icons
- **Vehicles** ✓✓: OSM transport icons
- **Weather** ✗: Not designed for weather
- **Natural Disasters** ✗: Not included
- **Military** ?: Limited OSM military POI
- **Infrastructure** ✓✓: OSM infrastructure POI
- **Communication/Satellite** ✓: OSM communication towers
- **Map Markers** ✓✓✓: Designed for OSM map markers

**Quality**: Good. Clean, high-quality SVG specifically for OSM-style maps.

**Pros**: CC0 public domain. Clean design. OSM-compatible.

**Cons**: Limited to OSM POI types. No weather/disaster icons.

---

## Specialized Weather Icon Collections

### 11. Weather Icons (Erik Flowers)
**URL**: https://erikflowers.github.io/weather-icons
**GitHub**: https://github.com/erikflowers/weather-icons
**License**: SIL OFL 1.1 (icons), MIT (code), CC BY 3.0 (documentation)
**Icon Count**: 222 weather-themed icons
**Format**: SVG files with path data, icon font available

**Coverage for our needs**:
- **Aviation** ✗: Not included
- **Maritime** ✓✓: Maritime weather icons included
- **Vehicles** ✗: Not included
- **Weather** ✓✓✓✓✓: Comprehensive weather coverage (primary purpose)
- **Natural Disasters** ✓✓: storm, tornado, hurricane, tsunami icons
- **Military** ✗: Not included
- **Infrastructure** ✗: Not included
- **Communication/Satellite** ✗: Not included
- **Map Markers** ✗: Not designed as markers

**Quality**: Excellent. Designed specifically for weather, inspired by Font Awesome styling.

**Pros**: Most comprehensive weather icon collection. Maritime weather included. Wind direction indicators (degree + cardinal). API compatibility CSS for Forecast.io, OpenWeatherMap, WMO, Weather Underground, Yahoo.

**Cons**: SIL OFL license (not as permissive as MIT/CC0, but still free for commercial use). Only weather - no transport/military/infrastructure.

**Special note**: 222 weather icons makes this the definitive weather icon source. If you need weather icons, this is THE collection.

---

### 12. Meteocons (Bas Milius)
**URL**: https://basmilius.github.io/weather-icons
**GitHub**: https://github.com/basmilius/weather-icons
**License**: MIT
**Icon Count**: 447 weather icons
**Format**: Animated + static SVG files, Lottie files, PNG

**Coverage for our needs**:
- **Aviation** ✗: Not included
- **Maritime** ?: Limited
- **Vehicles** ✗: Not included
- **Weather** ✓✓✓✓✓: Comprehensive animated weather icons
- **Natural Disasters** ✓: Storm/severe weather icons
- **Military** ✗: Not included
- **Infrastructure** ✗: Not included
- **Communication/Satellite** ✗: Not included
- **Map Markers** ✗: Not designed as markers

**Quality**: Excellent. Handcrafted animated SVGs, static versions available, monochrome font style available.

**Pros**: MIT license. Animated SVGs (unique feature). 447 icons (even more than Weather Icons). Static versions if animation not needed. Font formats available.

**Cons**: Only weather icons. Animation may be overkill for map markers (but static versions available).

**Special note**: Best choice if you want animated weather icons. If animation not needed, Erik Flowers' Weather Icons may be simpler.

---

## Military Symbol Libraries

### 13. Milsymbol (JavaScript)
**URL**: https://spatialillusions.com/milsymbol
**GitHub**: https://github.com/spatialillusions/milsymbol
**License**: MIT
**Icon Count**: 1000s of symbols (generated, not static)
**Format**: JavaScript library generating SVG or Canvas output

**Coverage for our needs**:
- **Aviation** ✓✓✓: MIL-STD-2525 aircraft symbols
- **Maritime** ✓✓✓: Naval vessel symbols
- **Vehicles** ✓✓✓: Ground vehicle symbols
- **Weather** ✗: Not included
- **Natural Disasters** ✗: Not included
- **Military** ✓✓✓✓✓: Full MIL-STD-2525 C/D/E, APP-6 B/D/E support
- **Infrastructure** ✓✓: Military installations
- **Communication/Satellite** ✓: Military communications
- **Map Markers** ✓✓: Military unit markers

**Standards Supported**:
- MIL-STD-2525 (versions C, D, E)
- STANAG APP-6 (versions B, D, E)
- FM 1-02.2

**Quality**: Excellent. Pure JavaScript, generates symbols from code (no images/fonts). Fully customizable (fill, frame, color, size, stroke).

**Pros**: MIT license. Generates SVG output. Full NATO/US military standard support. Fast (1000 symbols in <20ms). No dependencies. Used in military systems worldwide.

**Cons**: JavaScript library (not static SVG files). Requires understanding of SIDC codes. Overkill if you only need basic military icons.

**Integration**: Works with Angular, Cesium, D3, Leaflet, etc. Available via npm.

---

### 14. Mission Command Open Source
**URL**: https://missioncommand.github.io
**Sponsor**: US Army
**License**: Unknown (check individual GitHub repos)
**Format**: Android, Java, TypeScript/JavaScript libraries

**Coverage for our needs**:
- **Military** ✓✓✓: MIL-STD-2525 D, E, C support

**Standards Supported**:
- MIL-STD-2525 versions C, D, E

**Quality**: Professional (US Army sponsored).

**Pros**: Official US Army project. Multi-platform (Android, Java, JS/TS). Includes Extensible Map Platform (EMP).

**Cons**: License unclear (need to check repos). Library-based (not static SVGs). Primarily for military applications.

**Note**: More complex than milsymbol. Best for full military applications, not simple icon needs.

---

### 15. Python Military Symbols
**GitHub**: https://github.com/nwroyer/Python-Military-Symbols
**License**: Unknown (check repo)
**Icon Count**: Thousands (APP-6D compliant)
**Format**: Python library generating SVG from SIDC codes or natural language

**Coverage for our needs**:
- **Military** ✓✓✓: NATO APP-6(D) compliant symbols

**Quality**: Good. Natural language input is user-friendly.

**Pros**: Natural language support (e.g., "friendly infantry platoon" → symbol). Python-based. Generates SVG output.

**Cons**: Python required (not JavaScript). Less established than milsymbol. License TBD.

**Note**: Good choice for Python-based projects needing military symbols.

---

## General Icon Libraries (Lower Priority)

### 16. Feather Icons
**URL**: https://feathericons.com
**GitHub**: https://github.com/feathericons/feather
**License**: MIT
**Icon Count**: 286 icons
**Format**: SVG files with path data

**Coverage**: General UI icons, 24x24 grid, 2px stroke. Limited transport/weather/military coverage.

**Note**: Lucide Icons is a more actively maintained fork with 1600+ icons. Use Lucide instead.

---

### 17. Heroicons
**URL**: https://heroicons.com
**GitHub**: https://github.com/tailwindlabs/heroicons
**License**: MIT
**Icon Count**: 316 icons
**Format**: SVG files, 4 styles (outline 24x24, solid, mini, micro)

**Coverage**: General UI icons. map, map-pin, globe, flag, truck present. No dedicated aviation/maritime/weather/military categories.

**Pros**: Tailwind CSS team. 4 sizes/styles. Clean design.

**Cons**: Limited transport/weather/military coverage. Small collection (316 icons).

**Note**: Good for general UI, but not specialized enough for monitoring map needs.

---

### 18. Ionicons
**URL**: https://ionic.io/ionicons
**GitHub**: https://github.com/ionic-team/ionicons
**License**: MIT
**Icon Count**: 1,300 icons
**Format**: SVG, Material Design + iOS variants

**Coverage**: General mobile UI icons. Transport/infrastructure present but not detailed.

**Pros**: MIT license. iOS + Material Design variants. 1300 icons.

**Cons**: Mobile-focused. Limited specialization for monitoring needs.

---

### 19. Material Design Icons (Google)
**URL**: https://fonts.google.com/icons
**GitHub**: https://github.com/google/material-design-icons
**License**: Apache 2.0
**Icon Count**: 7,638+ icons
**Format**: SVG files

**Coverage**: Huge collection (7600+). Transport, weather categories exist. Comprehensive general coverage.

**Pros**: Apache 2.0 license. Massive collection. Official Google design system.

**Cons**: Apache 2.0 requires attribution. Less focused than specialized collections.

---

### 20. Material Design Icons (Pictogrammers)
**URL**: https://pictogrammers.com/library/mdi
**License**: Apache 2.0
**Icon Count**: 7,447 icons
**Format**: SVG, 24px height standard

**Coverage**: Community-driven Material Design icons. 7447 icons including weather pack (32 icons).

**Pros**: Apache 2.0. Very large collection. Weather iconpack available.

**Cons**: Apache 2.0 requires attribution. Overlaps with Google MDI.

---

### 21. Font Awesome Free
**URL**: https://fontawesome.com
**GitHub**: https://github.com/FortAwesome/Font-Awesome
**License**: Icons CC BY 4.0, Fonts SIL OFL 1.1, Code MIT
**Icon Count**: 2,000+ free icons (v6+)
**Format**: SVG files with path data

**Coverage**: General icons. Transport, weather, map markers present. Widely used.

**Pros**: Well-known. Large collection. Free tier available.

**Cons**: CC BY 4.0 requires attribution. Pro tier is paid (5000+ more icons). Not specialized for maps.

---

### 22. Simple Icons
**URL**: https://simpleicons.org
**GitHub**: https://github.com/simple-icons/simple-icons
**License**: CC0 (icons mostly, check individual icons)
**Icon Count**: 3,300+ brand logos
**Format**: SVG path data via JSON/NPM

**Coverage**: Brand logos only (FlightRadar24, MarineTraffic, airlines, shipping companies, etc).

**Pros**: CC0 public domain (mostly). 3300+ brands. SVG path data available.

**Cons**: Only brand logos, not functional icons. Limited use for map markers (but could be useful for labeling data sources).

**Note**: Useful for branding data sources ("powered by FlightRadar24"), not for aircraft icons.

---

### 23. OpenMoji
**URL**: https://openmoji.org
**GitHub**: https://github.com/hfg-gmuend/openmoji
**License**: CC-BY-SA 4.0
**Icon Count**: 4,000+ emojis
**Format**: SVG files (colored + outlined), PNG exports

**Coverage**: Emoji set including transport, weather emojis (airplane ✈️, ship 🚢, cloud ☁️, etc).

**Pros**: 4000+ emojis. Colored + outlined variants. Open-source.

**Cons**: CC-BY-SA (ShareAlike, more restrictive). Emoji style may not fit professional monitoring UI. Too playful for military/infrastructure monitoring.

**Note**: Not recommended for serious monitoring application. Better for consumer apps.

---

## Additional Research Targets (Lower Priority)

### SVG Repo, UXWing, Flaticon
These are **aggregator platforms** collecting icons from multiple sources. They're useful for searching, but:
- **Mixed licenses** (must check each icon individually)
- **Quality varies** across sources
- **Not cohesive** design systems
- Better to use original source libraries for consistent design

### FlightRadar24, Windy, MarineTraffic Assets
**Not recommended**:
- Proprietary assets (no open license confirmed)
- Terms of service unclear for icon reuse
- Risk of trademark/copyright issues
- Better to use open-source alternatives

---

## Licensing Summary

### Most Permissive (Public Domain)
- **CC0-1.0**: Maki, Temaki, SJJB, OSMIC, Simple Icons (mostly)
- No attribution required
- Can be used/modified/redistributed freely

### Very Permissive (Require Attribution)
- **MIT**: Lucide, Tabler, Bootstrap, Phosphor, Iconoir, Feather, Heroicons, Ionicons, Meteocons, Milsymbol
- Attribution required (usually in docs/credits)
- Can be used commercially

### Permissive (Require Attribution)
- **Apache 2.0**: Remix Icon, Material Design Icons
- Attribution required
- Patent grant included
- Can be used commercially

### Font-Specific
- **SIL OFL 1.1**: Weather Icons (icons), Font Awesome (fonts)
- For fonts/icon fonts
- Free for commercial use
- Must include copyright/license notice

### More Restrictive
- **CC-BY 4.0**: Font Awesome Free (icons)
- Attribution required
- ShareAlike not required

- **CC-BY-SA 4.0**: OpenMoji
- Attribution required
- **ShareAlike required** (derivative works must use same license)
- Most restrictive in this list

---

## Recommended Icon Stack for Monitoring Map

Based on this research, here's the recommended combination:

### Core General Icons: **Lucide Icons** (MIT, 1653 icons)
- Best balance of quality, coverage, and modern design
- Excellent for UI, markers, general transport, weather, infrastructure
- Clean 24x24 grid, works great at small sizes
- **Primary source** for most needs

### Supplementary General: **Tabler Icons** (MIT, 4985 icons)
- Fill gaps in Lucide coverage
- Largest MIT-licensed collection
- **Secondary source** for missing icons

### Map-Specific POI: **Maki Icons** (CC0, 215 icons)
- Designed FOR maps at 15x15px
- Strong infrastructure/POI coverage
- Public domain
- **Primary source** for map markers/POI

### Weather (if needed): **Meteocons** (MIT, 447 icons) OR **Weather Icons** (SIL OFL, 222 icons)
- Meteocons: animated + static, MIT license, more icons
- Weather Icons: static, comprehensive, maritime weather
- Choose based on animation needs
- **Primary source** for weather visualization

### Military Symbols (if needed): **Milsymbol** (MIT, thousands)
- Full NATO/US standard support
- Generates SVG from code
- MIT licensed
- **Only source** needed for military symbols
- Overkill for basic military icons (use Lucide/Tabler instead)

### Brands (if needed): **Simple Icons** (CC0 mostly, 3300+ brands)
- For labeling data sources ("via FlightRadar24")
- Public domain (mostly)

### Total Coverage
- **General + Transport**: Lucide (1653) + Tabler (4985) = 6,638 icons
- **Map POI**: Maki (215) + Temaki (~300) = 515 icons
- **Weather**: Meteocons (447) or Weather Icons (222)
- **Military**: Milsymbol (thousands)
- **Brands**: Simple Icons (3300+)

**Total available**: 10,000+ icons covering all requirements, all with permissive licenses (MIT/CC0).

---

## Implementation Notes

### SVG Path Data Extraction

All recommended sources provide actual SVG files with `<path>` elements:

```svg
<!-- Lucide Icon Example -->
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
  <path d="M12 2L2 7L12 12L22 7L12 2Z"/>
  <path d="M2 17L12 22L22 17"/>
</svg>
```

Extract the `d` attribute from `<path>` elements for use in your rendering system.

### Sizing for Maps (16-32px)

All recommended sources are designed on 24x24 grids with 2px strokes, which scale perfectly to 16-32px:
- 16px: 0.67x scale (slightly bolder strokes, still readable)
- 24px: 1x scale (original design size)
- 32px: 1.33x scale (slightly thinner strokes, more detail visible)

### Color for Dark Backgrounds

Recommended sources use `stroke="currentColor"` or `fill="currentColor"`, allowing easy color override:
- White/light gray for main icons
- Color coding by category (blue=maritime, green=commercial aviation, red=military, yellow=weather)

### File Organization

Suggested structure:
```
icons/
├── general/
│   ├── lucide/         # Primary general icons
│   └── tabler/         # Supplementary general icons
├── map/
│   ├── maki/           # Map POI markers
│   └── temaki/         # Specialized POI
├── weather/
│   └── meteocons/      # Weather icons (or weather-icons/)
├── military/
│   └── milsymbol/      # Military symbol generator
└── brands/
    └── simple-icons/   # Brand logos
```

---

## Sources

**Icon Libraries:**
- [Lucide Icons](https://lucide.dev/icons)
- [Lucide GitHub](https://github.com/lucide-icons/lucide)
- [Tabler Icons](https://tabler.io/icons)
- [Tabler GitHub](https://github.com/tabler/tabler-icons)
- [Bootstrap Icons](https://icons.getbootstrap.com)
- [Bootstrap Icons GitHub](https://github.com/twbs/icons)
- [Phosphor Icons](https://phosphoricons.com)
- [Phosphor GitHub](https://github.com/phosphor-icons/core)
- [Remix Icon](https://remixicon.com)
- [Remix GitHub](https://github.com/Remix-Design/RemixIcon)
- [Iconoir](https://iconoir.com)
- [Iconoir GitHub](https://github.com/iconoir-icons/iconoir)

**Map-Specific:**
- [Maki Icons (Mapbox)](https://labs.mapbox.com/maki-icons)
- [Maki GitHub](https://github.com/mapbox/maki)
- [Temaki Icons](https://rapideditor.github.io/temaki/docs)
- [Temaki GitHub](https://github.com/ideditor/temaki)
- [SJJB Map Icons](https://www.sjjb.co.uk/mapicons)
- [Open SVG Map Icons GitHub](https://github.com/twain47/Open-SVG-Map-Icons)
- [OSMIC GitLab](https://gitlab.com/gmgeo/osmic)
- [OSMIC GitHub Mirror](https://github.com/gmgeo/osmic)

**Weather-Specific:**
- [Weather Icons (Erik Flowers)](https://erikflowers.github.io/weather-icons)
- [Weather Icons GitHub](https://github.com/erikflowers/weather-icons)
- [Meteocons (Bas Milius)](https://basmilius.github.io/weather-icons)
- [Meteocons GitHub](https://github.com/basmilius/weather-icons)

**Military:**
- [Milsymbol](https://spatialillusions.com/milsymbol)
- [Milsymbol GitHub](https://github.com/spatialillusions/milsymbol)
- [Mission Command Open Source](https://missioncommand.github.io)
- [Python Military Symbols GitHub](https://github.com/nwroyer/Python-Military-Symbols)

**Other:**
- [Feather Icons](https://feathericons.com)
- [Heroicons](https://heroicons.com)
- [Ionicons](https://ionic.io/ionicons)
- [Material Design Icons (Google)](https://fonts.google.com/icons)
- [Material Design Icons (Pictogrammers)](https://pictogrammers.com/library/mdi)
- [Font Awesome](https://fontawesome.com)
- [Simple Icons](https://simpleicons.org)
- [OpenMoji](https://openmoji.org)

**Documentation:**
- [OpenStreetMap Map Icons Wiki](https://wiki.openstreetmap.org/wiki/Map_Icons)
- [SIL Open Font License 1.1](https://scripts.sil.org/OFL)
- [Creative Commons Licenses](https://creativecommons.org/licenses/)
- [MIT License](https://opensource.org/licenses/MIT)
- [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0)

---

## Next Steps

1. **Download recommended icon sets**: Lucide + Tabler (general), Maki (map POI), Meteocons (weather)
2. **Extract SVG path data** from needed icons
3. **Create icon registry** mapping icon names to SVG paths
4. **Implement rendering system** with color/size customization
5. **Test at target sizes** (16-32px) on dark map background
6. **Consider Milsymbol integration** if military symbol complexity needed (can add later)

---

**Research completed**: 2026-02-16
**Researcher**: research-agent (Claude Sonnet 4.5)
**Total sources evaluated**: 23 icon libraries
**Recommended stack**: Lucide + Tabler + Maki + Meteocons (9,000+ icons, all MIT/CC0)
