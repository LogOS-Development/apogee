Apogee — Development Work Plan
Tech Stack (Locked)
Layer	Technology	Role
Simulation Core	Rust + Bevy ECS	Headless server, authoritative physics
Numerics	nalgebra, odeint, spice-rs	Math, integration, ephemeris
Networking	Quinn (QUIC) + FlatBuffers	Transport + serialization
Client Renderer	Godot 4.x (GDExtension bridge)	Visuals, input, UI
Bridge	Rust → Godot via GDExtension	Shared memory or IPC
Persistence	PostgreSQL + TimescaleDB + Redis	State, telemetry, sessions
CI/CD	GitHub Actions + self-hosted runners	Test, build, validate
Infra	Docker + Kubernetes (later)	Shard orchestration
Phase 0: Foundation (Weeks 1–4)
0.1 Project Scaffolding
apogee/
├── crates/
│   ├── apogee-core/          # Simulation engine (no I/O, no rendering)
│   │   ├── src/
│   │   │   ├── components/   # ECS component definitions
│   │   │   ├── systems/      # ECS systems (force agg, integrator, etc.)
│   │   │   ├── ephemeris/     # JPL kernel loading + Chebyshev eval
│   │   │   ├── frames/       # Frame transformation service
│   │   │   ├── gravity/      # Spherical harmonics, N-body
│   │   │   ├── aero/         # Aerodynamic models
│   │   │   ├── integrator/   # Multi-rate integration
│   │   │   ├── lib.rs
│   │   │   └── tests/
│   │   └── Cargo.toml
│   ├── apogee-net/           # Networking layer
│   │   ├── src/
│   │   │   ├── server/       # QUIC listener, snapshot builder
│   │   │   ├── client/       # Connection, prediction, reconciliation
│   │   │   ├── protocol/     # FlatBuffer schemas + generated code
│   │   │   └── interest/     # Visibility, bandwidth allocation
│   │   └── Cargo.toml
│   ├── apogee-server/        # Binary: headless sim server
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   ├── apogee-godot/         # GDExtension bridge (cdylib)
│   │   ├── src/
│   │   │   ├── bridge.rs     # State transfer from sim to Godot
│   │   │   ├── nodes/        # Custom Godot nodes
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   └── apogee-common/        # Shared types, constants, error types
│       └── Cargo.toml
├── godot/                    # Godot project files
│   ├── project.godot
│   ├── scenes/
│   ├── shaders/
│   ├── scripts/              # GDScript for UI logic
│   └── addons/
├── schemas/                  # FlatBuffer .fbs definitions
├── data/                     # Ephemeris kernels, gravity models
├── tests/                    # Integration + validation tests
├── ci/                       # CI pipeline configs
└── docker/                   # Container definitions

Deliverable: Compiling workspace, empty crates, CI pipeline running cargo test (even if no tests yet).

Exit criterion: cargo build --workspace succeeds. CI green on PR.
0.2 Data Acquisition Pipeline
Asset	Source	Format	Storage
JPL DE441 ephemeris	NAIF/SPICE	Binary kernel (.bsp)	data/ephemeris/
EGM2008 gravity model	NGA	Spherical harmonic coefficients	data/gravity/
NRLMSISE-00 source code	NRL	Fortran → port to Rust	crates/apogee-core/src/atmosphere/
F10.7 / geomagnetic data	NOAA SWPC / Celestrak	CSV / JSON	data/spaceweather/
Leap second table	IERS	Text file	data/time/
Earth orientation params	IERS	EOP C04 series	data/eop/
Sample TLEs	Celestrak / Space-Track	TLE format	tests/fixtures/

Deliverable: Download script that fetches and validates all data. CI test verifies data integrity (file hashes, format parsing).

