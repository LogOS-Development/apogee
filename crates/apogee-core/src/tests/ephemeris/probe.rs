//! DE441 ephemeris validation against JPL Horizons reference vectors.
//!
//! These tests load the full DE441 kernel and compare evaluated states for
//! multiple bodies (Mercury, Venus, Earth, Mars, Jupiter) at multiple epochs
//! against hard-coded JPL Horizons reference vectors.
//!
//! Horizons query parameters for each reference:
//!   - CENTER = @0 (solar system barycenter)
//!   - TIME_TYPE = TDB
//!   - OUT_UNITS = KM-S
//!   - REF_PLANE = FRAME (ICRF)
//!   - VEC_TABLE = 2

use crate::ephemeris::Kernel;
use hifitime::Epoch;

/// DE441 kernel path relative to the crate manifest dir.
const DE441_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/ephemeris/de441.bsp"
);

/// A Horizons reference state vector for a single body at a single epoch.
/// All values are in km and km/s (ICRF, SSB-centered, TDB).
struct HorizonsRef {
    naif_id: i32,
    name: &'static str,
    et_s: f64,
    pos: [f64; 3],
    vel: [f64; 3],
}

/// Epoch: 2025-01-01 12:00:00 TDB (et = 789004800 s)
const EPOCH_1_ET: f64 = 789_004_800.0;

/// Epoch: 2025-07-01 12:00:00 TDB
const EPOCH_2_ET: f64 = 789_004_800.0 + 181.0 * 86_400.0; // ~2025-07-01

/// Reference vectors from JPL Horizons (DE441 source), ICRF, SSB-centered.
/// Generated via the Horizons API with OUT_UNITS=KM-S, REF_PLANE=FRAME.
const REFS: &[HorizonsRef] = &[
    // --- Epoch 1: 2025-01-01 12:00:00 TDB ---
    HorizonsRef {
        naif_id: 1,
        name: "Mercury Barycenter",
        et_s: EPOCH_1_ET,
        pos: [
            -5.839199605530626e7,
            -2.582183292582754e7,
            -7.732733360142212e6,
        ],
        vel: [10.03259431338766, -37.04617867658504, -20.82830484009049],
    },
    HorizonsRef {
        naif_id: 2,
        name: "Venus Barycenter",
        et_s: EPOCH_1_ET,
        pos: [
            6.578_482_433_857_72e7,
            7.840390475382325e7,
            3.110336615653447e7,
        ],
        vel: [-27.66080761338582, 18.92166578470883, 10.26478215400927],
    },
    HorizonsRef {
        naif_id: 3,
        name: "Earth Barycenter",
        et_s: EPOCH_1_ET,
        pos: [
            -2.887091561098969e7,
            1.318119052498327e8,
            5.716800038754769e7,
        ],
        vel: [-29.71622734915274, -5.312969752487387, -2.303142894284423],
    },
    HorizonsRef {
        naif_id: 4,
        name: "Mars Barycenter",
        et_s: EPOCH_1_ET,
        pos: [
            -7.984_998_626_423_25e7,
            2.057572514152151e8,
            9.655311121122174e7,
        ],
        vel: [-21.96553725994941, -5.560580018886087, -1.957730883643913],
    },
    HorizonsRef {
        naif_id: 5,
        name: "Jupiter Barycenter",
        et_s: EPOCH_1_ET,
        pos: [
            1.565643810330189e8,
            6.844256122609742e8,
            2.895574108839124e8,
        ],
        vel: [-12.93394449470417, 2.932_931_677_331_32, 1.572019035393726],
    },
    // --- Epoch 2: ~2025-07-01 12:00:00 TDB ---
    HorizonsRef {
        naif_id: 1,
        name: "Mercury Barycenter",
        et_s: EPOCH_2_ET,
        pos: [
            -5.115753241725348e7,
            -4.064557512018487e7,
            -1.637833625440764e7,
        ],
        vel: [21.646_925_227_270_2, -30.1006012511239, -18.32201061645758],
    },
    HorizonsRef {
        naif_id: 2,
        name: "Venus Barycenter",
        et_s: EPOCH_2_ET,
        pos: [
            1.027948642116541e8,
            -2.864314331375229e7,
            -1.939597479052624e7,
        ],
        vel: [10.52404819409221, 30.51076995983477, 13.06375201440726],
    },
    HorizonsRef {
        naif_id: 3,
        name: "Earth Barycenter",
        et_s: EPOCH_2_ET,
        pos: [
            2.459243423621869e7,
            -1.383504834081714e8,
            -5.994712581148305e7,
        ],
        vel: [28.90487685948367, 4.433161234993208, 1.921275005453349],
    },
    HorizonsRef {
        naif_id: 4,
        name: "Mars Barycenter",
        et_s: EPOCH_2_ET,
        pos: [
            -2.423030206272712e8,
            -3.997105153450167e7,
            -1.177155745335202e7,
        ],
        vel: [4.926_876_872_417_53, -19.796_157_478_189_5, -9.212774188932173],
    },
    HorizonsRef {
        naif_id: 5,
        name: "Jupiter Barycenter",
        et_s: EPOCH_2_ET,
        pos: [
            -4.896_992_096_361_96e7,
            7.048983142502066e8,
            3.033363908220054e8,
        ],
        vel: [-13.19118092953951, -0.3107246294206415, 0.1879585519427641],
    },
];

