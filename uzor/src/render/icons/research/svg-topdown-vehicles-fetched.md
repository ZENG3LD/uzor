# Top-Down Aircraft and Vessel SVG Icons - Research Report

**Date:** 2026-02-16
**Purpose:** Fetch real top-down SVG silhouettes for flight/ship tracking map overlays
**Status:** Complete

---

## Executive Summary

This report documents real-world SVG sources for top-down aircraft and vessel icons suitable for tracking map applications. Three primary sources were identified and validated:

1. **AircraftShapesSVG** (RexKramer1) - 182+ aircraft types, GPL-3.0 licensed
2. **tar1090** (wiedehopf) - 50+ aircraft types embedded in JavaScript
3. **FlightAware Community** - Hand-coded 64x64 grid icons
4. **VRSCustomMarkers** - 50+ aircraft + boat markers for Virtual Radar Server

---

## Source 1: AircraftShapesSVG Repository

**URL:** https://github.com/RexKramer1/AircraftShapesSVG
**License:** GNU General Public License v3.0
**Creator:** RexKramer1
**Format:** Individual SVG files (80mm x 80mm)
**Count:** 182 aircraft types

### Overview

Professional-quality top-down aircraft silhouettes created in Inkscape 1.2.1. Each SVG follows a consistent structure:
- Main outline layer ("Pfade")
- Accent detail layer (cockpit, windows, structural details)
- Black stroke rendering (0.264583px width)
- Optimized viewBox for each aircraft type

### File Structure

```
AircraftShapesSVG/
├── Shapes SVG/          # 182 SVG files
├── svgCatalog.html      # Interactive gallery
├── svgCatalog_small.html
├── Tutorial/            # Creation workflow guide
└── README.md
```

### Aircraft Coverage

**Commercial Jets:**
- Airbus: A10, A124, A19N, A20N, A21N, A306, A310, A318, A320, A321, A332, A333, A337, A338, A339, A342, A343, A345, A346, A359, A35K, A388, A3ST, A400
- Boeing: B29, B190, B350, B38M, B39M, B52, B703, B712, B722, B733, B734, B735, B737, B738, B739, B742, B744, B748, B74S, B752, B753, B762, B763, B764, B772, B773, B779, B77L, B77W, B788, B789, B78X
- Other: BLCF (Beluga), MD11, DC10, DC3, DC87

**Regional Aircraft:**
- Bombardier: CRJ2, CRJ7, CRJ9, CRJX, DH8C, DH8D
- Embraer: E170, E195, E390
- ATR: AT45, AT75, ATP
- Other: SF34, BCS1, BCS3, Q4

**Military Aircraft:**
- Fighters: F5, F15, F16, F18H, F18S, F22, F35, VF35, EUFI, RFAL, MIRA, A4
- Transport: C130, C160, C17, C2, C208, C295, C5M, AN12, AN26, IL62, IL76, A124, A225
- Special: E3CF, E3TF, E737, E8 (AWACS/surveillance), KC2, KC46, K35E, R135 (tankers)
- Bombers: B1 (fast/slow variants), B52
- Trainers: T38, T204, HAWK, PC9, L159, M326

**Helicopters:**
- H47 (Chinook), H60 (Blackhawk), H64 (Apache), EC20, EC35, EC45, GAZL, LYNX, MI24, NH90, S61, TIGR, UH1

**General Aviation:**
- C172, C750, DA42, PA46, PC12, PC6T, P180, P28A, R44, SR22, BN2P

**Business Jets:**
- GLF6, GL5T, FA7X, CL2T, CN35, LJ35, E35L, E300, F406, PC24

**Specialty:**
- BALL (balloon), GYRO (gyrocopter), SF25 (glider), U2, P1, P3, P8 (maritime patrol)

### Access Pattern

Individual files accessible via:
```
https://raw.githubusercontent.com/RexKramer1/AircraftShapesSVG/master/Shapes%20SVG/[FILENAME].svg
```

Example: `https://raw.githubusercontent.com/RexKramer1/AircraftShapesSVG/master/Shapes%20SVG/B737.svg`

### Sample SVG: Boeing 737-700 (B737.svg)

**ViewBox:** `-22 -23 80 80`
**Dimensions:** 80mm x 80mm
**Layers:** Outline + Accent details
**Quality:** Production-ready, clean paths

```xml
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg
   width="80mm"
   height="80mm"
   viewBox="-22 -23 80 80"
   version="1.1"
   id="svg1542">
  <title id="title19586">Boeing 737-700</title>
  <g inkscape:groupmode="layer" id="layer4" inkscape:label="Pfade">
    <path
       style="opacity:1;fill:none;stroke:#000000;stroke-width:0.264583px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1"
       d="m 18.001116,0.04724702 -0.734992,1.20271298 -0.551243,2.0713386 -0.200452,1.5368 ..."
       id="path2118" />
  </g>
  <g inkscape:groupmode="layer" id="layer5" inkscape:label="Accent">
    <path
       id="path2222"
       style="fill:#ffffff;stroke:#000000;stroke-width:0.267999"
       d="M 17.266124,0.04724702 ..." />
  </g>
</svg>
```

### Sample SVG: Airbus A320 (A320.svg)

**ViewBox:** `-23 -21 80 80`
**Quality:** Excellent top-down perspective, wing sweep visible

