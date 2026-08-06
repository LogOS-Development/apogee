# Design: Solar System Visualizer — Earth Atmosphere View

## Goal

Build the foundation of a physics-faithful solar-system scene in Godot. The first focus is Earth with a realistic atmosphere view: surface texture, day/night lighting driven by the Sun, and clouds that respond to simulated density, wind, solar flux (F10.7), and geomagnetic activity (Ap). The Sun is a real object in the scene, not just a light direction. The Moon will be added next.

This document intentionally scopes Phase 1 to Earth + Sun so that later bodies (Moon, spacecraft, planets) can slot into the same coordinate system and lighting model.

## Long-term architecture

```
SolarSystemRoot (Node3D)
├── Sun (MeshInstance3D, emissive)
│   └── SunLight (DirectionalLight3D)  // lights Earth
├── Earth (Node3D)
│   ├── Surface (MeshInstance3D, textured, lit by Sun)
│   ├── CloudShell (MeshInstance3D, translucent)
│   ├── AtmosphereGridSampler (Rust GDExtension)
│   ├── DensityMultiMesh (wind/density particles)
│   └── WindMultiMesh (arrow vectors)
└── CameraRig (Camera3D + orbit controls)
```

All positions are in a single canonical frame: meters, J2000-equatorial orientation, with the barycenter near the Sun. For the Earth-atmosphere close-up, we scale the view so 1 Godot unit = some convenient factor (e.g. 1 unit = 1000 km) and place the camera near Earth.

## Scope

### In scope (Phase 1)

- Replace the flat-blue Earth placeholder with a textured, lit sphere.
- Add the Sun as a visible emissive sphere that illuminates Earth.
- Compute true Sun direction from simulated time (DOY, UTC) using a simple ephemeris.
- Make Earth rotate and tilt on its axis; the terminator moves correctly with UTC.
- Add a cloud layer whose coverage and movement respond to density and wind.
- Keep the existing density spheres and wind arrows as child layers of Earth.
- Maintain camera orbit/zoom/pan controls from the current atmosphere scene.

### Out of scope (Phase 1)

- Moon and other planets.
- Full JPL ephemerides; we use a low-fidelity analytic model sufficient for visuals.
- Real satellite imagery assets (a procedural fallback is acceptable).
- Multi-scattering atmospheric light transport.
- Shadows cast by Earth onto its own atmosphere (single-directional Sun light is enough for now).

## Physics model

### Units and frame

- Simulation time: `day_of_year` (1–366) and `seconds_utc` (0–86400).
- Earth radius: `R_E = 6371 km`.
- Astronomical unit: `AU = 149_597_870.7 km`.
- For the close-up atmosphere view we use a scaled scene: Earth radius = 6.371 Godot units (1 unit = 1000 km). The Sun is placed at `1 AU / 1000 km ≈ 149_597.87` units away and scaled visually so it remains visible. Alternatively, we use a symbolic Sun at a much nearer distance for lighting, while keeping a separate true-position marker.

### Sun direction

For visual fidelity we need the unit vector from Earth to Sun.

Low-fidelity analytic model (good to ~1°):

```
// Julian Day from DOY/UTC, assume non-leap-year for visuals
JD = 2451545.0 + day_of_year - 1 + seconds_utc / 86400.0

// Days since J2000.0
D = JD - 2451545.0

// Mean longitude of the Sun
L = (280.460 + 0.9856474 * D) mod 360

// Mean anomaly of the Sun
g = (357.528 + 0.9856003 * D) mod 360

// Ecliptic longitude of the Sun
lambda = L + 1.915 * sin(g) + 0.020 * sin(2g)

// Obliquity of the ecliptic
epsilon = 23.439 - 0.0000004 * D

// Sun unit vector in geocentric equatorial J2000 frame
X = cos(lambda)
Y = cos(epsilon) * sin(lambda)
Z = sin(epsilon) * sin(lambda)
sun_direction = normalize(Vector3(X, Z, -Y))   // matches current visualizer frame
```