Exit criterion: ./scripts/fetch_data.sh populates data/ with validated files. CI test confirms.
0.3 Continuous Integration Foundation
# .github/workflows/ci.yml (conceptual)
jobs:
  test:
    - cargo test --workspace
    - cargo clippy -- -D warnings
    - cargo fmt --check
  
  validation:
    - cargo test --test celestial_validation  # Horizons comparison
    - cargo test --test aero_validation       # Known coefficient check
    - cargo test --test tle_propagation       # TLE decay comparison
  
  build-godot:
    - cd godot && godot --export-release Linux
    - artifact: apogee-client-linux.tar.gz
  
  build-server:
    - cargo build --release -p apogee-server
    - docker build -t apogee-server .

Deliverable: CI runs on every PR. Three test suites: unit, validation, build.

Exit criterion: A trivial PR triggers all jobs successfully.
Phase 1: Core Propagator (Weeks 5–20)
1.1 Time & Frames (Weeks 5–7)

Tasks:
Task	Description	Validation
ClockService	TDB, TAI, UTC, UT1 conversion with leap seconds	GPS clock offset ≈ 38 μs/day
FrameService	ICRF ↔ ECI ↔ ECEF ↔ ECLIPJ2000 transformations	Compare rotation matrix against SPICE toolkit
EarthOrientationParametersLoader	Parse IERS EOP C04	Polar motion values match published data
NutationPrecessionModel	IAU 2000/2006 nutation model	Arcsecond-level accuracy vs SOFA library

Dependencies: 0.2 (data files)

Exit criterion: Given a TDB epoch, can transform a position vector between any two reference frames with < 1 arcsec angular error compared to SPICE.
1.2 Ephemeris Service (Weeks 5–8, parallel with 1.1)

Tasks:
Task	Description	Validation
SPICE Kernel Loader	Parse DE441 binary format, extract Chebyshev coefficients	Segment boundaries match NAIF documentation
Chebyshev Evaluator	Evaluate position/velocity at arbitrary epoch	Mars position vs JPL Horizons API: < 100 km after 1 year
Ephemeris Cache	LRU cache for active segments	Cache hit rate > 90% during steady-state operation
Batch Query	All-body state lookup in single call	< 1ms for 50 bodies with warm cache

Dependencies: 0.2 (kernel files)
// Core interface
pub trait Ephemeris: Send + Sync {
    fn state_at(&self, body: NaifId, epoch: Epoch) -> Result<BodyState>;
    fn all_states_at(&self, epoch: Epoch) -> Result<SolarSystemState>;
    fn bodies(&self) -> &[BodyDescriptor];
}

Exit criterion: TCE-01 passes (Mars position error < 100 km vs Horizons over 1 year).
1.3 Gravity Module (Weeks 8–12)

Tasks:
Task	Description	Validation
Point-mass N-body	Gravitational acceleration from all celestial bodies	Energy/momentum conservation over 1000 orbits
Spherical Harmonics Engine	Cunningham recursion, C/S coefficient evaluation up to degree N	Gravity acceleration matches EGM2008 at known test points
Gravity Gradient Torque	R̂ × I·R̂ formulation	Torque sign and magnitude correct for known spacecraft config
Tesseral/Harmonic Selection	Activate high-degree terms only when close to body	Performance: degree-70 eval < 50μs

Dependencies: 1.2 (ephemeris provides body positions)
pub trait GravityModel: Send + Sync {
    /// Returns acceleration in inertial frame
    fn acceleration(
        &self,
        position: &Vector3<f64>,
        celestial: &SolarSystemState,
        attitude: &Quaternion<f64>,
    ) -> Vector3<f64>;
    
    /// Returns gravity gradient torque in body frame
    fn gradient_torque(
        &self,
        position: &Vector3<f64>,
        inertia: &Matrix3<f64>,
        celestial: &SolarSystemState,
    ) -> Vector3<f64>;
}

Exit criterion: TCE-02 passes (LEO satellite propagated 24h, error < 1 km vs TLE).
1.4 Atmospheric Models (Weeks 10–14, parallel with 1.3)