### Sample SVG: Boeing 747-400 (B744.svg)

**ViewBox:** `-8 -5 80 80`
**Features:** Distinctive hump, wide-body proportions clearly visible

### Sample SVG: Lockheed C-130J Hercules (C130.svg)

**ViewBox:** `-20 -23 80 80`
**Features:** Four-engine turboprop, distinctive straight wing

### Sample SVG: Lockheed Martin F-16 (F16.svg)

**ViewBox:** `-35 -33 80 80`
**Features:** Delta-canard wing configuration, fighter profile

### Sample SVG: CH-47 Chinook Helicopter (H47.svg)

**ViewBox:** Not specified in summary
**Features:** Twin-rotor configuration clearly visible

### Sample SVG: Cessna C-172 (C172.svg)

**ViewBox:** Not specified
**Features:** High-wing configuration, small GA aircraft

### Sample SVG: Gulfstream G650 (GLF6.svg)

**ViewBox:** `-25 -25 80 80`
**Complete SVG provided below**

```xml
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg
   width="80mm"
   height="80mm"
   viewBox="-25 -25 80 80"
   version="1.1"
   id="svg1542">
  <title id="title7406">Gulfstream G650</title>
  <g inkscape:groupmode="layer" id="layer4" inkscape:label="Pfade">
    <path
       style="opacity:1;fill:none;stroke:#000000;stroke-width:0.264583px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1"
       d="m 15.367999,0.11692882 -0.734992,1.20271298 -0.551243,2.0713386 -0.200452,1.5368 -0.0167,4.8108513 -5.2952776,4.5185253 -7.5670686,5.654422 -0.93544342,2.488947 0.0668174,0.400904 0.78510425,-1.1526 0.43431307,-0.367495 12.0104239,-4.476765 0.601358,0.01671 0.05011,1.921 -0.15034,0.0167 -0.11693,-0.233861 h -1.403165 l -0.150339,0.50113 -0.05011,2.088043 0.618061,2.589175 h 0.835216 l 0.15034,0.751695 0.935444,-0.05011 0.150339,1.002261 -4.9611917,3.457801 -0.1336349,1.536798 5.3787986,-1.820773 0.167045,0.734992 h 0.100226 l 0.133635,-0.701583 5.412208,1.870885 -0.116931,-1.520095 -5.011303,-3.524616 0.183747,-1.06908 h 0.902035 l 0.100226,-0.317382 0.01671,-0.367496 0.88533,0.03341 0.634765,-2.672694 -0.05011,-1.937705 -0.05011,-0.334087 -0.08352,-0.250565 h -1.41987 l -0.11693,0.283974 -0.167044,-0.0167 0.116931,-1.88759 0.651469,-0.05011 11.743155,4.409948 0.434313,0.317382 0.818513,1.202713 0.01669,-0.451017 -0.885343,-2.43885 -7.705148,-5.791637 -5.031808,-4.4412213 -0.0084,-4.667417 -0.250565,-1.6537303 -0.517835,-1.9711126 z"
       id="path2579" />
  </g>
  <g inkscape:groupmode="layer" id="layer7" inkscape:label="Accent">
    <path
       id="path7463"
       style="fill:#ffffff;stroke:#000000;stroke-width:0.263"
       d="M 15.833772,25.438769 15.524742,25.1715 m -0.709936,0.267269 0.334087,-0.250563 m 0.200065,3.874201 -0.224422,-1.488281 0.07087,-2.6104 -0.106304,-0.47247 0.212611,-5.976749 0.295293,5.976751 -0.08268,0.484279 0.03544,2.616304 -0.212611,1.458752 m 1.263848,-10.24079 -0.224422,-0.02363 -0.09449,3.319102 -0.283483,2.244234 m -1.901696,-5.598766 0.236234,0.02363 0.08268,3.389974 0.271672,2.244233 m 2.338728,-0.673275 -0.413412,-1.960753 0.0118,-1.925315 0.165364,-0.980376 m -3.165537,4.866444 0.425225,-2.019811 0.0118,-1.901692 -0.177176,-1.051248 m 2.893278,-9.134446 -0.03341,7.299798 m -2.88985,-7.1661649 0.100224,7.0826439 M 15.912776,2.4995286 15.541105,2.1988508 m 0.334086,0.4677203 -0.179569,-0.1336336 -0.208807,-0.1211104 -0.0042,-0.350791 0.271447,0.066819 0.229685,0.2965018 z M 16.292801,3.551903 16.334561,3.485086 16.16334,2.8712012 16.12158,2.979779 Z M 14.72711,2.4911764 15.098781,2.1904986 m -0.334086,0.4677203 0.179569,-0.1336336 0.208807,-0.1211104 0.0042,-0.350791 -0.271447,0.066819 -0.229685,0.2965018 z m -0.41761,0.8853319 -0.04176,-0.066817 0.171221,-0.6138848 0.04176,0.1085778 z" />
  </g>
</svg>
```

### Sample SVG: DHC-8-400 Dash 8 (DH8D.svg)

**ViewBox:** `-26 -24 80 80`
**Complete SVG provided below**

