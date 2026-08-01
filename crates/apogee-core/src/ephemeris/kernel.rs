//! SPICE binary kernel (.bsp) loader — DAF + SPK Type 3 parser.
//!
//! Parses the Double Precision Array File (DAF) envelope used by NAIF/SPICE
//! binary kernels, then extracts Type 3 (Chebyshev position and velocity)
//! segment summaries and evaluates body states from on-disk Chebyshev
//! coefficients.
//!
//! # DAF file format overview
//!
//! A DAF file is a sequence of 1024-byte records:
//!
//! 1. **File record** (record 1): file ID, internal name, forward/backward
//!    record pointers, free record pointer, endianness flag, number format.
//! 2. **Comment records** (optional): between the file record and the first
//!    summary record.
//! 3. **Summary records**: each contains a linked list of summary items.
//!    Each summary item is a fixed-size pair of (doubles, integers) describing
//!    one SPK segment.
//! 4. **Name records**: follow the summary records and hold the segment names.
//! 5. **Data records**: follow the name records and hold the Chebyshev
//!    coefficients.
//!
//! Record indexing in DAF is 1-based.
//!
//! # SPK Type 3 data record layout
//!
//! Each Type 3 logical record occupies one 1024-byte DAF record:
//!
//! ```text
//! [ begin_et (8 bytes) ][ end_et (8 bytes) ]
//! [ x Chebyshev coefficients (rsize * 8 bytes) ]
//! [ y Chebyshev coefficients (rsize * 8 bytes) ]
//! [ z Chebyshev coefficients (rsize * 8 bytes) ]
//! unused padding to 1024 bytes
//! ```
//!
//! where `rsize = (1024 - 16) / 24`. The begin/end epochs are stored only in
//! the first data record in this minimal implementation; subsequent records
//! are assumed to be contiguous and of equal length.
//!
//! # References
//!
//! - NAIF/SPICE "DAF Required Reading"
//!   <https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/daf.html>
//! - NAIF/SPICE "SPK Required Reading"
//!   <https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html>
//! - Newhall, X X (1989), "The Numerical Representation of Planets and
//!   Satellites", JPL IOM 89-032
//! - Acton, C.H. (1996), "Ancillary Data Services of NASA's Navigation and
//!   Ancillary Information Facility", Planet. Space Sci. 44, 65-70
//!   <https://doi.org/10.1016/0032-0633(95)00107-7>

use apogee_common::{ApogeeError, ApogeeResult, NaifId};

/// Size of a DAF record in bytes.
const RECORD_SIZE: usize = 1024;

/// Size of the file-record ID word and name area.
const IDWORD_LEN: usize = 8;
const INTERNAL_NAME_LEN: usize = 60;
const ND_LEN: usize = 4;
const NI_LEN: usize = 4;
const FWARD_LEN: usize = 4;
const BWARD_LEN: usize = 4;
const FREE_LEN: usize = 4;
const LOCFMT_LEN: usize = 8;

/// Parsed DAF file record.
#[derive(Debug, Clone, PartialEq)]
pub struct DafFileRecord {
    /// File type identifier, e.g. "DAF/SPK".
    pub idword: String,
    /// Internal file name.
    pub internal_name: String,
    /// Number of double-precision summary components.
    pub nd: i32,
    /// Number of integer summary components.
    pub ni: i32,
    /// First summary record number (1-based).
    pub first_summary_record: i32,
    /// Last summary record number (1-based).
    pub last_summary_record: i32,
    /// First free record number (1-based).
    pub first_free_record: i32,
    /// Endianness flag read from the file record.
    pub endianness: Endianness,
}

/// Endianness detected from a DAF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// Summary of a single SPK segment within a DAF.
#[derive(Debug, Clone, PartialEq)]
pub struct SpkSegment {
    /// Start epoch of the segment (seconds past J2000 TDB, SPICE ephemeris time).
    pub start_et: f64,
    /// End epoch of the segment (seconds past J2000 TDB).
    pub end_et: f64,
    /// Target body NAIF ID.
    pub target_id: NaifId,
    /// Center body NAIF ID.
    pub center_id: NaifId,
    /// Reference frame ID (e.g. 1 = J2000).
    pub frame_id: i32,
    /// SPK data type (we only support Type 3 in this phase).
    pub spk_type: i32,
    /// Number of data records in the segment.
    pub record_count: i32,
    /// First data record number (1-based, DAF record index).
    pub first_data_record: i32,
}