#[test]
#[ignore = "loads the DE441 kernel (3.5 GB) and is slow; run with --ignored or in the nightly slow-test job"]
#[ntest::timeout(300_000)]
fn de441_mars_barycenter_vs_horizons() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../data/ephemeris/de441.bsp"
    );
    if !std::path::Path::new(path).exists() {
        eprintln!("SKIP: DE441 kernel not found at {path}");
        return;
    }
    let kernel = Kernel::load(path).unwrap();

    let epoch = Epoch::from_gregorian(2025, 1, 1, 12, 0, 0, 0, hifitime::TimeScale::TDB);
    let state = kernel.state_at(4, epoch.to_tdb_seconds()).unwrap();

    // Horizons reference vector (TDB, KM-S).
    let expected_pos = nalgebra::Vector3::new(
        -79_849_986.264_232_5,
        205_757_251.415_215_1,
        96_553_111.211_221_74,
    );
    let expected_vel = nalgebra::Vector3::new(
        -21.965_537_259_949_41,
        -5.560_580_018_886_087,
        -1.957_730_883_643_913,
    );

    let pos_err = (state.position - expected_pos).norm();
    let vel_err = (state.velocity - expected_vel).norm();

    assert!(pos_err < 1.0, "position error too large: {} km", pos_err);
    assert!(vel_err < 1e-3, "velocity error too large: {} km/s", vel_err);
}

/// Validate DE441 ephemeris states for multiple bodies at two epochs against
/// JPL Horizons reference vectors. This is the main ephemeris validation test.
#[test]
#[ignore = "loads the DE441 kernel (3.5 GB) and is slow; run with --ignored or in the nightly slow-test job"]
#[ntest::timeout(300_000)]
fn de441_multi_body_vs_horizons() {
    if !std::path::Path::new(DE441_PATH).exists() {
        eprintln!("SKIP: DE441 kernel not found at {DE441_PATH}");
        return;
    }
    let kernel = Kernel::load(DE441_PATH).expect("DE441 kernel should load");

    let mut checked = 0;
    for r in REFS {
        let state = kernel
            .state_at(r.naif_id, r.et_s)
            .unwrap_or_else(|e| panic!("state_at({}) failed for {}: {:?}", r.naif_id, r.name, e));

        let expected_pos = nalgebra::Vector3::new(r.pos[0], r.pos[1], r.pos[2]);
        let expected_vel = nalgebra::Vector3::new(r.vel[0], r.vel[1], r.vel[2]);

        let pos_err = (state.position - expected_pos).norm();
        let vel_err = (state.velocity - expected_vel).norm();

        println!(
            "{} (NAIF {}) at et={:.0}: pos_err={pos_err:.6} km, vel_err={vel_err:.9} km/s",
            r.name, r.naif_id, r.et_s
        );

        // DE441 Chebyshev evaluation should match Horizons to sub-km precision.
        assert!(
            pos_err < 1.0,
            "{} position error too large: {pos_err:.6} km",
            r.name
        );
        assert!(
            vel_err < 1e-3,
            "{} velocity error too large: {vel_err:.9} km/s",
            r.name
        );
        checked += 1;
    }

    assert!(checked == REFS.len(), "did not check all reference vectors");
}