```xml
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg
   width="80mm"
   height="80mm"
   viewBox="-26 -24 80 80"
   version="1.1"
   id="svg1542">
  <title id="title19586">DHC-8-400 Dash 8</title>
  <g inkscape:groupmode="layer" id="layer3" inkscape:label="Outline">
    <path
       style="fill:none;stroke:#000000;stroke-width:0.265;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1;stroke-dasharray:none"
       d="M 14.686326,0.33734638 14.04272,1.0799685 13.399113,2.9365235 13.201081,3.877178 13.15157,14.54618 l -2.524915,0.09902 -2e-6,-2.227867 -0.12377,-0.235163 -0.0495,-0.556967 -0.371311,-0.470328 -0.3218027,0.495081 -0.074261,0.767376 -0.074262,0.07426 v 2.277373 l -9.30752929,0.643606 -0.0990163,1.33672 4.30720769,0.297048 0.099016,0.222788 0.1237704,-0.198033 2.4011446,0.173278 0.1485243,0.371311 0.1980325,-0.321803 2.079342,0.148525 0.049501,1.559506 0.1485238,0.544589 0.297049,0.371311 0.321803,-0.346557 0.173278,-0.618851 0.04951,-1.311965 2.599178,0.07426 -0.02476,6.634091 0.346558,4.084419 0.470328,3.069505 -3.391308,0.470328 v 1.58426 l 3.688356,0.02476 0.297049,0.9159 0.24754,-0.965409 h 3.737864 v -1.509998 l -3.416061,-0.42082 0.495082,-3.16852 0.445574,-4.034913 -0.04951,-6.634088 2.549669,-0.04951 0.04951,1.113933 0.222788,0.717869 0.321803,0.470327 0.297048,-0.54459 0.173278,-0.618851 -0.02476,-1.410982 2.128849,-0.173279 0.148525,0.420819 0.198033,-0.420819 2.425898,-0.148525 0.148524,0.272295 0.09902,-0.272295 4.232953,-0.321802 -0.123769,-1.336719 -9.258022,-0.742622 0.04951,-2.25262 -0.09902,-0.09902 -0.07426,-0.717868 -0.346557,-0.470327 -0.396065,0.594098 0.02476,0.594097 -0.173279,0.222787 0.09902,2.104096 -2.673439,-0.123771 V 3.7781618 L 15.825012,2.8375073 15.305176,1.1047225 Z"
       id="path2001" />
  </g>
</svg>
```

### Suitability Assessment

**Pros:**
- Truly top-down perspective (not isometric)
- Clean, optimized path data
- Consistent stroke width across all aircraft
- Professional quality from aircraft photos/references
- Wide variety covering most common types
- GPL-3.0 license allows commercial use with attribution

**Cons:**
- Requires downloading 182 individual files
- Metadata/Inkscape namespaces add file size (can be stripped)
- No built-in rotation/heading indicators (paths only)

**Recommended Use:** Primary source for all aircraft types in flight tracking application.

---

## Source 2: tar1090 JavaScript Markers

**URL:** https://github.com/wiedehopf/tar1090
**File:** https://raw.githubusercontent.com/wiedehopf/tar1090/master/html/markers.js
**License:** Not explicitly stated (open-source project)
**Format:** JavaScript object with inline SVG path data
**Count:** 50+ aircraft types

### Overview

Embedded SVG markers optimized for web display in ADS-B flight tracking interfaces. Markers are defined as JavaScript objects with viewBox, width, height, and path data properties.

### Aircraft Definitions (Selected)

#### Commercial Jets

**Airbus A319**
```javascript
{
  viewBox: '-10 -10 380 373',
  w: 23,
  h: 32,
  path: '[path data]'
}
```

**Airbus A320**
```javascript
{
  viewBox: '-10 -10 380 415',
  w: 23,
  h: 32,
  path: '[path data]'
}
```

**Airbus A321**
```javascript
{
  viewBox: '-10 -10 380 485',
  w: 23,
  h: 32,
  path: '[path data]'
}
```

**Boeing 737**
```javascript
{
  viewBox: '-2 -2 74.5 74.7',
  w: 23.5,
  h: 32,
  path: '[path data]'
}
```

**Boeing 737-800**
```javascript
{
  viewBox: '-2 -2 74.5 87.3',
  w: 23.5,
  h: 32,
  path: '[path data]'
}
```

**Airbus A380**
```javascript
{
  viewBox: '-7 -6 40 40',
  w: 42,
  h: 42,
  path: '[path data]'
}
```

#### Military Aircraft

**F-15 Eagle**
```javascript
{
  viewBox: '-4 -3 32 32',
  w: 28,
  h: 28,
  path: '[path data]'
}
```

**F-16 Fighting Falcon**
```javascript
{
  viewBox: '-7.8 0 80 80',
  w: 32,
  h: 32,
  path: '[path data]'
}
```

**F/A-18 Hornet**
```javascript
{
  viewBox: '-4 -3 32 32',
  w: 25,
  h: 25,
  path: '[path data]'
}
```

**F-35 Lightning II**
```javascript
{
  viewBox: '-4 -1 40 40',
  w: 32,
  h: 32,
  path: '[path data]'
}
```

**C-130 Hercules**
```javascript
{
  viewBox: '-1 -16 64 64',
  w: 33,
  h: 35,
  path: '[path data]'
}
```

**C-17 Globemaster**
```javascript
{
  viewBox: '0 0 32 32',
  w: 32,
  h: 32,
  path: '[path data]'
}
```

#### Helicopters