/// Body state from ephemeris.
#[derive(Debug, Clone)]
pub struct BodyState {
    pub position: apogee_common::Position,
    pub velocity: apogee_common::Velocity,
}

/// Descriptor for a body in the ephemeris.
#[derive(Debug, Clone)]
pub struct BodyDescriptor {
    pub naif_id: NaifId,
    pub name: String,
    pub center: NaifId,
}

/// Solar system state: all bodies at a single epoch.
#[derive(Debug, Clone, Default)]
pub struct SolarSystemState {
    pub states: Vec<BodyState>,
}

/// Loaded SPICE ephemeris kernel.
#[derive(Debug, Clone, PartialEq)]
pub struct Kernel {
    file_record: DafFileRecord,
    segments: Vec<SpkSegment>,
    // Raw bytes are kept so later steps can read coefficient data on demand.
    data: Vec<u8>,
}

impl Kernel {
    /// Load a binary SPK (.bsp) kernel from a byte slice.
    ///
    /// Parses the DAF file record and all SPK Type 3 segment summaries.
    /// No coefficient data is read yet.
    pub fn from_bytes(bytes: &[u8]) -> ApogeeResult<Self> {
        if bytes.len() < RECORD_SIZE {
            return Err(ApogeeError::Ephemeris(
                "SPK file too short to contain a DAF file record".into(),
            ));
        }

        let file_record = parse_file_record(bytes)?;
        let segments = parse_segments(bytes, &file_record)?;

        Ok(Self {
            file_record,
            segments,
            data: bytes.to_vec(),
        })
    }

