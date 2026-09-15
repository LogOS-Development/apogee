use apogee_core::ephemeris::EphemerisService;
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
    let mut eph = EphemerisService::load(&ephemeris_path(), 32).unwrap();
    let epoch = Epoch::from_gregorian_utc(2026, 1, 1, 0, 0, 0, 0);

    for b in eph.bodies() {
        print!("NAIF={} ", b.naif_id);
    }
    println!();

    for naif in [10, 199, 299, 399, 301, 3, 4] {
        match eph.state_at(naif, epoch) {
            Ok(state) => println!(
                "NAIF {}: pos=[{:.3e}, {:.3e}, {:.3e}] vel=[{:.3e}, {:.3e}, {:.3e}]",
                naif,
                state.position[0],
                state.position[1],
                state.position[2],
                state.velocity[0],
                state.velocity[1],
                state.velocity[2]
            ),
            Err(e) => println!("NAIF {}: error: {}", naif, e),
        }
    }
}