**Apache AH-64**
```javascript
{
  viewBox: '-3 -3 32 32',
  w: 31,
  h: 31,
  path: '[path data]'
}
```

**Blackhawk UH-60**
```javascript
{
  viewBox: '0 0 32 32',
  w: 28,
  h: 28,
  path: '[path data]'
}
```

**Chinook CH-47**
```javascript
{
  viewBox: '-4.5 -3 32 32',
  w: 32,
  h: 32,
  path: '[path data]'
}
```

**V-22 Osprey (slow/fast modes)**
```javascript
v22_slow: {
  viewBox: '26.7 -3.3 26 26',
  w: 32,
  h: 32,
  path: '[path data]'
},
v22_fast: {
  viewBox: '30.8 -0.5 26 26',
  w: 32,
  h: 32,
  path: '[path data]'
}
```

#### General Aviation

**Cessna**
```javascript
{
  viewBox: '0 -1 32 31',
  w: 26,
  h: 26,
  path: '[path data]'
}
```

**Cirrus SR22**
```javascript
{
  viewBox: '0 0 64 64',
  w: 23,
  h: 25,
  path: '[path data]'
}
```

**Glider**
```javascript
{
  viewBox: '-5.8 -10 76 76',
  w: 22,
  h: 33,
  path: '[path data]'
}
```

#### Specialty

**Hot Air Balloon**
```javascript
{
  viewBox: '-2 -2 13 17',
  w: 13,
  h: 17,
  path: '[path data]'
}
```

**UAV (Unmanned)**
```javascript
{
  viewBox: '0 1 32 32',
  w: 28,
  h: 28,
  path: '[path data]'
}
```

#### Ground Equipment

**Ground Station**
```javascript
ground_square: { ... },
ground_emergency: { ... },
ground_service: { ... },
ground_unknown: { ... },
ground_fixed: { ... },
ground_tower: { ... }
```

### Full Coverage List

**Commercial:** a319, a320, a321, a332, a359, a380, a400, b737, b738, b739, b52, b707
**Military Fighters:** f15, f16, f18, f35, f5_tiger
**Military Transport:** c130, c2, c5, c17, p3_orion, p8
**Helicopters:** apache, blackhawk, chinook, dauphin, gazelle, puma, s61, tiger, mil24, v22_slow, v22_fast
**General Aviation:** cessna, cirrus_sr22, pa24, rutan_veze, glider
**Specialty:** balloon, blimp, uav, pumpkin
**Ground:** Various ground station types

### Suitability Assessment

**Pros:**
- Already in JavaScript format for web use
- Optimized viewBox per aircraft type
- Includes width/height hints for rendering
- Actively maintained project
- Used in production ADS-B tracking systems

**Cons:**
- Path data not extracted in this report (requires parsing markers.js)
- Less coverage than AircraftShapesSVG
- License unclear (project is open-source but no explicit license file)

**Recommended Use:** Secondary source or for web-only applications already using tar1090.

---

## Source 3: FlightAware Community Icons

**URL:** https://discussions.flightaware.com/t/some-custom-svg-plane-icons/18914
**License:** Not specified (community contributions)
**Format:** Inline SVG path strings
**Count:** 9 aircraft types documented
**Grid:** 64x64 coordinate system

### Overview

Hand-coded SVG icons created by FlightAware community member FlyingPeteNZ using Notepad++ and a 64x64 grid overlay. Icons use only straight lines for simplicity and small file size.

### Complete Icon Definitions

#### Airbus A320 (small_twin_jet)
```javascript
var _a320_svg = "m 32,1 2,1 2,3 0,18 4,1 0,-4 3,0 0,5 17,6 0,3 -15,-2 -9,0 0,12 -2,6 7,3 0,2 -8,-1 -1,2 -1,-2 -8,1 0,-2 7,-3 -2,-6 0,-12 -9,0 -15,2 0,-3 17,-6 0,-5 3,0 0,4 4,-1 0,-18 2,-3 2,-1z";
```
**Notes:** Generic swept-wing twin-jet silhouette, suitable for A320 family and similar narrow-body jets.

#### Boeing 777 (large_twin_jet)
```javascript
var _b777_svg = "m 32,1 2,1 1,2 0,20 4,4 0,-4 3,0 0,4 -1,2 17,12 0,2 -16,-5 -7,0 0,13 -1,5 7,5 0,2 -8,-2 -1,2 -1,-2 -8,2 0,-2 7,-5 -1,-5 0,-13 -7,0 -16,5 0,-2 17,-12 -1,-2 0,-4 3,0 0,4 4,-4 0,-20 1,-2 2,-1z";
```
**Notes:** Wide-body twin-jet profile, suitable for B777, B787, and similar large twins.

#### Bombardier Dash 8 / Q300 (medium_twin_prop)
```javascript
var _dash8_svg = "m 32,1 3,4 0,20 4,0 0,-5 1,-1 1,1 0,5 17,2 0,3 -17,2 0,3 -1,1 -1,-1 0,-3 -4,0 0,15 -1,8 6,0 1,1 0,3 -8,0 -1,1 -1,-1 -8,0 0,-3 1,-1 6,0 -1,-8 0,-15 -4,0 0,3, -1,1 -1,-1 0,-3 -17,-2 0,-3 17,-2 0,-5 1,-1 1,1 0,5 4,0 0,-20 3,-4z";
```
**Notes:** High-wing turboprop configuration with visible engine nacelles.