    /// Load a binary SPK (.bsp) kernel from a file path.
    pub fn load(path: &str) -> ApogeeResult<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| ApogeeError::Ephemeris(format!("failed to read SPK file: {e}")))?;
        Self::from_bytes(&bytes)
    }

    /// Return a reference to the parsed DAF file record.
    pub fn file_record(&self) -> &DafFileRecord {
        &self.file_record
    }

    /// Return all parsed segments.
    pub fn segments(&self) -> &[SpkSegment] {
        &self.segments
    }

    /// Find a segment covering the given epoch for a target body.
    ///
    /// `epoch_et` is seconds past J2000 TDB. Returns the first matching
    /// segment or `None` if no segment covers the body/epoch.
    pub fn find_segment(&self, target_id: NaifId, epoch_et: f64) -> Option<&SpkSegment> {
        self.segments
            .iter()
            .find(|s| s.target_id == target_id && s.start_et <= epoch_et && epoch_et <= s.end_et)
    }

    /// Evaluate position and velocity for `body` at `epoch_et`.
    ///
    /// `epoch_et` is seconds past J2000 TDB. This implementation supports
    /// SPK Type 3 segments with equal-length Chebyshev records.
    pub fn state_at(&self, body: NaifId, epoch_et: f64) -> ApogeeResult<BodyState> {
        let segment = self.find_segment(body, epoch_et).ok_or_else(|| {
            ApogeeError::Ephemeris(format!(
                "no SPK segment covers body {body} at epoch {epoch_et}"
            ))
        })?;

        if segment.spk_type != 3 {
            return Err(ApogeeError::Ephemeris(format!(
                "unsupported SPK data type {} for body {body}",
                segment.spk_type
            )));
        }

        // Read metadata from the first data record if not already cached.
        let (initial_epoch, interval_length, rsize) = self.read_segment_metadata(segment)?;

        if interval_length <= 0.0 {
            return Err(ApogeeError::Ephemeris(
                "invalid SPK Type 3 interval length".into(),
            ));
        }

        let record_index_f = (epoch_et - initial_epoch) / interval_length;
        let record_index = record_index_f.floor() as i32;
        if record_index < 0 || record_index >= segment.record_count {
            return Err(ApogeeError::Ephemeris(format!(
                "epoch {epoch_et} falls outside segment data records"
            )));
        }

        // Read coefficients for this record.
        let coeffs = self.read_record_coefficients(segment, record_index, rsize)?;

        // Map physical time to Chebyshev domain [-1, 1].
        let record_start = initial_epoch + record_index as f64 * interval_length;
        let mid = record_start + interval_length * 0.5;
        let radius = interval_length * 0.5;
        let x = crate::ephemeris::chebyshev::normalized_time(epoch_et, mid, radius);

        let (position, velocity_normalized) =
            crate::ephemeris::chebyshev::chebyshev_eval_3d(x, [&coeffs[0], &coeffs[1], &coeffs[2]]);

        // Convert normalized velocity to physical units.
        let scale = 2.0 / interval_length;
        Ok(BodyState {
            position,
            velocity: velocity_normalized * scale,
        })
    }

    /// Read Type 3 segment metadata from the first data record.
    ///
    /// SPK Type 3 uses fixed-size DAF records. The first 16 bytes of the
    /// first data record hold the begin and end epoch of the first logical
    /// record; the remaining bytes hold Chebyshev coefficients. The number
    /// of coefficients per coordinate is inferred from the record size.
    fn read_segment_metadata(&self, segment: &SpkSegment) -> ApogeeResult<(f64, f64, usize)> {
        let record_num = segment.first_data_record;
        if record_num <= 0 {
            return Err(ApogeeError::Ephemeris(
                "invalid first data record number".into(),
            ));
        }

        let offset = (record_num as usize - 1) * RECORD_SIZE;
        if offset + 16 > self.data.len() {
            return Err(ApogeeError::Ephemeris(
                "segment metadata extends past end of file".into(),
            ));
        }

        let read_f64 = |o: usize| read_f64_at(&self.data, offset + o, self.file_record.endianness);

        let initial_epoch = read_f64(0);
        let first_record_end = read_f64(8);
        let interval_length = first_record_end - initial_epoch;

        // Each record: begin(8) + end(8) + 3 * rsize * 8 bytes.
        let rsize = (RECORD_SIZE - 16) / 24;
        if rsize == 0 {
            return Err(ApogeeError::Ephemeris(
                "invalid SPK Type 3 coefficient count".into(),
            ));
        }

        Ok((initial_epoch, interval_length, rsize))
    }

    /// Read Chebyshev coefficients for one Type 3 data record.
    fn read_record_coefficients(
        &self,
        segment: &SpkSegment,
        record_index: i32,
        rsize: usize,
    ) -> ApogeeResult<[Vec<f64>; 3]> {
        let record_num = segment.first_data_record + record_index;
        let offset = (record_num as usize - 1) * RECORD_SIZE;

        let header_size = 16usize;
        let coeff_block_size = rsize * 8;
        let record_data_size = header_size + 3 * coeff_block_size;
        if offset + record_data_size > self.data.len() {
            return Err(ApogeeError::Ephemeris(
                "coefficient data extends past end of file".into(),
            ));
        }

        let read = |o: usize| read_f64_at(&self.data, offset + o, self.file_record.endianness);

        let mut cx = vec![0.0; rsize];
        let mut cy = vec![0.0; rsize];
        let mut cz = vec![0.0; rsize];

        for i in 0..rsize {
            cx[i] = read(header_size + i * 8);
            cy[i] = read(header_size + coeff_block_size + i * 8);
            cz[i] = read(header_size + 2 * coeff_block_size + i * 8);
        }

        Ok([cx, cy, cz])
    }
}

