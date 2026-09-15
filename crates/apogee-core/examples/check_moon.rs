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

    let moon = eph.state_at(301, epoch).unwrap();
    let moon_dist =
        (moon.position.x.powi(2) + moon.position.y.powi(2) + moon.position.z.powi(2)).sqrt();
    println!(
        "Moon raw (rel Earth): |r| = {} m = {} km",
        moon_dist,
        moon_dist / 1000.0
    );

    let earth = eph.state_at(399, epoch).unwrap();
    let earth_dist =
        (earth.position.x.powi(2) + earth.position.y.powi(2) + earth.position.z.powi(2)).sqrt();
    println!(
        "Earth raw (rel EMB): |r| = {} m = {} km",
        earth_dist,
        earth_dist / 1000.0
    );

    let earth_ssb = eph.state_at_ssb(399, epoch).unwrap();
    let earth_ssb_dist = (earth_ssb.position.x.powi(2)
        + earth_ssb.position.y.powi(2)
        + earth_ssb.position.z.powi(2))
    .sqrt();
    println!(
        "Earth SSB: |r| = {} m = {} km",
        earth_ssb_dist,
        earth_ssb_dist / 1000.0
    );

    let moon_ssb = eph.state_at_ssb(301, epoch).unwrap();

    let dx = moon_ssb.position.x - earth_ssb.position.x;
    let dy = moon_ssb.position.y - earth_ssb.position.y;
    let dz = moon_ssb.position.z - earth_ssb.position.z;
    let em_dist = (dx * dx + dy * dy + dz * dz).sqrt();
    println!(
        "Earth-Moon distance (from SSB): {} m = {} km",
        em_dist,
        em_dist / 1000.0
    );
    println!("Expected: ~384400000 m = 384400 km");
}