Tasks:
Task	Description	Validation
NRLMSISE-00 Port	Fortran → Rust translation, validate against reference outputs	Density at 400km matches to < 5% for given F10.7, Ap
Space Weather Data Loader	Daily F10.7, Ap/Kp index ingestion	Historical date lookup returns correct values
Jacchia-Bowman (optional)	Alternative model for cross-validation	Density within 10% of NRLMSISE at test altitudes
Atmosphere Winds Model	HWM model for horizontal wind velocity	Wind direction matches published climatology

Dependencies: 0.2 (data)

Exit criterion: TAE-01 setup complete (density model returns physically reasonable values matching reference implementations).
1.5 Multi-Rate Integrator (Weeks 12–16)

Tasks:
Task	Description	Validation
RK8(9) Adaptive	Dormand-Prince or Verner method, adaptive step	Orbit energy conservation: ΔE/E < 10⁻¹² per orbit
RK4(5) Fixed	Inner loop for attitude/control	Step stability for bang-bang RCS inputs
Coupling Layer	Inner loop averaged thrust feeds outer loop	No energy drift over 1-hour burn
Stiffness Detector	Flag when step size collapses due to stiffness	Trigger fallback or warn

Dependencies: 1.3 (gravity), 1.4 (atmosphere)
pub trait Integrator {
    fn step<F: FnMut(&StateVector) -> StateDerivative>(
        &mut self,
        state: &mut StateVector,
        derivative_fn: F,
        dt: Duration,
    ) -> IntegrationResult;
}

pub struct MultiRateIntegrator {
    pub outer: Box<dyn Integrator>,  // RK8(9) for translation
    pub inner: Box<dyn Integrator>,  // RK4(5) for attitude
    pub flexible: Box<dyn Integrator>, // RK4 for modal coords
}

Exit criterion: Two-body propagation (Earth + satellite) matches Keplerian analytical solution to < 1 m over 10 orbits.
1.6 Single-Spacecraft 6DOF Propagation (Weeks 14–20)

Tasks:
Task	Description	Validation
ECS Component Registration	All core components defined, spawn single spacecraft	Entity spawns and persists
ForceAggregator System	Collects gravity + drag + SRP forces	Force sum balances at equilibrium (circular orbit)
Multi-Rate Step System	Integrates position/velocity/attitude	Stable over 24h propagation
Solar Radiation Pressure	Panelized model with eclipse detection	SRP acceleration matches cannonball within 5%
Atmospheric Drag	Ballistic coefficient from vehicle config	ISS TLE propagation matches within 1 km / 24h
Mass/CG Tracking	Update mass as fuel burns, update inertia	CG shift matches analytical fuel distribution

Dependencies: 1.3, 1.4, 1.5
// Phase 1 demo: single spacecraft in LEO
fn main() {
    let mut world = World::new();
    
    // Spawn Earth (from ephemeris)
    world.spawn(BodyState::earth_at(epoch));
    
    // Spawn spacecraft at ISS altitude
    let iss_state = tle_to_statevec(ISS_TLE);
    world.spawn(SpacecraftBundle {
        kinematics: Kinematics::from_statevec(iss_state),
        dynamics: Dynamics {
            mass: 420_000.0,  // ISS mass
            inertia: Matrix3::identity() * 1e7,
            ..
        },
        ..default()
    });
    
    // Propagate 24 hours
    let mut clock = ClockService::new(epoch);
    for tick in 0..86_400 {
        world.run_system(aggregate_forces);
        world.run_system(multi_rate_step, Duration::from_secs(1));
        clock.advance(Duration::from_secs(1));
    }
    
    // Validate against next-day TLE
    assert!(position_error < 1_000.0);  // < 1 km
}

Exit criterion: MVP Milestone G1. Single spacecraft propagates 24 hours in LEO with < 1 km error vs TLE. All P0 celestial and gravity requirements validated.
Phase 2: Networking & Multiplayer (Weeks 20–32)
2.1 FlatBuffer Schema Design (Weeks 20–22)

Tasks:
Task	Description
Define snapshot schema	World state: celestial bodies + spacecraft deltas
Define command schema	Player inputs: thrust, attitude, valve, etc.
Define event schema	Docking, staging, collision, alarm events
Generate Rust + C++ bindings	For both server and Godot client
Delta encoding utilities	Difference from last-acknowledged state
// schemas/snapshot.fbs
namespace apogee;