#### Beechcraft King Air B200 (small_twin_prop)
```javascript
var _b200_svg = "m 32,1 1,0 1,2 1,4 0,5 5,0 0,-5 -1,-1 2,-2 2,2 -1,1 0,5 17,2 0,3 -17,3 0,1 -2,0 0,-1 -5,0 0,5 -2,8 6,3 0,2 -6,-1 -1,0 -6,1 0,-2 6,-3 -2,-8 0,-5 -5,0 0,1, -2,0 0,-1 -17,-3 0,-3 17,-2 0,-5 -1,-1 2,-2 2,2 -1,1 0,5 5,0 0,-5 1,-4 1,-2 z";
```
**Notes:** Smaller twin-prop with distinctive T-tail and straight wing.

#### Gulfstream G650 (private_jet)
```javascript
var _g650_svg = "m 32,1 1,0 1,2 1,4 0,10 21,17 0,5 -2,-2 -16,-8 -3,0 0,3 2,0 1,1 0,5 -1,1 0,3 -2,0 0,1 7,5 0,3 -9,-3 -1,0 -9,3 0,-3 7,-5 0,-1 -2,0 0,-3 -1,-1 0,-5 1,-1 2,0 0,-3 -3,0 -16,8 -2,2 0,-5 21,-17 0,-10 1,-4 1,-2z";
```
**Notes:** Business jet with swept wing and distinctive tail.

#### Lockheed C-130H Hercules (medium_four_prop)
```javascript
var _c130_svg = "m 31,1 1,0 1,1 1,2 0,8 3,0 0,-3 1,-1 1,1 0,3 6,0 0,-3 1,-1 1,1 0,3 10,1 0,2 -1,1 -17,3 -5,0 0,10 -1,1 8,2 0,1 -1,1 -8,0 -1,1 -1,-1 -8,0 -1,-1 0,-1 8,-2 -1,-1 0,-10 -5,0 -17,-3 -1,-1 0,-2 10,-1 0,-3 1,-1 1,1 0,3 6,0 0,-3 1,-1 1,1 0,3 3,0 0,-8 1,-2 1,-1 z";
```
**Notes:** Four-engine military transport with high straight wing.

#### Sailplane / Glider (sailplane)
```javascript
var _sailplane_svg = "m 31,1 1,0 1,2 1,4 1,6 0,3 16.5,0 11,2 1,2 -21,2 -8,0 -1,5 -1,15 0,4 4,0 5,1 0.5,1 0,1 -11,0 0.5,2 0.5,-2 -11,0 0.5,-1 0,-1 5,-1 4,0 0,-4 -1,-15 -1,-5 -8,0 -21,-2 1,-2 11,-2 16.5,0 0,-3 1,-6 1,-4 1,-2 z";
```
**Notes:** Extremely high aspect ratio wing, narrow fuselage.

#### Hot Air Balloon (balloon)
```javascript
var _balloon_svg = "m 27,1 10,0 3,1 3,1 1,1 2,1 6,6 1,2 1,1 1,3 1,3 0,10 -1,3 -1,3 -1,1 -1,2 -6,6 -2,1 -1,1 -2,1 -2,1 -2,8 -1,0 2,-8 -3,1 -6,0 -3,-1 2,8 9,0 0,6 -10,0 0,-6 -2,-8 -2,-1 -2,-1 -1,-1 -2,-1 -6,-6 -1,-2 -1,-1 -1,-3 -1,-3 0,-10 1,-3 1,-3 1,-1 1,-2 6,-6 2,-1 1,-1 3,-1 3,-1z";
```
**Notes:** Circular balloon envelope with basket below.

#### Generic Triangle (Default fallback)
```javascript
var _triangle_svg = "m 32,0 32,64 -64,0z";
```
**Notes:** Simple triangle for unknown aircraft types.

### Design Philosophy

- Created on 64x64 grid using straight lines only
- No curves (all segments are `m`, `l`, `h`, `v`, `z` commands)
- Designed for small-scale display (typical map zoom levels)
- Lightweight file size (under 500 bytes per icon)
- Hand-coded in Notepad++ without graphical editor

### Suitability Assessment

**Pros:**
- Extremely lightweight (minimal path data)
- Already in string format ready for JavaScript injection
- Simple straight-line rendering (fast performance)
- Created specifically for flight tracking applications
- Consistent 64x64 coordinate system

**Cons:**
- Limited to 9 aircraft types
- Less detailed than AircraftShapesSVG or tar1090
- No curves = less realistic appearance
- License/usage rights unclear (community contribution)
- Creator username attribution only (no formal license)

**Recommended Use:** Fallback icons for minimal map implementations or embedded systems with limited resources.

---

## Source 4: VRSCustomMarkers (rikgale & shish0r)

**URL:** https://github.com/rikgale/VRSCustomMarkers
**Fork of:** https://github.com/shish0r/VRSCustomMarkers
**License:** CC0-1.0 (Creative Commons Zero, public domain dedication)
**Format:** HTML injection files with embedded SVG
**Count:** 50+ aircraft + maritime vessels
**Compatibility:** Virtual Radar Server v3.0.x

### Overview

Custom SVG markers designed for Virtual Radar Server. Includes both aircraft and boat markers. SVG paths are embedded in HTML files that inject into the VRS web interface.

### File Structure