This gives us the Sun's direction as seen from Earth. For Phase 1 we place the Sun visually along this vector at a chosen distance.

### Earth orientation

- Axial tilt: 23.5° from ecliptic pole.
- Earth rotation angle (GMST): `theta = 280.46061837 + 360.98564736629 * D` degrees.
- We apply rotation to the Earth mesh so the texture rotates with UTC.
- The cloud shell rotates with Earth (co-rotating clouds are not wanted, so wind advection happens in lat/lon space).

## Visual targets

1. **Sun**: large emissive sphere with a bloom glow, positioned along the true Sun direction.
2. **Earth surface**: oceans, continents, ice caps; night side dark with optional city lights.
3. **Day/night terminator**: sharp but antialiased by shader, driven by Sun direction.
4. **Clouds**: translucent shell above the surface; coverage tied to density and solar/geomagnetic inputs; advected by horizontal wind.
5. **Atmosphere particles**: existing density spheres and wind arrows as child layers.

## Data sources already available

`AtmosphereGridSampler` produces per-grid-cell:

- `altitude_km`
- `latitude_rad`, `longitude_rad`
- `density_kg_m3`
- `temperature_k`
- `east_mps`, `north_mps`, `up_mps`

Node-level parameters:

- `day_of_year`
- `seconds_utc`
- `f107`, `f107a`
- `ap`

These feed the visuals directly.

## Proposed implementation

### 1. Scene refactor: Earth as a node

Rename and restructure `AtmosphereWindVisualizer` so the Earth-related objects are grouped under an `Earth` node:

```
AtmosphereWindVisualizer (Node3D)
├── Sun
│   ├── MeshInstance3D (emissive sphere)
│   └── DirectionalLight3D (casts light on Earth)
├── Earth
│   ├── Surface
│   ├── CloudShell
│   ├── AtmosphereGridSampler
│   ├── DensityMultiMesh
│   └── WindMultiMesh
├── Camera3D
└── UI
```

The root script still owns simulation time and camera controls. The `Earth` node holds transforms for rotation/tilt.

### 2. Sun shader and light

Sun material:

- `ShaderMaterial` with emissive color and a radial glow via fresnel.
- Billboard or large sphere; no texture needed initially.

Sun light:

- `DirectionalLight3D` child of Sun, aimed at Earth.
- Since Sun distance is huge, directional light is the correct approximation.
- The light direction equals `-sun_direction`.

### 3. Earth surface shader

Inputs:

- `sun_direction`: vec3 (Earth → Sun, world space).
- `texture_map`: sampler2D.
- `night_lights`: sampler2D (optional).

Fragment logic:

- `NdotL = max(dot(normal, sun_direction), 0.0)`.
- Day/night mix with smoothstep terminator.
- Night side shows city lights texture if available, otherwise dark blue.
- Rim glow on lit limb using view fresnel.

### 4. Earth rotation and orientation

The Earth's local transform is updated each frame:

```gdscript
_earth_node.rotation_degrees.x = 23.5   // axial tilt
_earth_node.rotation_degrees.y = gmst   // rotation about tilted axis
```

For Phase 1 we keep it simple: a single tilt and rotate around Y. Later we use a proper obliquity + precession model.

### 5. Cloud layer

A `MeshInstance3D` sphere slightly larger than the surface (`radius * 1.015`).

Shader inputs:

- `cloud_texture`: 2D lat/lon texture or 3D texture.
- `sun_direction`: vec3.
- `wind_advection_time`: float.
- `f107`, `ap`.

For Phase 1, bake a 2D lat/lon cloud opacity texture from the grid (average over altitudes, or a selected altitude slice). The Rust side can expose `bake_cloud_texture(width, height)` returning `ImageTexture`.

Cloud shader:

- Convert world position to lat/lon.
- Sample opacity and wind vector.
- Advect longitude by `wind_east * advection_time`.
- Output premultiplied alpha cloud color, brighter on day side.