table WorldSnapshot {
  tick: ulong;
  server_time_tdb: double;
  celestial: [CelestialBodyState];
  spacecraft: [SpacecraftDelta];
  events: [GameEvent];
}

table CelestialBodyState {
  naif_id: int;
  position: Vec3;
  velocity: Vec3;
}

table SpacecraftDelta {
  id: uint;
  position: Vec3 (deprecated);
  pos_delta: Vec3Delta;
  velocity: Vec3;
  vel_delta: Vec3Delta;
  attitude: Quat;
  angular_vel: Vec3;
  mass: double;
  fuel: [TankDelta];
  systems: SystemsSnapshot;
  projection: StateProjection;
  timestamp: double;
}

struct Vec3Delta { x: short; y: short; z: short; scale: float; }
struct Quat { w: float; x: float; y: float; z: float; }
struct Vec3 { x: double; y: double; z: double; }

Dependencies: Phase 1 (need to know what state to serialize)

Exit criterion: Rust server can serialize a snapshot, Godot client can deserialize it, round-trip verified in unit test.
2.2 QUIC Server (Weeks 22–26)

Tasks:
Task	Description	Validation
Connection Manager	Quinn-based QUIC listener, handshake, auth	100 concurrent connections stable
Snapshot Builder	Per-client delta-encoded state packets	Packet size < 1KB per spacecraft
Rate Limiter	Per-connection bandwidth allocation	Total bandwidth < 5 Mbps for 100 players
Heartbeat / Timeout	Dead connection cleanup	Timeout within 5s of connection loss

Dependencies: 2.1 (schema)

Exit criterion: Server broadcasts simulated state to 10 connected clients at 10 Hz for 1 hour without disconnects.
2.3 Client Prediction & Reconciliation (Weeks 26–30)

Tasks:
Task	Description	Validation
Input Command Queue	Buffer pending inputs, tag with tick	Commands arrive at server in order
Local Prediction	Client-side propagation using simplified model	Prediction error < 1m at 100ms RTT
Server Reconciliation	Snap authoritative state, replay pending inputs	No visible teleport at < 200ms RTT
Interpolation Buffer	100ms snapshot buffer, interpolate at 60fps	Smooth rendering, no jitter
Projection Matching	Client uses server-provided future-state projection	Snapping only occurs under packet loss

Dependencies: 2.2 (server), Phase 1 (propagation model for prediction)

Exit criterion: TMN-01 preliminary — player-controlled spacecraft responsive at < 100ms input lag, no visual stutter at 200ms RTT.
2.4 Interest Management (Weeks 28–32)

Tasks:
Task	Description	Validation
Spatial Octree	Per-shard octree of spacecraft positions	Query < 1ms for 1000 entities
Relevance Scoring	Distance, ownership, docking status weighting	Player's own craft always highest priority
Variable Rate Dispatch	Different update rates per relevance tier	Distant craft at 1Hz, proximate at 50Hz
Cross-Shard Visibility	Neighboring shards share boundary entities	No "pop-in" when crossing shard boundary

Dependencies: 2.2, 2.3

Exit criterion: 100 simulated spacecraft across 2 shards, each client receives only relevant updates, total bandwidth < 5 Mbps per client.
Phase 3: Godot Client (Weeks 24–40, overlaps with Phase 2)
3.1 GDExtension Bridge (Weeks 24–28)

Tasks:
Task	Description
Rust GDExtension crate	Compile as .so/.dll, load in Godot
Shared State Buffer	Ring buffer in shared memory for sim → render data
Custom Node types	SpacecraftNode, CelestialBodyNode, TrajectoryLine
Float Origin Manager	Rebases Godot coordinates when camera crosses thresholds
Frame Interpolator	Smooth between sim updates at 10Hz to 60fps render
// apogee-godot/src/bridge.rs
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SpacecraftNode {
    #[base]
    base: Base<Node3D>,
    
    sim_entity_id: u64,
    position_buffer: RingBuffer<Vector3>,
    attitude_buffer: RingBuffer<Quaternion>,
    last_snapshot_tick: u64,
}