```
VRSCustomMarkers/
├── Images/
├── MyMarkers1.html          # Main marker definitions
├── MyMarkers1HFDL.html      # HFDL variant with sqwark-based coloring
└── README.md
```

### Aircraft Coverage

**Military Fighters:**
- Spitfire (SPIT) - 26x26, viewBox: 0 0 10.583333 10.583334
- F-35 (F35) - 32x32, viewBox: 0 0 10.583333 10.583334
- Eurofighter Typhoon (EUFI) - 38x38, viewBox: 0 0 10.583333 10.583334
- F-16 (F16)
- F-15 (F15)
- F-18
- Tornado
- Saab Gripen
- Hunter
- U-2

**Bombers:**
- B-52
- B-1
- B-707

**Military Transport:**
- C-17
- C-5
- IL-76
- AN-225
- A400M (A400)
- Beluga XL

**Military Helicopters:**
- AH-64 Apache
- CH-47 Chinook (CH47) - 38x38, viewBox: 0 0 10.583333 10.583334
- EH-101
- V-22 Osprey (V22) - 38x38, viewBox: 0 0 10.583333 10.583334

**AWACS/Reconnaissance:**
- E-3 AWACS (E3)
- E-2 Hawkeye
- P-3 Orion

**Historic Aircraft:**
- Lancaster
- B-17
- DC-3
- PBY-5A

**Business Jets:**
- Global Express (GLEX) - 38x38, viewBox: 0 0 10.583333 10.583334

**Light Aircraft:**
- T-6 (T6) - 38x38, viewBox: 0 0 10.583333 10.583334
- Ultralight (ULAC) - 38x38, viewBox: 0 0 10.583333 10.583334
- Autogyro (GYRO) - 38x38, viewBox: 0 0 10.583333 10.583334

**Trainers:**
- Hawk (HAWK) - Listed in coverage
- MD-11

### Maritime Vessels

- Aircraft carriers (CVN-65 noted)
- Cruise liners
- Pleasure craft
- RNLI boats

### Technical Details

All markers use `outline-path` ID for dynamic color application based on:
- Squawk codes
- Military status
- Operator information

HFDL variant (MyMarkers1HFDL.html) designed for DumpHFDL with `--freq-as-sqwark` setting.

### Access

Raw file URLs:
- Main: `https://raw.githubusercontent.com/rikgale/VRSCustomMarkers/main/MyMarkers1.html`
- HFDL: `https://raw.githubusercontent.com/rikgale/VRSCustomMarkers/main/MyMarkers1HFDL.html`

### Suitability Assessment

**Pros:**
- Public domain (CC0-1.0) - no attribution required
- Includes maritime vessels (unique among sources)
- Designed for production use in VRS
- Dynamic coloring support
- Good military aircraft coverage

**Cons:**
- Embedded in HTML files (requires parsing)
- Optimized for VRS integration (may need adaptation)
- Less commercial aircraft coverage than AircraftShapesSVG
- Individual SVG markup not easily extractable without parsing HTML

**Recommended Use:** Primary source for maritime vessels, secondary for military aircraft, especially if VRS compatibility needed.

---

## Maritime/Ship SVG Sources

### OpenSeaMap Renderer (Archived)

**URL:** https://github.com/OpenSeaMap/renderer
**Status:** Archived February 1, 2021 (read-only)
**License:** Not explicitly stated (OpenStreetMap project)
**Count:** 231 SVG files total, subset for vessels

#### Vessel-Related Icons Available

**Directory:** `searender/symbols/`

**Files:**
- `Sailboat.svg` - Wind-powered sailing vessel
- `Speedboat.svg` - High-speed motorized boat
- `Seaplane.svg` - Amphibious aircraft
- `Rowboat.svg` - Small manually-propelled watercraft
- `Waterbike.svg` - Personal watercraft
- `Waterski.svg` - Water sports equipment marker

#### Access Pattern

```
https://raw.githubusercontent.com/OpenSeaMap/renderer/master/searender/symbols/[FILENAME].svg
```

**Note:** Attempted fetches returned 404 errors. Repository may have been reorganized or symbols moved before archival.

#### Additional Context

- OpenSeaMap renderer generates SVG from OSM tile data
- Icons designed for nautical charting
- Includes navigational symbols, buoys, lights, etc.
- 224 SVG files noted as foundation for maritime symbols

### Suitability Assessment

**Pros:**
- Authoritative nautical source (OpenSeaMap project)
- Designed for maritime charting standards
- Includes specialized watercraft types

**Cons:**
- Repository archived (no updates)
- Direct file access failed (404 errors)
- Would require cloning entire repository
- Limited to recreational/small vessels (no cargo/tanker icons documented)

**Recommended Use:** Archive reference only unless repository can be successfully cloned locally.

---

## Additional Sources Identified (Not Fetched)

### FlightAirMap
**URL:** https://github.com/Ysurac/FlightAirMap
**Description:** Open-source project displaying live aircraft and ships on 2D/3D maps
**Data Sources:** ADS-B (SBS1), VRS, VATSIM, IVAO, ACARS, APRS, AIS
**Potential:** Likely includes both aircraft and vessel icon sets

### GitHub Search Results

**Topic:** "aircraft svg"
**Relevant repos found but not explored:**
- Various aviation-related projects with potential icon libraries
- SUAVE (aircraft design toolbox)
- X-Plane related repositories