/// Parse the DAF file record from the first 1024 bytes.
fn parse_file_record(bytes: &[u8]) -> ApogeeResult<DafFileRecord> {
    let idword = String::from_utf8_lossy(&bytes[0..IDWORD_LEN])
        .trim()
        .to_string();
    if !idword.starts_with("DAF") {
        return Err(ApogeeError::Ephemeris(format!(
            "not a DAF file: idword is '{}'",
            idword
        )));
    }

    let internal_name = String::from_utf8_lossy(&bytes[IDWORD_LEN..IDWORD_LEN + INTERNAL_NAME_LEN])
        .trim()
        .to_string();

    let endianness = detect_endianness(bytes)?;
    let read_i32 = |offset: usize| read_i32_at(bytes, offset, endianness);

    let nd = read_i32(IDWORD_LEN + INTERNAL_NAME_LEN);
    let ni = read_i32(IDWORD_LEN + INTERNAL_NAME_LEN + ND_LEN);

    let fward = read_i32(IDWORD_LEN + INTERNAL_NAME_LEN + ND_LEN + NI_LEN);
    let bward = read_i32(IDWORD_LEN + INTERNAL_NAME_LEN + ND_LEN + NI_LEN + FWARD_LEN);
    let free = read_i32(IDWORD_LEN + INTERNAL_NAME_LEN + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN);

    let locfmt = String::from_utf8_lossy(
        &bytes[IDWORD_LEN + INTERNAL_NAME_LEN + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN + FREE_LEN
            ..IDWORD_LEN
                + INTERNAL_NAME_LEN
                + ND_LEN
                + NI_LEN
                + FWARD_LEN
                + BWARD_LEN
                + FREE_LEN
                + LOCFMT_LEN],
    )
    .trim()
    .to_string();

    let endianness_from_locfmt = match locfmt.as_str() {
        "LITTLE-IEEE" => Endianness::Little,
        "BIG-IEEE" => Endianness::Big,
        _ => {
            // If locfmt is empty or unexpected, fall back to the byte-order test.
            endianness
        }
    };

    Ok(DafFileRecord {
        idword,
        internal_name,
        nd,
        ni,
        first_summary_record: fward,
        last_summary_record: bward,
        first_free_record: free,
        endianness: endianness_from_locfmt,
    })
}

/// Detect endianness from the DAF file record.
///
/// The number of double and integer summary components are stored as i32s
/// right after the internal file name. For SPK kernels the standard pair is
/// (nd=2, ni=6). We read the values as both little- and big-endian and pick
/// the interpretation that matches this pair.
fn detect_endianness(bytes: &[u8]) -> ApogeeResult<Endianness> {
    let nd_offset = IDWORD_LEN + INTERNAL_NAME_LEN;
    let ni_offset = nd_offset + ND_LEN;

    let nd_le = i32::from_le_bytes([
        bytes[nd_offset],
        bytes[nd_offset + 1],
        bytes[nd_offset + 2],
        bytes[nd_offset + 3],
    ]);
    let ni_le = i32::from_le_bytes([
        bytes[ni_offset],
        bytes[ni_offset + 1],
        bytes[ni_offset + 2],
        bytes[ni_offset + 3],
    ]);

    let nd_be = i32::from_be_bytes([
        bytes[nd_offset],
        bytes[nd_offset + 1],
        bytes[nd_offset + 2],
        bytes[nd_offset + 3],
    ]);
    let ni_be = i32::from_be_bytes([
        bytes[ni_offset],
        bytes[ni_offset + 1],
        bytes[ni_offset + 2],
        bytes[ni_offset + 3],
    ]);

    if nd_le == 2 && ni_le == 6 {
        Ok(Endianness::Little)
    } else if nd_be == 2 && ni_be == 6 {
        Ok(Endianness::Big)
    } else {
        Err(ApogeeError::Ephemeris(format!(
            "unable to detect DAF endianness: le=(nd={nd_le}, ni={ni_le}), be=(nd={nd_be}, ni={ni_be})"
        )))
    }
}

fn read_i32_at(bytes: &[u8], offset: usize, endianness: Endianness) -> i32 {
    let b = [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ];
    match endianness {
        Endianness::Little => i32::from_le_bytes(b),
        Endianness::Big => i32::from_be_bytes(b),
    }
}

