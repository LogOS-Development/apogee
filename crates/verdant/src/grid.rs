//! Surface grid — geographic coordinate mapping and multi-tile stitching.
//!
//! SurfaceGrid wraps an ElevationGrid with a geographic coordinate system,
//! providing (i,j) ↔ (lat,lon) mapping. Supports stitching multiple
//! adjacent 3DEP tiles into a single contiguous grid.

use crate::terrain::{ElevationGrid, GeoTransform, TerrainError};

/// Geographic bounding box in decimal degrees.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundingBox {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

impl BoundingBox {
    pub fn new(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> Self {
        Self {
            min_lon,
            min_lat,
            max_lon,
            max_lat,
        }
    }

    pub fn width_deg(&self) -> f64 {
        self.max_lon - self.min_lon
    }

    pub fn height_deg(&self) -> f64 {
        self.max_lat - self.min_lat
    }

    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        lon >= self.min_lon && lon <= self.max_lon && lat >= self.min_lat && lat <= self.max_lat
    }
}

/// Geographic coordinate system for a surface grid.
///
/// For 3DEP tiles in geographic projection (lat/lon), the geotransform
/// maps pixel (col,row) → (lon, lat) directly. For UTM-projected tiles,
/// the geotransform maps to easting/northing and a CRS transform is needed.
/// SurfaceGrid assumes geographic coordinates (lon, lat) for simplicity.
#[derive(Debug, Clone)]
pub struct SurfaceGrid {
    pub elevation: ElevationGrid,
    pub bbox: BoundingBox,
}

impl SurfaceGrid {
    /// Create a SurfaceGrid from a single ElevationGrid, computing the
    /// geographic bounding box from the geotransform.
    pub fn from_elevation(elevation: ElevationGrid) -> Self {
        let gt = &elevation.geotransform;
        let min_lon = gt.origin_x;
        let max_lon = gt.origin_x + elevation.width as f64 * gt.pixel_width;
        let min_lat = gt.origin_y + elevation.height as f64 * gt.pixel_height;
        let max_lat = gt.origin_y;

        Self {
            elevation,
            bbox: BoundingBox::new(min_lon, min_lat, max_lon, max_lat),
        }
    }

    /// Load a SurfaceGrid from a GeoTIFF file.
    pub fn from_geotiff_file(path: &str) -> Result<Self, TerrainError> {
        let elevation = ElevationGrid::from_geotiff_file(path)?;
        Ok(Self::from_elevation(elevation))
    }

    /// Elevation at geographic coordinates (lon, lat) via bilinear interpolation.
    pub fn elevation_at(&self, lon: f64, lat: f64) -> f32 {
        self.elevation.interpolate(lon, lat)
    }

    /// Slope at geographic coordinates (lon, lat).
    pub fn slope_at(&self, lon: f64, lat: f64) -> f32 {
        let (col, row) = self.elevation.geotransform.world_to_pixel(lon, lat);
        self.elevation.slope(col, row)
    }

    /// Check if a geographic point is within this grid's bounds.
    pub fn contains(&self, lon: f64, lat: f64) -> bool {
        self.bbox.contains(lon, lat)
    }