#[godot_api]
impl INode3D for SpacecraftNode {
    fn process(&mut self, delta: f64) {
        // Interpolate position from buffer
        let t = self.base.get_process_delta_time();
        let interp_pos = self.position_buffer.interpolate_at(t);
        let interp_att = self.attitude_buffer.slerp_at(t);
        
        // Apply floating origin rebasing
        let local_pos = self.float_origin.to_local(interp_pos);
        self.base.set_position(local_pos);
    }
}

Dependencies: 2.1 (FlatBuffer schema for parsing)

Exit criterion: Godot scene displays a single cube moving in a circular orbit, driven by real sim data, at 60fps.
3.2 Camera & Scene Management (Weeks 28–32)

Tasks:
Task	Description
Floating Origin System	Rebases all rendered objects when camera exceeds threshold
LOD Manager	Switch between full mesh, billboards, invisible based on distance
Camera Modes	Free, chase, cockpit, map/2D orbital view
Scene Graph Bridge	Map sim entities to Godot scene nodes automatically

Dependencies: 3.1

Exit criterion: Camera can follow spacecraft from LEO to lunar transfer, no visual artifacts, smooth LOD transitions.
3.3 Planet Rendering (Weeks 30–36)

Tasks:

| Task | Description | |------|-------------|------------| | Chunked Sphere LOD | Quadtree subdivision of sphere, adaptive resolution | Earth surface detail from LEO to GEO transition | | Heightmap Displacement | DEM data for terrain elevation | Mountains visible from orbit | | Atmospheric Scattering Shader | Rayleigh/Mie scattering for sky and limb | Matches astronaut photography qualitatively | | Cloud Layer | Procedural or texture-based cloud cover | Animated, parallax with surface | | Night Lights | City light textures on dark side | Visible from orbit |

Dependencies: 3.2

Exit criterion: Earth renders from surface to orbit with visual quality comparable to KSP/RSS with scatterer mod.
3.4 HUD & Instruments (Weeks 34–40)

Tasks:

| Task | Description | |------|-------------|------------| | Orbit Display | 2D map view showing trajectory, apoapsis/periapsis, nodes | Conic sections match sim propagation | | Attitude Indicator | Artificial horizon, rate gyros, ball | Updates at 60fps from sim state | | Nav Display | Orbital elements, velocity, altitude, dynamic pressure | All values from authoritative sim state | | Maneuver Planner UI | Click-to-set burn nodes, Δv calculator | Burn solution matches analytic Hohmann within 1 m/s | | Alarm/Warning Panel | Surface contact, thermal limit, fuel low, comm blackout | Triggers from sim event stream |

Dependencies: 3.1, Phase 1 (sim state)

Exit criterion: Player can see their spacecraft position, velocity, orbital elements, and attitude in real-time, all sourced from the authoritative simulation.
Phase 4: Vehicle Systems (Weeks 36–52)
4.1 Propulsion & Staging (Weeks 36–42)

Tasks:
Task	Description	Validation
Thruster Component	Per-engine thrust, ISP, gimbal, throttle range	Δv matches rocket equation within 5%
Propellant Tank Component	Fuel mass, volume, pressure, feed system	Mass depletion rate matches commanded thrust
Staging System	Separation events, dynamic component removal/addition	Stage separation creates debris with correct velocity
Gimbal Control	Thrust vector deflection, TVC coupling to attitude	Attitude response matches LQR controller design

Dependencies: Phase 1 (integrator, ECS)
4.2 Aerodynamic Database System (Weeks 38–46)

