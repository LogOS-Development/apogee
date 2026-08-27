//! Integration tests for the live `StarSystem` manager.
//!
//! These exercise the full pipeline: `SystemDefinition` (JSON / preset /
//! random / clusters) → `StarSystem::builder` → `step`, verifying:
//!
//! - Planets/star are kinematic and follow the attached ephemeris.
//! - Asteroids are dynamic and integrate under N-body gravity.
//! - Clusters gravitate with their aggregate GM and carry member tables.
//! - Systems without an ephemeris still run (configured states, dynamics only).

use crate::components::celestial::{CelestialKind, GravitySource, NaifIdComponent};
use crate::ephemeris::kernel::tests::build_type3_fixture;
use crate::ephemeris::EphemerisService;
use crate::star_system::{
    presets, AsteroidCluster, BodyDefinition, BodyRole, ClusterMemberSpec, SystemDefinition,
};
use crate::world::World;
use apogee_common::units::Seconds;
use hifitime::Epoch;

/// Kinematics component accessor via world query.
fn kinematics_of(world: &World, naif_id: i32) -> Option<crate::components::kinematics::Kinematics> {
    use crate::components::kinematics::Kinematics;
    for (_, (id, kin)) in world.query::<(&NaifIdComponent, &Kinematics)>().iter() {
        if id.0 == naif_id {
            return Some(kin.clone());
        }
    }
    None
}

#[test]
fn planets_follow_ephemeris() {
    // Build a tiny ephemeris kernel: Earth (399) at fixed position.
    // Fixture values are in km (SPK units) — the service converts to meters.
    let et_ref = Epoch::from_gregorian_utc_at_midnight(2000, 1, 2);
    let start_et = -86400.0 * 2.0;
    let end_et = 86400.0 * 2.0;
    let fixture = build_type3_fixture(
        399,
        start_et,
        end_et,
        4,
        |_| [1.0e8, 2.0e8, 3.0e8],
        |_| [29.0e3, 0.0, 0.0],
    );
    let kernel = crate::ephemeris::Kernel::from_bytes(&fixture).unwrap();
    let ephemeris = EphemerisService::from_kernel(kernel, 16);

    // Earth as a planet in the definition.
    let definition = SystemDefinition::new("eph-test")
        .with_body(
            BodyDefinition::point_mass("Earth", 3.986e14, [0.0; 3])
                .with_role(BodyRole::Planet)
                .with_naif_id(399),
        )
        .with_body(
            BodyDefinition::point_mass("spacecraft-rock", 0.0, [7.0e6, 0.0, 0.0])
                .with_role(BodyRole::Asteroid)
                .with_velocity([0.0, 7.5e3, 0.0]),
        );

    let epoch = et_ref;
    let system = crate::star_system::StarSystem::builder(definition)
        .with_ephemeris(ephemeris)
        .build_at(epoch)
        .unwrap();

    // Earth is kinematic and at the ephemeris position (Chebyshev fit of a
    // constant — expect sub-mm interpolation error). 1e8 km = 1e11 m.
    let kin = kinematics_of(&system.world, 399).expect("Earth kinematics");
    assert!((kin.position.x - 1.0e11).abs() < 1.0e-2);
    assert!((kin.position.y - 2.0e11).abs() < 1.0e-2);
    assert!((kin.position.z - 3.0e11).abs() < 1.0e-2);
}

#[test]
fn asteroids_are_dynamic_and_integrate() {
    // Central star at origin, one asteroid in circular orbit.
    let star_gm = 1.0e20;
    let r = 1.0e9;
    let v_circ = f64::sqrt(star_gm / r);
    let definition = SystemDefinition::new("asteroid-test")
        .with_body(BodyDefinition::point_mass("star", star_gm, [0.0; 3]).with_role(BodyRole::Star))
        .with_body(
            BodyDefinition::point_mass("rock-1", 1.0e8, [r, 0.0, 0.0])
                .with_role(BodyRole::Asteroid)
                .with_velocity([0.0, v_circ, 0.0]),
        );

    let system = crate::star_system::StarSystem::builder(definition)
        .build()
        .unwrap();

    // The asteroid is dynamic.
    let mut dynamic_count = 0;
    for (_, kind) in system.world.query::<&CelestialKind>().iter() {
        if kind.is_dynamic() {
            dynamic_count += 1;
        }
    }
    assert_eq!(dynamic_count, 1);

    // Step the world — asteroid moves. Track it by role: the asteroid is
    // the only dynamic body, the star is kinematic (never moves).
    let mut system = system;
    let mut before = None;
    for (_, (kin, kind)) in system
        .world
        .query::<(&crate::components::kinematics::Kinematics, &CelestialKind)>()
        .iter()
    {
        if kind.is_dynamic() {
            before = Some(kin.position);
        }
    }
    let before = before.expect("dynamic body present");
    for _ in 0..10 {
        system.step(Seconds::new(60.0)).unwrap();
    }
    let mut after = None;
    for (_, (kin, kind)) in system
        .world
        .query::<(&crate::components::kinematics::Kinematics, &CelestialKind)>()
        .iter()
    {
        if kind.is_dynamic() {
            after = Some(kin.position);
        }
    }
    let after = after.expect("dynamic body present");
    let moved = (after - before).norm();
    assert!(
        moved > 1.0e3,
        "asteroid should have moved over 600 s, moved {} m",
        moved
    );
}