    /// Stitch multiple adjacent tiles into a single SurfaceGrid.
    ///
    /// Tiles must be in the same projection and have compatible pixel sizes.
    /// The output grid covers the union bounding box. Missing tiles or
    /// gaps are filled with NaN.
    pub fn stitch(tiles: &[SurfaceGrid]) -> Result<Self, TerrainError> {
        if tiles.is_empty() {
            return Err(TerrainError::NoData);
        }
        if tiles.len() == 1 {
            return Ok(tiles[0].clone());
        }

        // Compute union bounding box
        let mut min_lon = f64::INFINITY;
        let mut min_lat = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;
        let mut max_lat = f64::NEG_INFINITY;

        for tile in tiles {
            min_lon = min_lon.min(tile.bbox.min_lon);
            min_lat = min_lat.min(tile.bbox.min_lat);
            max_lon = max_lon.max(tile.bbox.max_lon);
            max_lat = max_lat.max(tile.bbox.max_lat);
        }

        // Use the pixel size from the first tile (assume uniform)
        let pixel_width = tiles[0].elevation.geotransform.pixel_width;
        let pixel_height = tiles[0].elevation.geotransform.pixel_height;

        let total_width = ((max_lon - min_lon) / pixel_width).round() as usize;
        let total_height = ((max_lat - min_lat) / pixel_height.abs()).round() as usize;

        if total_width == 0 || total_height == 0 {
            return Err(TerrainError::BadGeoTransform(format!(
                "stitched grid has zero size: {total_width}x{total_height}"
            )));
        }

        let mut elevations = vec![f32::NAN; total_width * total_height];

        // Blit each tile into the output grid
        for tile in tiles {
            let t = &tile.elevation;
            // Offset of this tile within the stitched grid (in pixels)
            let col_offset = ((t.geotransform.origin_x - min_lon) / pixel_width).round() as i64;
            let row_offset =
                ((max_lat - t.geotransform.origin_y) / pixel_height.abs()).round() as i64;

            for row in 0..t.height {
                for col in 0..t.width {
                    let dst_col = col_offset + col as i64;
                    let dst_row = row_offset + row as i64;

                    if dst_col < 0
                        || dst_col >= total_width as i64
                        || dst_row < 0
                        || dst_row >= total_height as i64
                    {
                        continue;
                    }

                    let val = t.at(col, row);
                    if !val.is_nan() {
                        elevations[dst_row as usize * total_width + dst_col as usize] = val;
                    }
                }
            }
        }

        let geotransform = GeoTransform {
            origin_x: min_lon,
            pixel_width,
            origin_y: max_lat,
            pixel_height,
        };

        let elevation = ElevationGrid::new(total_width, total_height, geotransform, elevations)?;

        Ok(Self {
            elevation,
            bbox: BoundingBox::new(min_lon, min_lat, max_lon, max_lat),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid(
        width: usize,
        height: usize,
        origin_lon: f64,
        origin_lat: f64,
        pixel_size: f64,
        value: f32,
    ) -> SurfaceGrid {
        let gt = GeoTransform {
            origin_x: origin_lon,
            pixel_width: pixel_size,
            origin_y: origin_lat,
            pixel_height: -pixel_size,
        };
        let elevation = ElevationGrid::new(width, height, gt, vec![value; width * height]).unwrap();
        SurfaceGrid::from_elevation(elevation)
    }

    #[test]
    fn test_bbox_from_geotransform() {
        let grid = make_grid(10, 5, -105.0, 40.0, 0.01, 100.0);
        assert!((grid.bbox.min_lon - (-105.0)).abs() < 0.001);
        assert!((grid.bbox.max_lon - (-104.9)).abs() < 0.001);
        assert!((grid.bbox.max_lat - 40.0).abs() < 0.001);
        assert!((grid.bbox.min_lat - 39.95).abs() < 0.001);
    }

    #[test]
    fn test_elevation_at_geo_point() {
        let grid = make_grid(10, 10, -105.0, 40.0, 0.01, 500.0);
        let z = grid.elevation_at(-104.95, 39.97);
        assert!((z - 500.0).abs() < 0.1);
    }

    #[test]
    fn test_contains() {
        let grid = make_grid(10, 10, -105.0, 40.0, 0.01, 100.0);
        assert!(grid.contains(-104.95, 39.97));
        assert!(!grid.contains(-106.0, 39.97));
        assert!(!grid.contains(-104.95, 41.0));
    }

    #[test]
    fn test_stitch_horizontal() {
        // Two tiles side by side: [0,1] and [1,2] in lon
        let tile_a = make_grid(4, 4, 0.0, 4.0, 1.0, 100.0);
        let tile_b = make_grid(4, 4, 4.0, 4.0, 1.0, 200.0);

        let stitched = SurfaceGrid::stitch(&[tile_a, tile_b]).unwrap();

        assert_eq!(stitched.elevation.width, 8);
        assert_eq!(stitched.elevation.height, 4);

        // Left half should be 100.0
        assert!((stitched.elevation.at(0, 0) - 100.0).abs() < 0.1);
        assert!((stitched.elevation.at(3, 0) - 100.0).abs() < 0.1);

        // Right half should be 200.0
        assert!((stitched.elevation.at(4, 0) - 200.0).abs() < 0.1);
        assert!((stitched.elevation.at(7, 0) - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_stitch_vertical() {
        // Two tiles stacked: top [0,4] and bottom [0,0] in lat
        let tile_a = make_grid(4, 4, 0.0, 4.0, 1.0, 100.0);
        let tile_b = make_grid(4, 4, 0.0, 0.0, 1.0, 200.0);

        let stitched = SurfaceGrid::stitch(&[tile_a, tile_b]).unwrap();

        assert_eq!(stitched.elevation.width, 4);
        assert_eq!(stitched.elevation.height, 8);

        // Top rows should be 100.0 (origin_y=4, so top of grid)
        assert!((stitched.elevation.at(0, 0) - 100.0).abs() < 0.1);
        // Bottom rows should be 200.0
        assert!((stitched.elevation.at(0, 7) - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_stitch_single_tile() {
        let tile = make_grid(4, 4, 0.0, 4.0, 1.0, 100.0);
        let stitched = SurfaceGrid::stitch(std::slice::from_ref(&tile)).unwrap();
        assert_eq!(stitched.elevation.width, 4);
        assert_eq!(stitched.elevation.height, 4);
    }

    #[test]
    fn test_stitch_empty() {
        let result = SurfaceGrid::stitch(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_stitch_2x2() {
        // 2x2 arrangement of tiles with distinct values
        let tl = make_grid(2, 2, 0.0, 4.0, 1.0, 10.0);
        let tr = make_grid(2, 2, 2.0, 4.0, 1.0, 20.0);
        let bl = make_grid(2, 2, 0.0, 2.0, 1.0, 30.0);
        let br = make_grid(2, 2, 2.0, 2.0, 1.0, 40.0);

        let stitched = SurfaceGrid::stitch(&[tl, tr, bl, br]).unwrap();

        assert_eq!(stitched.elevation.width, 4);
        assert_eq!(stitched.elevation.height, 4);

        // Top-left: 10, Top-right: 20, Bottom-left: 30, Bottom-right: 40
        assert!((stitched.elevation.at(0, 0) - 10.0).abs() < 0.1);
        assert!((stitched.elevation.at(3, 0) - 20.0).abs() < 0.1);
        assert!((stitched.elevation.at(0, 3) - 30.0).abs() < 0.1);
        assert!((stitched.elevation.at(3, 3) - 40.0).abs() < 0.1);
    }
}
