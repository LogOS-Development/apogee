//! Elevation transect sampler.
//!
//! Loads a GeoTIFF elevation tile and dumps a CSV transect across the grid.
//! Useful for verifying terrain data and visualizing elevation profiles.
//!
//! Usage:
//!   cargo run --example elevation_transect -- <geotiff_path> [output.csv]
//!
//! If no output path is given, CSV is written to stdout.

use verdant::ElevationGrid;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: elevation_transect <geotiff_path> [output.csv]");
        std::process::exit(1);
    }

    let path = &args[1];
    let output = args.get(2);

    let grid = ElevationGrid::from_geotiff_file(path).unwrap_or_else(|e| {
        eprintln!("Failed to load GeoTIFF: {e}");
        std::process::exit(1);
    });

    let stats = grid.stats();
    println!(
        "Loaded: {}x{} grid, {:.0} valid pixels",
        grid.width, grid.height, stats.valid_pixels
    );
    println!(
        "Elevation: min={:.1}m  max={:.1}m  mean={:.1}m",
        stats.min, stats.max, stats.mean
    );
    println!(
        "Pixel size: {:.4}m x {:.4}m",
        grid.geotransform.pixel_width,
        grid.geotransform.pixel_height.abs()
    );

    // Sample a transect along the middle row
    let mid_row = grid.height / 2;
    let mut csv = String::new();
    csv.push_str("col,x_m,y_m,elevation_m,slope_rad,aspect_rad\n");

    for col in 0..grid.width {
        let (x, y) = grid.geotransform.pixel_to_world(col, mid_row);
        let elev = grid.at(col, mid_row);
        let slope = grid.slope(col, mid_row);
        let aspect = grid.aspect(col, mid_row);

        csv.push_str(&format!(
            "{col},{x:.2},{y:.2},{elev:.2},{slope:.6},{aspect:.6}\n"
        ));
    }

    match output {
        Some(path) => {
            std::fs::write(path, &csv).unwrap_or_else(|e| {
                eprintln!("Failed to write CSV: {e}");
                std::process::exit(1);
            });
            println!("Transect written to {path} ({} samples)", grid.width);
        }
        None => {
            print!("{csv}");
        }
    }
}