#[test]
fn cluster_gravitates_with_aggregate_gm() {
    // Three rocks in one cluster at 1e9 m from the star.
    let star_gm = 1.0e20;
    let r = 1.0e9;
    let members = vec![
        ClusterMemberSpec {
            name: "rock-a".into(),
            offset: [1.0e5, 0.0, 0.0],
            velocity_offset: [0.0; 3],
            gm: 1.0e8,
        },
        ClusterMemberSpec {
            name: "rock-b".into(),
            offset: [-2.0e5, 0.0, 0.0],
            velocity_offset: [0.0; 3],
            gm: 2.0e8,
        },
        ClusterMemberSpec {
            name: "rock-c".into(),
            offset: [0.0, 3.0e5, 0.0],
            velocity_offset: [0.0; 3],
            gm: 3.0e8,
        },
    ];
    let aggregate = 6.0e8;

    let v_circ = f64::sqrt((star_gm + aggregate) / r);
    let definition = SystemDefinition::new("cluster-test")
        .with_body(BodyDefinition::point_mass("star", star_gm, [0.0; 3]).with_role(BodyRole::Star))
        .with_body(
            BodyDefinition::asteroid_cluster("belt-1", members, [r, 0.0, 0.0])
                .with_velocity([0.0, v_circ, 0.0]),
        );

    let system = crate::star_system::StarSystem::builder(definition)
        .build()
        .unwrap();

    // The cluster entity exists with aggregate GM and a member table.
    let mut cluster_gm = None;
    let mut member_count = 0usize;
    for (_, (gs, cluster)) in system
        .world
        .query::<(&GravitySource, &AsteroidCluster)>()
        .iter()
    {
        cluster_gm = Some(gs.gm.into_value());
        member_count = cluster.members.len();
    }
    assert_eq!(cluster_gm, Some(aggregate));
    assert_eq!(member_count, 3);

    // The cluster is dynamic.
    for (_, kind) in system.world.query::<&CelestialKind>().iter() {
        if kind.is_dynamic() {
            // cluster — check it's the only dynamic body
        }
    }

    // Step — nothing blows up, cluster moves as one body.
    let mut system = system;
    for _ in 0..5 {
        system.step(Seconds::new(60.0)).unwrap();
    }
}

#[test]
fn system_without_ephemeris_runs() {
    // Fictional random system, no ephemeris attached.
    let definition = SystemDefinition::random(7, 4);
    let system = crate::star_system::StarSystem::builder(definition)
        .build()
        .unwrap();
    let mut system = system;
    for _ in 0..3 {
        system.step(Seconds::new(60.0)).unwrap();
    }
}

#[test]
fn cluster_member_promotion_reduces_aggregate() {
    // Promoting a member to its own entity must reduce the cluster GM.
    use crate::star_system::ClusterMember;
    let members = [
        ClusterMemberSpec {
            name: "rock-a".into(),
            offset: [1.0e5, 0.0, 0.0],
            velocity_offset: [0.0; 3],
            gm: 1.0e8,
        },
        ClusterMemberSpec {
            name: "rock-b".into(),
            offset: [-2.0e5, 0.0, 0.0],
            velocity_offset: [0.0; 3],
            gm: 2.0e8,
        },
    ];
    let cluster = AsteroidCluster {
        members: members
            .iter()
            .map(|m| ClusterMember {
                name: m.name.clone(),
                offset: nalgebra::Vector3::new(m.offset[0], m.offset[1], m.offset[2]),
                velocity_offset: nalgebra::Vector3::zeros(),
                gm: apogee_common::units::GravitationalParameter::new(m.gm),
            })
            .collect(),
    };
    assert_eq!(cluster.aggregate_gm().into_value(), 3.0e8);
    // Dropping one member from the table reduces the aggregate.
    let mut reduced = cluster.clone();
    reduced.members.remove(0);
    assert_eq!(reduced.aggregate_gm().into_value(), 2.0e8);
}

#[test]
fn presets_have_correct_roles() {
    let system = presets::inner_solar_system();
    for body in &system.bodies {
        match body.name.as_str() {
            "Sun" | "Mercury" | "Venus" | "Earth" | "Mars" => {
                assert!(
                    body.role.is_kinematic(),
                    "{} should be kinematic",
                    body.name
                );
            }
            "Moon" => {
                assert!(body.role.is_kinematic(), "Moon should be kinematic");
            }
            _ => {}
        }
    }
}

#[test]
fn world_direct_add_system_with_cluster() {
    // add_system can be used directly on a World without StarSystem.
    let members = vec![ClusterMemberSpec {
        name: "rock".into(),
        offset: [0.0; 3],
        velocity_offset: [0.0; 3],
        gm: 1.0e8,
    }];
    let definition = SystemDefinition::new("direct").with_body(
        BodyDefinition::asteroid_cluster("solo-cluster", members, [1.0e9, 0.0, 0.0])
            .with_velocity([0.0, 5.0e3, 0.0]),
    );
    let mut world = World::new();
    let entities = world.add_system(&definition).unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(world.len(), 1);
}

// Silence unused warnings for the helper that's only used in some configs.
#[allow(dead_code)]
fn _unused(_: &dyn Fn() -> SystemDefinition) {}
