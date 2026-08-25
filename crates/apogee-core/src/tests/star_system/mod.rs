//! Config-driven system definitions integration tests.
//!
//! These exercise the full pipeline: `SystemDefinition` (JSON / preset /
//! random) → `World::add_system` → `step_world`, verifying that per-body
//! gravity models survive the config → ECS boundary.

use crate::components::celestial::GravitySource;
use crate::star_system::{presets, BodyRole, GravityConfig, SystemDefinition};
use crate::systems::step::step_world;
use crate::world::World;
use apogee_common::units::Seconds;

/// Find the `GravitySource` of the body with the given NAIF ID.
fn gravity_source_of(world: &World, naif_id: i32) -> Option<GravitySource> {
    use crate::components::celestial::NaifIdComponent;
    use crate::components::kinematics::Kinematics;
    for (_, (id, gs, _)) in world
        .query::<(&NaifIdComponent, &GravitySource, &Kinematics)>()
        .iter()
    {
        if id.0 == naif_id {
            return Some(gs.clone());
        }
    }
    None
}

#[test]
fn add_system_attaches_j2_to_earth_source() {
    let system = presets::earth_moon_j2();
    let mut world = World::new();
    world.add_system(&system).unwrap();

    // Earth (NAIF 399) must carry the resolved J2 model.
    let earth = gravity_source_of(&world, 399).expect("Earth gravity source");
    let sh = earth.spherical_harmonics.expect("Earth SH model");
    assert_eq!(sh.c[2][0], presets::EARTH_C20);

    // Moon (NAIF 301) stays point-mass.
    let moon = gravity_source_of(&world, 301).expect("Moon gravity source");
    assert!(moon.spherical_harmonics.is_none());

    // Three entities spawned.
    assert_eq!(world.len(), 3);
}

#[test]
fn json_system_into_world_runs() {
    let json = r#"{
        "name": "test-system",
        "bodies": [
            {
                "name": "primary",
                "role": "central",
                "position": [0.0, 0.0, 0.0],
                "gm": 3.986e14,
                "radius": 6378000.0,
                "gravity": {"type": "j2", "c20": -0.00048}
            },
            {
                "name": "companion",
                "role": "moon",
                "position": [400000000.0, 0.0, 0.0],
                "velocity": [0.0, 1022.0, 0.0],
                "gm": 4.9e12
            }
        ]
    }"#;

    let system = SystemDefinition::from_json_str(json).unwrap();
    let mut world = World::new();
    world.add_system(&system).unwrap();
    assert_eq!(world.len(), 2);

    // Both bodies present: central (kinematic) + companion (dynamic).
    let kinematic_count = world
        .query::<&crate::components::celestial::CelestialKind>()
        .iter()
        .filter(|(_, k)| !k.is_dynamic())
        .count();
    let dynamic_count = world
        .query::<&crate::components::celestial::CelestialKind>()
        .iter()
        .filter(|(_, k)| k.is_dynamic())
        .count();
    assert_eq!(kinematic_count, 1);
    assert_eq!(dynamic_count, 1);

    // Step the world — nothing should blow up.
    step_world(&mut world, Seconds::new(10.0));
}

#[test]
fn random_system_into_world_steps() {
    let system = SystemDefinition::random(1234, 3);
    let mut world = World::new();
    world.add_system(&system).unwrap();

    assert_eq!(world.len(), 4); // star + 3 planets

    // Star is kinematic — position never changes.
    let star_pos_before = world
        .query::<&crate::components::kinematics::Kinematics>()
        .iter()
        .map(|(_, k)| k.position)
        .min_by(|a, b| a.norm().partial_cmp(&b.norm()).unwrap())
        .unwrap();

    for _ in 0..100 {
        step_world(&mut world, Seconds::new(60.0));
    }

    let star_pos_after = world
        .query::<&crate::components::kinematics::Kinematics>()
        .iter()
        .map(|(_, k)| k.position)
        .min_by(|a, b| a.norm().partial_cmp(&b.norm()).unwrap())
        .unwrap();

    assert_eq!(star_pos_before, star_pos_after);

    // All bodies still finite after 100 minutes of propagation.
    for (_, kin) in world
        .query::<&crate::components::kinematics::Kinematics>()
        .iter()
    {
        assert!(kin.position.iter().all(|c| c.is_finite()));
        assert!(kin.velocity.iter().all(|c| c.is_finite()));
    }
}

#[test]
fn preset_inner_solar_system_round_trips() {
    let system = presets::inner_solar_system();
    let json = system.to_json_str().unwrap();
    let parsed = SystemDefinition::from_json_str(&json).unwrap();
    assert_eq!(parsed, system);

    // Mars carries J2 with the derived C_2,0.
    let mars = system.body("Mars").unwrap();
    assert_eq!(
        mars.gravity,
        GravityConfig::J2 {
            c20: presets::MARS_C20
        }
    );
    assert_eq!(mars.role, BodyRole::Planet);
}