fn read_f64_at(bytes: &[u8], offset: usize, endianness: Endianness) -> f64 {
    let b = [
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ];
    match endianness {
        Endianness::Little => f64::from_le_bytes(b),
        Endianness::Big => f64::from_be_bytes(b),
    }
}

/// Parse all segment summaries from the DAF summary/name records.
fn parse_segments(bytes: &[u8], file_record: &DafFileRecord) -> ApogeeResult<Vec<SpkSegment>> {
    let nd = file_record.nd.max(0) as usize;
    let ni = file_record.ni.max(0) as usize;
    let summary_size = nd * 8 + ni * 4; // bytes per summary item
    if summary_size == 0 {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let mut current_record = file_record.first_summary_record;
    let read_i32_abs = |offset: usize| read_i32_at(bytes, offset, file_record.endianness);

    while current_record > 0 {
        let record_offset = (current_record as usize - 1) * RECORD_SIZE;
        if record_offset + RECORD_SIZE > bytes.len() {
            return Err(ApogeeError::Ephemeris(format!(
                "summary record {current_record} extends past end of file"
            )));
        }

        let next_summary_record = read_i32_abs(record_offset);
        let prev_summary_record = read_i32_abs(record_offset + 4);
        let n_summary = read_i32_abs(record_offset + 8);
        let _ = (next_summary_record, prev_summary_record);

        let data_start = record_offset + 16;
        let data_len = n_summary as usize * summary_size;
        if data_start + data_len > bytes.len() {
            return Err(ApogeeError::Ephemeris(format!(
                "summary data in record {current_record} extends past end of file"
            )));
        }

        for i in 0..n_summary as usize {
            let offset = data_start + i * summary_size;
            let summary = &bytes[offset..offset + summary_size];
            let read_f64 = |o: usize| read_f64_at(summary, o, file_record.endianness);
            let read_i32 = |o: usize| read_i32_at(summary, o, file_record.endianness);
            let seg = parse_spk_type3_summary(summary, nd, ni, read_f64, read_i32)?;
            segments.push(seg);
        }

        current_record = next_summary_record;
    }

    Ok(segments)
}

/// Parse one SPK Type 3 summary item.
fn parse_spk_type3_summary(
    _summary: &[u8],
    nd: usize,
    _ni: usize,
    mut read_f64: impl FnMut(usize) -> f64,
    mut read_i32: impl FnMut(usize) -> i32,
) -> ApogeeResult<SpkSegment> {
    // SPK Type 3 summary layout (nd=2, ni=6):
    // doubles[0] = start epoch (et)
    // doubles[1] = end epoch (et)
    // ints[0]    = target ID
    // ints[1]    = center ID
    // ints[2]    = frame ID
    // ints[3]    = SPK type
    // ints[4]    = first data record number (1-based)
    // ints[5]    = last data record number (1-based)

    if nd < 2 {
        return Err(ApogeeError::Ephemeris(format!(
            "SPK summary has insufficient double components: nd={nd}"
        )));
    }

    let start_et = read_f64(0);
    let end_et = read_f64(8);
    let target_id = read_i32(nd * 8) as NaifId;
    let center_id = read_i32(nd * 8 + 4) as NaifId;
    let frame_id = read_i32(nd * 8 + 8);
    let spk_type = read_i32(nd * 8 + 12);
    let first_data_record = read_i32(nd * 8 + 16);
    let last_data_record = read_i32(nd * 8 + 20);

    Ok(SpkSegment {
        start_et,
        end_et,
        target_id,
        center_id,
        frame_id,
        spk_type,
        record_count: last_data_record - first_data_record + 1,
        first_data_record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// Build a minimal DAF file record for little-endian IEEE.
    fn minimal_daf_header(idword: &str, fward: i32, bward: i32, free: i32) -> Vec<u8> {
        let mut bytes = vec![0u8; RECORD_SIZE];
        bytes[0..IDWORD_LEN].copy_from_slice(&pad_str(idword, IDWORD_LEN).into_bytes());
        bytes[IDWORD_LEN..IDWORD_LEN + INTERNAL_NAME_LEN]
            .copy_from_slice(&pad_str("TEST", INTERNAL_NAME_LEN).into_bytes());

        let nd_offset = IDWORD_LEN + INTERNAL_NAME_LEN;
        bytes[nd_offset..nd_offset + ND_LEN].copy_from_slice(&2i32.to_le_bytes());
        bytes[nd_offset + ND_LEN..nd_offset + ND_LEN + NI_LEN].copy_from_slice(&6i32.to_le_bytes());
        bytes[nd_offset + ND_LEN + NI_LEN..nd_offset + ND_LEN + NI_LEN + FWARD_LEN]
            .copy_from_slice(&pad_i32_le(fward));
        bytes[nd_offset + ND_LEN + NI_LEN + FWARD_LEN
            ..nd_offset + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN]
            .copy_from_slice(&pad_i32_le(bward));
        bytes[nd_offset + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN
            ..nd_offset + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN + FREE_LEN]
            .copy_from_slice(&pad_i32_le(free));
        bytes[nd_offset + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN + FREE_LEN
            ..nd_offset + ND_LEN + NI_LEN + FWARD_LEN + BWARD_LEN + FREE_LEN + LOCFMT_LEN]
            .copy_from_slice(&pad_str("LITTLE-IEEE", LOCFMT_LEN).into_bytes());

        bytes
    }

    fn pad_str(s: &str, len: usize) -> String {
        let mut out = s.to_string();
        out.truncate(len);
        while out.len() < len {
            out.push(' ');
        }
        out
    }

    fn pad_i32_le(v: i32) -> [u8; 4] {
        v.to_le_bytes()
    }

    /// Build a complete SPK Type 3 segment fixture.
    ///
    /// The fixture contains a file record, one summary record, and one or more
    /// data records. The coefficient generator receives the normalized Chebyshev
    /// time `x` in [-1, 1] and returns the physical position for that record.
    fn build_type3_fixture<F>(
        target_id: i32,
        start_et: f64,
        end_et: f64,
        record_count: i32,
        mut position_fn: F,
    ) -> Vec<u8>
    where
        F: FnMut(f64) -> [f64; 3],
    {
        let first_data_record = 3; // file record + summary record = records 1 and 2
        let last_data_record = first_data_record + record_count - 1;
        let total_records = last_data_record;
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, total_records + 1);
        bytes.resize(RECORD_SIZE * total_records as usize, 0);

        // Summary record at record 2.
        let summary_offset = RECORD_SIZE;
        bytes[summary_offset..summary_offset + 4].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 4..summary_offset + 8].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 8..summary_offset + 12].copy_from_slice(&pad_i32_le(1));

        let data_offset = summary_offset + 16;
        bytes[data_offset..data_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&end_et.to_le_bytes());
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&target_id.to_le_bytes());
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&3i32.to_le_bytes());
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&first_data_record.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&last_data_record.to_le_bytes());

        let rsize = (RECORD_SIZE - 16) / 24;
        let interval_length = (end_et - start_et) / record_count as f64;

        for rec in 0..record_count {
            let record_num = first_data_record + rec;
            let offset = (record_num as usize - 1) * RECORD_SIZE;
            let rec_start = start_et + rec as f64 * interval_length;
            let rec_end = rec_start + interval_length;
            bytes[offset..offset + 8].copy_from_slice(&rec_start.to_le_bytes());
            bytes[offset + 8..offset + 16].copy_from_slice(&rec_end.to_le_bytes());

            // Generate coefficients by sampling position_fn at Chebyshev nodes.
            let n = rsize;
            let mut samples = vec![[0.0; 3]; n];
            for i in 0..n {
                let x = ((2 * i + 1) as f64 * std::f64::consts::PI / (2.0 * n as f64)).cos();
                samples[i] = position_fn(x);
            }

            // Convert samples to Chebyshev coefficients via DCT-I-like projection.
            // For exactness we use the discrete orthogonality of T_k at the
            // Chebyshev nodes x_j = cos((2j+1)π/(2N)).
            let mut coeffs = vec![[0.0; 3]; n];
            for k in 0..n {
                let mut sum = [0.0; 3];
                for (j, sample) in samples.iter().enumerate() {
                    let x_j = ((2 * j + 1) as f64 * std::f64::consts::PI / (2.0 * n as f64)).cos();
                    let t_k = crate::ephemeris::chebyshev::chebyshev_polynomial(k, x_j);
                    sum[0] += sample[0] * t_k;
                    sum[1] += sample[1] * t_k;
                    sum[2] += sample[2] * t_k;
                }
                let scale = if k == 0 {
                    1.0 / n as f64
                } else {
                    2.0 / n as f64
                };
                coeffs[k][0] = sum[0] * scale;
                coeffs[k][1] = sum[1] * scale;
                coeffs[k][2] = sum[2] * scale;
            }

            for i in 0..n {
                let cx_offset = offset + 16 + i * 8;
                let cy_offset = offset + 16 + rsize * 8 + i * 8;
                let cz_offset = offset + 16 + 2 * rsize * 8 + i * 8;
                bytes[cx_offset..cx_offset + 8].copy_from_slice(&coeffs[i][0].to_le_bytes());
                bytes[cy_offset..cy_offset + 8].copy_from_slice(&coeffs[i][1].to_le_bytes());
                bytes[cz_offset..cz_offset + 8].copy_from_slice(&coeffs[i][2].to_le_bytes());
            }
        }

        bytes
    }

    #[test]
    fn test_detect_little_endian() {
        let bytes = minimal_daf_header("DAF/SPK", 0, 0, 0);
        let endianness = detect_endianness(&bytes).unwrap();
        assert_eq!(endianness, Endianness::Little);
    }

    #[test]
    fn test_detect_big_endian() {
        let mut bytes = minimal_daf_header("DAF/SPK", 0, 0, 0);
        let nd_offset = IDWORD_LEN + INTERNAL_NAME_LEN;
        bytes[nd_offset..nd_offset + ND_LEN].copy_from_slice(&2i32.to_be_bytes());
        bytes[nd_offset + ND_LEN..nd_offset + ND_LEN + NI_LEN].copy_from_slice(&6i32.to_be_bytes());
        let endianness = detect_endianness(&bytes).unwrap();
        assert_eq!(endianness, Endianness::Big);
    }

    #[test]
    fn test_parse_file_record() {
        let bytes = minimal_daf_header("DAF/SPK", 2, 2, 3);
        let rec = parse_file_record(&bytes).unwrap();
        assert_eq!(rec.idword, "DAF/SPK");
        assert_eq!(rec.internal_name, "TEST");
        assert_eq!(rec.nd, 2);
        assert_eq!(rec.ni, 6);
        assert_eq!(rec.first_summary_record, 2);
        assert_eq!(rec.last_summary_record, 2);
        assert_eq!(rec.first_free_record, 3);
        assert_eq!(rec.endianness, Endianness::Little);
    }

    #[test]
    fn test_reject_non_daf() {
        let mut bytes = minimal_daf_header("NOTDAF! ", 0, 0, 0);
        bytes[0..8].copy_from_slice(b"NOTDAF! ");
        let err = parse_file_record(&bytes).unwrap_err();
        assert!(err.to_string().contains("not a DAF file"));
    }

    #[test]
    fn test_parse_single_segment_summary() {
        // File record (1024 bytes) + summary record (1024 bytes).
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, 3);
        bytes.resize(RECORD_SIZE * 2, 0);

        let summary_offset = RECORD_SIZE;
        bytes[summary_offset..summary_offset + 4].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 4..summary_offset + 8].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 8..summary_offset + 12].copy_from_slice(&pad_i32_le(1));

        // Summary data: nd=2 doubles + ni=6 ints = 40 bytes.
        let data_offset = summary_offset + 16;
        let start_et = 0.0f64;
        let end_et = 86400.0f64;
        bytes[data_offset..data_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&end_et.to_le_bytes());

        let target_id: i32 = 499; // Mars
        let center_id: i32 = 10;
        let frame_id: i32 = 1;
        let spk_type: i32 = 3;
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&target_id.to_le_bytes());
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&center_id.to_le_bytes());
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&frame_id.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&spk_type.to_le_bytes());
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&3i32.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&3i32.to_le_bytes());

        let kernel = Kernel::from_bytes(&bytes).unwrap();
        assert_eq!(kernel.segments.len(), 1);

        let seg = &kernel.segments[0];
        assert_eq!(seg.target_id, 499);
        assert_eq!(seg.center_id, 10);
        assert_eq!(seg.frame_id, 1);
        assert_eq!(seg.spk_type, 3);
        assert_eq!(seg.record_count, 1);
        assert_eq!(seg.first_data_record, 3);
        assert!((seg.start_et - start_et).abs() < 1e-9);
        assert!((seg.end_et - end_et).abs() < 1e-9);
    }

    #[test]
    fn test_find_segment_by_target_and_epoch() {
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, 3);
        bytes.resize(RECORD_SIZE * 2, 0);

        let summary_offset = RECORD_SIZE;
        bytes[summary_offset..summary_offset + 4].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 4..summary_offset + 8].copy_from_slice(&pad_i32_le(0));
        bytes[summary_offset + 8..summary_offset + 12].copy_from_slice(&pad_i32_le(1));

        let data_offset = summary_offset + 16;
        bytes[data_offset..data_offset + 8].copy_from_slice(&100.0f64.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&200.0f64.to_le_bytes());
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&4i32.to_le_bytes()); // Mars barycenter
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&1i32.to_le_bytes()); // center
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&3i32.to_le_bytes());
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&3i32.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&3i32.to_le_bytes());

        let kernel = Kernel::from_bytes(&bytes).unwrap();
        assert!(kernel.find_segment(4, 150.0).is_some());
        assert!(kernel.find_segment(4, 50.0).is_none());
        assert!(kernel.find_segment(5, 150.0).is_none());
    }

    #[test]
    fn test_state_at_constant_position() {
        let start = 0.0;
        let end = 86400.0;
        let fixture = build_type3_fixture(499, start, end, 1, |_x| [1.0, 2.0, 3.0]);

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, 43200.0).unwrap();

        assert_relative_eq!(state.position.x, 1.0, epsilon = 1e-9);
        assert_relative_eq!(state.position.y, 2.0, epsilon = 1e-9);
        assert_relative_eq!(state.position.z, 3.0, epsilon = 1e-9);
        assert_relative_eq!(state.velocity.x, 0.0, epsilon = 1e-9);
        assert_relative_eq!(state.velocity.y, 0.0, epsilon = 1e-9);
        assert_relative_eq!(state.velocity.z, 0.0, epsilon = 1e-9);
    }

    #[test]
    fn test_state_at_linear_trajectory() {
        // Position p(t) = [t, 2t, 3t] over one day.
        // Using normalized time x in [-1, 1]:
        //   t = mid + x * radius
        // Choose interval [0, 86400] -> mid=43200, radius=43200.
        let start = 0.0;
        let end = 86400.0;
        let mid = (start + end) * 0.5;
        let radius = (end - start) * 0.5;
        let fixture = build_type3_fixture(499, start, end, 1, |x| {
            let t = mid + x * radius;
            [t, 2.0 * t, 3.0 * t]
        });

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, 43200.0).unwrap();

        assert_relative_eq!(state.position.x, mid, epsilon = 1e-3);
        assert_relative_eq!(state.position.y, 2.0 * mid, epsilon = 1e-3);
        assert_relative_eq!(state.position.z, 3.0 * mid, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.x, 1.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.y, 2.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.z, 3.0, epsilon = 1e-6);
    }

    #[test]
    fn test_state_at_rejects_uncovered_epoch() {
        let start = 0.0;
        let end = 86400.0;
        let fixture = build_type3_fixture(499, start, end, 1, |_x| [1.0, 2.0, 3.0]);

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        assert!(kernel.state_at(499, -1.0).is_err());
        assert!(kernel.state_at(499, end + 1.0).is_err());
        assert!(kernel.state_at(500, 43200.0).is_err());
    }
}