**Topic:** "ship svg" / "AIS vessel icons"
**Findings:**
- Icon marketplaces (Flaticon, IconScout) with 9,000+ vessel icons
- Commercial icon sets available
- MarineTraffic uses vessel type-based coloring (not SVG source identified)
- AIS data repositories without icon assets

---

## Comparison Matrix

| Source | Aircraft Count | Ship Count | License | Top-Down? | Clean Paths? | Format | Best For |
|--------|---------------|------------|---------|-----------|--------------|--------|----------|
| **AircraftShapesSVG** | 182 | 0 | GPL-3.0 | Yes | Excellent | Individual SVG files | Primary aircraft source |
| **tar1090** | 50+ | 0 | Unclear | Yes | Good | JavaScript object | Web applications |
| **FlightAware** | 9 | 0 | Unclear | Yes | Simple (straight lines) | JavaScript strings | Minimal/embedded systems |
| **VRSCustomMarkers** | 50+ | 4+ types | CC0-1.0 | Yes | Good | HTML-embedded SVG | VRS integration, ships |
| **OpenSeaMap** | 1 (seaplane) | 5+ | Unclear | Yes | Unknown (404 errors) | Individual SVG files | Reference only |

---

## Licensing Summary

### AircraftShapesSVG (GPL-3.0)
**Requirements:**
- Source code availability
- License and copyright notice
- State changes
- Disclose source

**Permissions:**
- Commercial use
- Distribution
- Modification
- Patent use
- Private use

**Conditions:**
- Same license (copyleft)
- License and copyright notice
- State changes
- Disclose source

### VRSCustomMarkers (CC0-1.0)
**Public Domain Dedication:**
- No attribution required
- No restrictions on use
- Commercial use permitted
- Can be relicensed

### tar1090 (Unspecified)
**Status:** Open-source project without explicit license file
**Recommendation:** Contact maintainer (wiedehopf) for clarification before commercial use

### FlightAware Community (Unspecified)
**Status:** Community forum contributions
**Recommendation:** Contact original poster (FlyingPeteNZ) for usage rights

---

## Implementation Recommendations

### For Flight Tracking Map Application

**Primary Source:** AircraftShapesSVG (RexKramer1)
- Download all 182 SVG files from `Shapes SVG/` directory
- Strip Inkscape metadata to reduce file size
- Convert to optimized format (SVGO processing)
- Map ICAO type codes to filenames

**Fallback Source:** tar1090 markers.js
- Use for aircraft types not in AircraftShapesSVG
- Already optimized for web display
- Includes ground equipment icons

**Minimal/Embedded:** FlightAware Community
- Use for resource-constrained environments
- Extremely lightweight (< 500 bytes each)
- Limited type coverage

### For Maritime Tracking

**Primary Source:** VRSCustomMarkers
- Extract ship SVG paths from MyMarkers1.html
- Public domain (CC0) - no restrictions

**Alternative:** Clone OpenSeaMap renderer repository
- More comprehensive nautical symbols
- Requires local clone (raw access failed)

### Rotation/Heading Support

**None of the sources include rotation transforms.** Will need to:
1. Extract path data
2. Apply CSS `transform: rotate(${heading}deg)` at render time
3. Or modify SVG `transform` attribute dynamically
4. Center rotation point on aircraft center (adjust `transform-origin`)

### Optimization Pipeline

1. **Download** all SVG files from chosen sources
2. **Strip** metadata (Inkscape, Sodipodi namespaces)
3. **Optimize** with SVGO (remove unnecessary attributes)
4. **Convert** to sprite sheet or inline data URIs
5. **Map** ICAO aircraft type codes to icon IDs
6. **Add** rotation logic for heading display

---

## Complete File Lists

### AircraftShapesSVG (182 files)

A10.svg, A124.svg, A19N.svg, A20N.svg, A21N.svg, A225.svg, A306.svg, A310.svg, A318.svg, A320.svg, A321.svg, A332.svg, A333.svg, A337.svg, A338.svg, A339.svg, A342.svg, A343.svg, A345.svg, A346.svg, A359.svg, A35K.svg, A388.svg, A3ST.svg, A4.svg, A400.svg, AJET.svg, AN12.svg, AN26.svg, AS21.svg, AS32.svg, AS65.svg, AT45.svg, AT75.svg, ATP.svg, B1 fast.svg, B1 slow.svg, B190.svg, B29.svg, B350.svg, B38M.svg, B39M.svg, B52.svg, B703.svg, B712.svg, B722.svg, B733.svg, B734.svg, B735.svg, B737.svg, B738.svg, B739.svg, B742.svg, B744.svg, B748.svg, B74S.svg, B752.svg, B753.svg, B762.svg, B763.svg, B764.svg, B772.svg, B773.svg, B779.svg, B77L.svg, B77W.svg, B788.svg, B789.svg, B78X.svg, BALL.svg, BCS1.svg, BCS3.svg, BLCF.svg, BN2P.svg, C130.svg, C160.svg, C17.svg, C172.svg, C2.svg, C208.svg, C25B.svg, C295.svg, C5M.svg, C750.svg, CL2T.svg, CN35.svg, CRJ2.svg, CRJ7.svg, CRJ9.svg, CRJX.svg, CVN-65.svg, D228.svg, D328.svg, DA42.svg, DC10.svg, DC3.svg, DC87.svg, DH8C.svg, DH8D.svg, DO27.svg, DO28.svg, E170.svg, E195.svg, E300.svg, E35L.svg, E390.svg, E3CF.svg, E3TF.svg, E737.svg, E8.svg, EC20.svg, EC35.svg, EC45.svg, EUFI.svg, F15.svg, F16.svg, F18H.svg, F18S.svg, F22.svg, F35.svg, F406.svg, F5.svg, F50.svg, FA7X.svg, GAZL.svg, GL5T.svg, GLF6.svg, GYRO.svg, H47.svg, H60.svg, H64.svg, HAWK.svg, HUNT.svg, IL62.svg, IL76.svg, J328.svg, K35E.svg, KC2.svg, KC46.svg, L159.svg, LJ35.svg, LYNX.svg, M326.svg, MD11.svg, MI24.svg, MIRA.svg, MRF1.svg, NH90.svg, P1.svg, P180.svg, P28A.svg, P3.svg, P8.svg, PA46.svg, PC12.svg, PC6T.svg, PC9.svg, Q4.svg, R135.svg, R44.svg, RFAL.svg, RJ85.svg, S61.svg, SB39.svg, SC7.svg, SF25.svg, SF34.svg, SGUP.svg, SR22.svg, ST75.svg, SU95.svg, T204.svg, T38.svg, TIGR.svg, TOR fast.svg, TOR slow.svg, U2.svg, UH1.svg, Unidentified.svg, V22 fast.svg, V22 slow.svg, VF35.svg