Tasks:
Task	Description	Validation
Component Aero Library	Per-part aero coefficients (nose cone, cylinder, fin)	Individual part C_L/C_D match analytical slender body theory
Interference Matrix	Pairwise body-fin, body-body corrections	Wave drag within 20% of area-rule estimate
Assembly Builder	Combine parts → vehicle-level aero DB dynamically	Staging changes aero DB in real-time
Regime Blending	Smooth transition between aero/vacuum based on dynamic pressure	No discontinuity > 0.01 m/s² in acceleration
CFD Import Pipeline	Parse OpenFOAM/FUN3D output → internal coefficient tables	Imported data matches source within interpolation error

Dependencies: Phase 1 (ECS, force aggregator), Phase 3 (part visualization)
4.3 Flexible Body & Slosh (Weeks 42–48)

Tasks:
Task	Description	Validation
Modal FEM Import	Parse NASTRAN/CalculiX output, extract mode shapes	Mode frequencies match source FEM
Modal Integrator	Integrate modal coordinates at high rate	Energy conservation in uncoupled modes
Coupling System	Feed rigid-body acceleration into modal excitation, feed modal reaction back	Attitude perturbation matches analytical 2-body + appendage model
Fuel Slosh Model	Spring-mass-damper per tank, excited by acceleration	Pendulum period matches published S-IVB data within 10%
CG Shift Tracking	Update spacecraft CG and inertia as fuel redistributes	CG position matches geometric fuel distribution

Dependencies: 4.1 (propulsion provides acceleration excitation), Phase 1 (integrator)
4.4 Interior Dynamics (Weeks 44–52)

Tasks:
Task	Description	Validation
CW Propagator	Clohessy-Wiltshire for interior objects	Free-floating object returns to start ± 1m over 10 orbits
TH Propagator	Tschauner-Hempel for elliptical reference	Error vs full propagation < 0.1% over 1 orbit
Indoor Aero	Drag from cabin atmosphere	Object deceleration matches ½ρv²CdA within 20%
Contact Solver	Sequential impulse, stacking, constraints	Objects rest on surfaces stably during burns
HVAC Flow Field	Vent-positioned airflow vectors	Objects drift toward vents over minutes
EVA Dynamics	CW + environmental forces + SAFER thrusters	EVA trajectory reproducible to < 1m over 1 hour
Feedback Coupling	Interior contact impulses feed back to spacecraft dynamics	Push-off impulse matches Newton's 3rd law

Dependencies: Phase 1 (spacecraft propagation provides reference orbit), Phase 2 (networking for multiplayer interior)
Phase 5: Gameplay & Accessibility (Weeks 48–64)
5.1 Progressive Difficulty System (Weeks 48–54)

Tasks:
Task	Description
Autopilot Controller	Full GNC stack: Lambert solver + LQR attitude + automated docking
Assisted Mode	Orbital maneuver suggestions, visual burn markers, attitude hold
Manual Mode	Raw thruster control, no automation
API Mode	Expose GNC API as Lua/scripts
Difficulty Config	YAML-based per-scenario settings
5.2 Mission & Campaign System (Weeks 52–58)

Tasks:
Task	Description
Mission Definition Format	YAML/JSON scenario files with objectives, constraints, win conditions
Tutorial Campaign	10-mission progression: orbit → rendezvous → landing → interplanetary
Sandbox Mode	Free-play with vehicle library, custom scenarios
Mission Sharing	Export/import scenario files
5.3 Vehicle Editor (Weeks 54–62)

Tasks:
Task	Description
Part Library	50+ base parts: fuel tanks, engines, capsules, fins, solar panels
Assembly UI	Drag-and-drop part attachment, snap points, symmetry
Staging Editor	Define stage separation sequence, event triggers
Validation	Check for missing components, insufficient Δv, CG issues
Craft Persistence	Save/load vehicle designs
5.4 Content Production (Weeks 58–64)