### 6. Solar/geomagnetic response mapping

- `f107` → overall cloud brightness and upper-atmosphere glow.
- `ap` → high-latitude aurora intensity (stub: a faint greenish ring).
- `density` → local cloud opacity.
- `day_of_year` → subsolar latitude and ice-cap extent.

## Implementation plan

### PR A: Solar-system skeleton — Sun and Earth

Files:

- `godot/scenes/atmosphere_wind_visualizer.tscn`
- `godot/scripts/atmosphere_wind_visualizer.gd`
- new: `godot/shaders/sun_surface.gdshader`
- new: `godot/shaders/earth_surface.gdshader`

Tasks:

1. Restructure scene: add `Sun` and `Earth` parent nodes.
2. Implement analytic Sun direction from DOY/UTC.
3. Add emissive Sun sphere and directional light aimed at Earth.
4. Replace Earth placeholder with shader-based textured globe.
5. Add Earth rotation/tilt.

### PR B: Clouds from atmosphere data

Files:

- `godot/scenes/atmosphere_wind_visualizer.tscn`
- `godot/scripts/atmosphere_wind_visualizer.gd`
- new: `godot/shaders/cloud_shell.gdshader`
- `crates/apogee-godot/src/atmosphere_visualizer.rs`

Tasks:

1. Add `CloudShell` sphere under Earth.
2. Add Rust helper `bake_cloud_texture(width, height)` using current grid.
3. Upload texture and update shader uniforms each resample.
4. Cloud shader: lat/lon sampling, wind advection, day-side brightness.

### PR C: Solar/geomagnetic visual response

Files:

- `godot/scripts/atmosphere_wind_visualizer.gd`
- `godot/shaders/earth_surface.gdshader`
- `godot/shaders/cloud_shell.gdshader`
- new: `godot/shaders/aurora_ring.gdshader`

Tasks:

1. Pass `f107`, `ap`, `day_of_year`, `seconds_utc` to shaders.
2. Add aurora ring at high latitudes modulated by `ap`.
3. Tie cloud brightness and coverage to `f107` and `ap`.

## Moon preview (Phase 2)

- Add `Moon` node under `SolarSystemRoot`.
- Use simplified lunar ephemeris (mean longitude, inclination, eccentricity).
- Visual scale: Moon radius ≈ 0.273 Earth radii.
- Tide-locked rotation.
- This only requires the same `Sun` directional light; no new light source.

## Open questions

1. Do we ship a small Earth texture asset, or generate a procedural lat/lon color map in the shader?
2. Do we use a real 1-AU Sun distance with a scaled visual proxy, or keep the Sun nearby for lighting and mark true direction separately?
3. Should clouds be a 2D lat/lon shell or a full 3D shell with altitude variation?
4. How often do we rebake the cloud texture during playback (every frame, every sim hour, on demand)?
5. Do we want city lights on the night side? Requires a texture asset or procedural dot mask.
6. Should we expose the ephemeris computation in Rust (reusable by other Apogee systems) or keep it in GDScript for now?

## Appendix: coordinate frame

The current visualizer maps geodetic coordinates to world space as:

```gdscript
x = r * cos(lat) * cos(lon)
y = r * cos(lat) * sin(lon)
z = r * sin(lat)
return Vector3(x, z, -y)
```

For the solar-system scene we keep this convention and add the Sun direction in the same frame.

## Appendix: recommended default scales

| Object | Real radius | Scaled radius (1 unit = 1000 km) |
|--------|-------------|----------------------------------|
| Earth  | 6371 km     | 6.371 units                      |
| Sun    | 696_340 km  | 696.34 units                     |
| 1 AU   | 149_597_870.7 km | 149_597.87 units            |

For the atmosphere close-up we place the camera ~10–20 units from Earth. The Sun is too far to see at true scale, so we render a symbolic Sun at ~1000 units along the true direction while the directional light comes from the true vector.