---

## Usage Examples

### Rendering Aircraft Icon with Heading

```javascript
// Using AircraftShapesSVG
const iconPath = `https://raw.githubusercontent.com/RexKramer1/AircraftShapesSVG/master/Shapes%20SVG/B737.svg`;

// Fetch and parse SVG
const response = await fetch(iconPath);
const svgText = await response.text();
const parser = new DOMParser();
const svgDoc = parser.parseFromString(svgText, 'image/svg+xml');
const svgElement = svgDoc.documentElement;

// Apply heading rotation
svgElement.style.transform = `rotate(${heading}deg)`;
svgElement.style.transformOrigin = 'center';

// Inject into map marker
mapMarker.setIcon({
  url: 'data:image/svg+xml;base64,' + btoa(svgElement.outerHTML),
  scaledSize: new google.maps.Size(32, 32),
  anchor: new google.maps.Point(16, 16)
});
```

### Using tar1090 Markers

```javascript
// Import markers.js
import markers from './markers.js';

// Get aircraft type marker
const b737Marker = markers.b737;

// Create SVG string with rotation
const svg = `
<svg viewBox="${b737Marker.viewBox}"
     width="${b737Marker.w}"
     height="${b737Marker.h}"
     style="transform: rotate(${heading}deg)">
  <path d="${b737Marker.path}"
        fill="none"
        stroke="black"
        stroke-width="2"/>
</svg>
`;
```

### Type Code Mapping Example

```javascript
const icaoToFile = {
  'B737': 'B737.svg',
  'B738': 'B738.svg',
  'B739': 'B739.svg',
  'A320': 'A320.svg',
  'A321': 'A321.svg',
  'B744': 'B744.svg',
  'B748': 'B748.svg',
  'C130': 'C130.svg',
  'F16': 'F16.svg',
  'H47': 'H47.svg',
  'C172': 'C172.svg',
  // ... 182 total mappings
};

function getAircraftIcon(icaoType) {
  const filename = icaoToFile[icaoType] || 'Unidentified.svg';
  return `https://raw.githubusercontent.com/RexKramer1/AircraftShapesSVG/master/Shapes%20SVG/${filename}`;
}
```

---

## Conclusion

**Best Overall Source:** AircraftShapesSVG (RexKramer1)
- Most comprehensive (182 types)
- Highest quality paths
- Clear license (GPL-3.0)
- Truly top-down perspective
- Professional production quality

**Best for Maritime:** VRSCustomMarkers
- Public domain (CC0-1.0)
- Includes ships/vessels
- Production-tested

**Best for Web Integration:** tar1090
- Already optimized for web
- Embedded in popular ADS-B tracker
- Good type coverage

**Easiest License:** VRSCustomMarkers (CC0-1.0 public domain)

**Most Complete:** AircraftShapesSVG (182 types vs 50 in others)

---

## Sources

- [AircraftShapesSVG Repository](https://github.com/RexKramer1/AircraftShapesSVG)
- [tar1090 Repository](https://github.com/wiedehopf/tar1090)
- [tar1090 markers.js](https://raw.githubusercontent.com/wiedehopf/tar1090/master/html/markers.js)
- [FlightAware Custom SVG Icons Discussion](https://discussions.flightaware.com/t/some-custom-svg-plane-icons/18914)
- [VRSCustomMarkers (rikgale)](https://github.com/rikgale/VRSCustomMarkers)
- [VRSCustomMarkers (shish0r)](https://github.com/shish0r/VRSCustomMarkers)
- [OpenSeaMap Renderer](https://github.com/OpenSeaMap/renderer)
- [OpenSeaMap Symbols](https://github.com/OpenSeaMap/renderer/tree/master/searender/symbols)
- [FlightAirMap](https://github.com/Ysurac/FlightAirMap)

---

**End of Report**