Tasks:
Task	Description
Vehicle Templates	20 pre-built spacecraft (probe, station, lander, launcher)
Mission Pack	50 community-ready scenarios
Planet Textures	Earth, Moon, Mars high-resolution surface maps
Audio	Engine sounds, radio chatter, ambient, alarms
Localization	English first, extendable to 11 languages
Phase 6: Hardening & Launch (Weeks 60–72)
6.1 Performance Optimization
Task	Target
Profile and optimize hot paths	Force aggregation < 5ms per 200 spacecraft
SIMD vectorize spherical harmonics	2× throughput improvement
Parallelize ECS systems	All embarrassingly parallel systems use Rayon
Memory pool for snapshots	Zero allocation during steady-state networking
GPU instancing for debris fields	10,000+ fragments at 60fps
6.2 Validation Campaign
Test	Method	Acceptance
TCE-01: Mars ephemeris	Compare vs Horizons 1-year propagation	< 100 km
TCE-02: LEO 24h propagation	Compare vs ISS TLE + Celestrak	< 1 km
TAE-01: Drag decay	Propagate Starlink TLE 7 days	< 10% Cd error
TAE-02: Launch trajectory	Model Falcon 9, compare vs webcast	< 5% velocity at MECO
TAE-03: Re-entry	Model Tiangong-1 decay	< 5 km downrange at impact
TMN-01: Multiplayer stress	200 spacecraft, 10 clients	< 10ms tick, < 5Mbps/client
TMN-02: Shard handoff	Transfer from Earth to Moon shard	< 0.1m discontinuity
TMN-03: Packet loss recovery	20% packet loss injection	Recovery within 2s
6.3 Deployment Infrastructure
Task	Description
Docker compose for single-shard dev	Server + PostgreSQL + Redis in containers
Kubernetes manifests for production	Shard autoscaling, load balancing
Backup & recovery	Database snapshots, state checkpointing
Monitoring	Prometheus metrics, Grafana dashboards
Critical Path Diagram
Phase 0: Foundation
  0.1 Scaffolding ──┐
  0.2 Data Pipeline ─┤
  0.3 CI Setup ──────┘
         │
         ▼
Phase 1: Core Propagator
  1.1 Time/Frames ──┐
  1.2 Ephemeris ────┤ (parallel)
  1.3 Gravity ───────┤
  1.4 Atmosphere ────┤ (parallel with 1.3)
  1.5 Integrator ────┤
  1.6 6DOF Demo ─────┘ ← MVP G1
         │
         ├──────────────────────────┐
         ▼                          ▼
Phase 2: Networking          Phase 3: Godot Client
  2.1 Schemas ────┐          3.1 GDExtension ──┐
  2.2 QUIC Server ┤          3.2 Camera/Scene ──┤
  2.3 Prediction ──┤          3.3 Planets ──────┤
  2.4 Interest ────┘          3.4 HUD ──────────┘ ← G2 Alpha
         │                          │
         └──────────┬───────────────┘
                    ▼
Phase 4: Vehicle Systems
  4.1 Propulsion/Staging ──┐
  4.2 Aero DB ────────────┤
  4.3 Flex/Slosh ─────────┤
  4.4 Interior ───────────┘ ← G3 Beta
         │
         ▼
Phase 5: Gameplay
  5.1 Difficulty ──┐
  5.2 Missions ────┤
  5.3 Editor ──────┤
  5.4 Content ──────┘ ← G4 Feature Complete
         │
         ▼
Phase 6: Hardening
  6.1 Optimize ──┐
  6.2 Validate ──┤
  6.3 Deploy ────┘ ← G5 Launch
Effort Estimate Summary
Phase	Duration	Effort (person-weeks)	Key Risk
0: Foundation	4 weeks	4	Low — scaffolding
1: Core Propagator	16 weeks	16	High — numerical correctness
2: Networking	12 weeks	12	Medium — prediction/reconciliation
3: Godot Client	16 weeks	16	Medium — floating origin, planet LOD
4: Vehicle Systems	16 weeks	16	Medium — aero DB, flex body
5: Gameplay	16 weeks	16	Low — content creation
6: Hardening	12 weeks	12	Medium — performance tuning
Total	~72 weeks	~92 person-weeks	

At 1 full-time engineer: ~18 months to launch. With a team of 3 (sim/core, networking, client): ~8–10 months with overlap.