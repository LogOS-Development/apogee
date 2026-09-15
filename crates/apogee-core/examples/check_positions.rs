use apogee_core::ephemeris::EphemerisService;
use apogee_core::star_system::{presets, StarSystem};
use hifitime::Epoch;
use std::path::PathBuf;

fn ephemeris_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("data/ephemeris/de441.bsp")
        .to_string_lossy()
        .into_owned()
}

fn main() {
    let def = presets::earth_moon();
    let epoch = Epoch::from_gregorian_utc(2026, 1, 1, 0, 0, 0, 0);
    let eph = EphemerisService::load(&ephemeris_path(), 32).unwrap();
    let system = StarSystem::builder(def)
        .with_ephemeris(eph)
        .build_at(epoch)
        .unwrap();

    let world = &system.world;
    for body in &system.definition.bodies {
        let naif = body.naif_id.unwrap_or(0);
        if let Some(entity) = world.find_celestial(naif) {
            if let Ok(mut q) = world
                .ecs
                .query_one::<&apogee_core::components::kinematics::Kinematics>(entity)
            {
                if let Some(kin) = q.get() {
                    let pos = kin.position;
                    println!(
                        "{} (NAIF {}): pos = [{:.3e}, {:.3e}, {:.3e}] m",
                        body.name, naif, pos.x, pos.y, pos.z
                    );
                }
            }
        }
    }
}
