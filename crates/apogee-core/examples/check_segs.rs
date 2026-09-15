use apogee_core::ephemeris::Kernel;
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
    let kernel = Kernel::load(&ephemeris_path()).unwrap();
    for seg in kernel.segments() {
        if [301, 399, 3, 10].contains(&seg.target_id) {
            println!(
                "target={} center={} type={} start={} end={}",
                seg.target_id, seg.center_id, seg.spk_type, seg.start_et, seg.end_et
            );
        }
    }
}
