//! WRF Kessler microphysics sampler — emits CSV to stdout for validation.
//!
//! Run with: cargo run --example wrf_kessler_samples -p apogee-core --features wrf

use apogee_core::aero::wrf;

fn main() {
    println!("# WRF Kessler microphysics — single column, 10s timestep");
    println!("# k,theta_in,theta_out,qv_in,qv_out,qc_in,qc_out,qr_in,qr_out,rain_mm,rain_rate");

    let nk = 10;
    let input = wrf::KesslerInput {
        theta: (0..nk).map(|k| 300.0 - k as f32 * 3.0).collect(),
        qv: (0..nk).map(|k| 0.015 - k as f32 * 0.001).collect(),
        qc: (0..nk)
            .map(|k| {
                if k < 5 {
                    0.001 - k as f32 * 0.0001
                } else {
                    0.0
                }
            })
            .collect(),
        qr: (0..nk)
            .map(|k| if k > 0 && k < 5 { 0.0003 } else { 0.0 })
            .collect(),
        rho: (0..nk).map(|k| 1.0 + k as f32 * 0.05).collect(),
        pii: (0..nk).map(|k| 1.0 - k as f32 * 0.02).collect(),
        z: (0..nk).map(|k| 100.0 + k as f32 * 300.0).collect(),
        dz8w: vec![300.0; nk],
        dt: 10.0,
    };

    let output = wrf::kessler(&input);

    for k in 0..nk {
        println!(
            "{},{:.4},{:.4},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            k + 1,
            input.theta[k],
            output.theta[k],
            input.qv[k],
            output.qv[k],
            input.qc[k],
            output.qc[k],
            input.qr[k],
            output.qr[k],
            output.rain_accumulated,
            output.rain_rate
        );
    }

    eprintln!("\nRain accumulated: {:.6} mm", output.rain_accumulated);
    eprintln!("Rain rate: {:.6} mm", output.rain_rate);
}
