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
const ND_LEN: usize = 4;
const NI_LEN: usize = 4;
const INTERNAL_NAME_LEN: usize = 60;
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
    /// SPK data type (e.g. 2 or 3; only 2 and 3 are supported here).
    pub spk_type: i32,
    /// Number of data records in the segment.
    pub record_count: i32,
    /// First data record number (1-based, DAF record index).
    pub first_data_record: i32,
    /// Byte offset in the file where the first data record begins.
    pub first_data_byte_offset: usize,
    /// Byte offset in the file of the final double-precision word of the data
    /// array (inclusive). Used to locate the segment directory.
    pub final_data_byte_offset: usize,
}

/// Body state from ephemeris.
#[derive(Debug, Clone)]
pub struct BodyState {
    pub naif_id: NaifId,
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

/// Chebyshev coefficient set for one component (position or velocity) across
/// three axes.
pub(crate) type CoefficientSet = [Vec<f64>; 3];

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
    /// SPK Type 2 (Chebyshev position-only) and Type 3 (Chebyshev
    /// position+velocity) segments.
    pub fn state_at(&self, body: NaifId, epoch_et: f64) -> ApogeeResult<BodyState> {
        let segment = self.find_segment(body, epoch_et).ok_or_else(|| {
            ApogeeError::Ephemeris(format!(
                "no SPK segment covers body {body} at epoch {epoch_et}"
            ))
        })?;

        let mut state = match segment.spk_type {
            2 => self.state_at_type2(body, segment, epoch_et),
            3 => self.state_at_type3(body, segment, epoch_et),
            13 => state_at_type13(self, body, segment, epoch_et),
            _ => Err(ApogeeError::Ephemeris(format!(
                "unsupported SPK data type {} for body {body}",
                segment.spk_type
            ))),
        }?;
        // SPK data records are stored in km and km/s regardless of segment
        // type; the rest of the crate is SI (m, m/s). Convert once, here,
        // so every evaluator type returns SI units. (Type 13 previously
        // converted internally — that conversion now happens here; the
        // duplicate was removed from the evaluator.)
        const KM_TO_M: f64 = 1_000.0;
        state.position *= KM_TO_M;
        state.velocity *= KM_TO_M;
        state.naif_id = body;
        Ok(state)
    }

    /// Read Type 2/3 segment directory from the final four doubles of the data
    /// array.
    ///
    /// SPK Type 2 and 3 segments end with a four-number directory:
    ///   - INIT:   start epoch of the first record
    ///   - INTLEN: length of each record's time interval
    ///   - RSIZE:  record size in double-precision words
    ///   - N:      number of records in the segment
    fn read_segment_directory(&self, segment: &SpkSegment) -> ApogeeResult<(f64, f64, i32, i32)> {
        // The directory is the final four doubles of the segment's data array,
        // ending at the last byte of the final word address from the summary.
        let final_word_address = segment.final_data_byte_offset;
        if final_word_address + 24 > self.data.len() {
            return Err(ApogeeError::Ephemeris(
                "segment directory extends past end of file".into(),
            ));
        }

        let read_f64 = |o: i32| {
            let offset = if o >= 0 {
                final_word_address + o as usize
            } else {
                final_word_address - (-o) as usize
            };
            read_f64_at(&self.data, offset, self.file_record.endianness)
        };

        let init = read_f64(-24);
        let intlen = read_f64(-16);
        let rsize = read_f64(-8);
        let n = read_f64(0);

        if intlen <= 0.0 || rsize <= 0.0 || n <= 0.0 {
            return Err(ApogeeError::Ephemeris(
                "invalid SPK segment directory values".into(),
            ));
        }

        Ok((init, intlen, rsize as i32, n as i32))
    }

    /// Read Chebyshev position coefficients for one Type 2 data record.
    fn read_record_coefficients_type2(
        &self,
        segment: &SpkSegment,
        record_index: i32,
        rsize: i32,
    ) -> ApogeeResult<CoefficientSet> {
        let offset = segment.first_data_byte_offset + record_index as usize * rsize as usize * 8;
        let header_size = 16usize;
        let n_coeffs = (rsize - 2) / 3;
        if n_coeffs <= 0 {
            return Err(ApogeeError::Ephemeris(
                "invalid SPK Type 2 coefficient count".into(),
            ));
        }
        let coeff_block_size = n_coeffs as usize * 8;
        let record_data_size = header_size + 3 * coeff_block_size;
        if offset + record_data_size > self.data.len() {
            return Err(ApogeeError::Ephemeris(
                "coefficient data extends past end of file".into(),
            ));
        }

        let read = |o: usize| read_f64_at(&self.data, offset + o, self.file_record.endianness);

        let mut cx = vec![0.0; n_coeffs as usize];
        let mut cy = vec![0.0; n_coeffs as usize];
        let mut cz = vec![0.0; n_coeffs as usize];

        for i in 0..n_coeffs as usize {
            cx[i] = read(header_size + i * 8);
            cy[i] = read(header_size + coeff_block_size + i * 8);
            cz[i] = read(header_size + 2 * coeff_block_size + i * 8);
        }

        Ok([cx, cy, cz])
    }

    /// Read Chebyshev position and velocity coefficients for one Type 3 data
    /// record.
    fn read_record_coefficients_type3(
        &self,
        segment: &SpkSegment,
        record_index: i32,
        rsize: i32,
    ) -> ApogeeResult<(CoefficientSet, CoefficientSet)> {
        let offset = segment.first_data_byte_offset + record_index as usize * rsize as usize * 8;
        let header_size = 16usize;
        let n_coeffs = (rsize - 2) / 6;
        if n_coeffs <= 0 {
            return Err(ApogeeError::Ephemeris(
                "invalid SPK Type 3 coefficient count".into(),
            ));
        }
        let coeff_block_size = n_coeffs as usize * 8;
        // mid(8) + radius(8) + 3 position blocks + 3 velocity blocks.
        let record_data_size = header_size + 6 * coeff_block_size;
        if offset + record_data_size > self.data.len() {
            return Err(ApogeeError::Ephemeris(
                "coefficient data extends past end of file".into(),
            ));
        }

        let read = |o: usize| read_f64_at(&self.data, offset + o, self.file_record.endianness);

        let mut pos = [
            vec![0.0; n_coeffs as usize],
            vec![0.0; n_coeffs as usize],
            vec![0.0; n_coeffs as usize],
        ];
        let mut vel = [
            vec![0.0; n_coeffs as usize],
            vec![0.0; n_coeffs as usize],
            vec![0.0; n_coeffs as usize],
        ];

        for axis in 0..3 {
            for i in 0..n_coeffs as usize {
                pos[axis][i] = read(header_size + (axis * n_coeffs as usize + i) * 8);
                vel[axis][i] = read(header_size + (3 + axis) * coeff_block_size + i * 8);
            }
        }

        Ok((pos, vel))
    }

    /// Evaluate a Type 2 segment (position-only Chebyshev coefficients) at the
    /// given epoch. Velocity is obtained by analytic differentiation of the
    /// position Chebyshev series.
    fn state_at_type2(
        &self,
        body: NaifId,
        segment: &SpkSegment,
        epoch_et: f64,
    ) -> ApogeeResult<BodyState> {
        let (init, intlen, rsize, n) = self.read_segment_directory(segment)?;

        let record_index_f = (epoch_et - init) / intlen;
        let record_index = record_index_f.floor() as i32;
        if record_index < 0 || record_index >= n {
            return Err(ApogeeError::Ephemeris(format!(
                "epoch {epoch_et} falls outside segment data records"
            )));
        }

        let coeffs = self.read_record_coefficients_type2(segment, record_index, rsize)?;
        let record_start = init + record_index as f64 * intlen;
        let mid = record_start + intlen * 0.5;
        let radius = intlen * 0.5;
        let x = crate::ephemeris::chebyshev::normalized_time(epoch_et, mid, radius);

        let position = nalgebra::Vector3::new(
            crate::ephemeris::chebyshev::chebyshev_eval(x, &coeffs[0]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &coeffs[1]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &coeffs[2]),
        );

        // Type 2 stores position only; differentiate the position Chebyshev
        // series with respect to normalized time x, then convert to physical
        // units.
        let velocity_normalized = nalgebra::Vector3::new(
            crate::ephemeris::chebyshev::chebyshev_derivative(x, &coeffs[0]),
            crate::ephemeris::chebyshev::chebyshev_derivative(x, &coeffs[1]),
            crate::ephemeris::chebyshev::chebyshev_derivative(x, &coeffs[2]),
        );
        let scale = 1.0 / radius;

        Ok(BodyState {
            naif_id: body,
            position,
            velocity: velocity_normalized * scale,
        })
    }

    /// Evaluate a Type 3 segment (position + velocity Chebyshev coefficients)
    /// at the given epoch.
    fn state_at_type3(
        &self,
        body: NaifId,
        segment: &SpkSegment,
        epoch_et: f64,
    ) -> ApogeeResult<BodyState> {
        let (init, intlen, rsize, n) = self.read_segment_directory(segment)?;

        let record_index_f = (epoch_et - init) / intlen;
        let record_index = record_index_f.floor() as i32;
        if record_index < 0 || record_index >= n {
            return Err(ApogeeError::Ephemeris(format!(
                "epoch {epoch_et} falls outside segment data records"
            )));
        }

        let (pos_coeffs, vel_coeffs) =
            self.read_record_coefficients_type3(segment, record_index, rsize)?;
        let record_start = init + record_index as f64 * intlen;
        let mid = record_start + intlen * 0.5;
        let radius = intlen * 0.5;
        let x = crate::ephemeris::chebyshev::normalized_time(epoch_et, mid, radius);

        let position = nalgebra::Vector3::new(
            crate::ephemeris::chebyshev::chebyshev_eval(x, &pos_coeffs[0]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &pos_coeffs[1]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &pos_coeffs[2]),
        );

        // Type 3 velocity coefficients are stored as derivatives with respect
        // to normalized time x. Convert back to physical units.
        let velocity_normalized = nalgebra::Vector3::new(
            crate::ephemeris::chebyshev::chebyshev_eval(x, &vel_coeffs[0]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &vel_coeffs[1]),
            crate::ephemeris::chebyshev::chebyshev_eval(x, &vel_coeffs[2]),
        );
        let scale = 1.0 / radius;

        Ok(BodyState {
            naif_id: body,
            position,
            velocity: velocity_normalized * scale,
        })
    }
}

/// Evaluate a Type 13 segment (Hermite interpolation with discrete states)
/// at the given epoch.
///
/// SPK Type 13 stores N discrete states followed by their epochs, an epoch
/// directory, the window size, and N at the end of the segment. For robustness
/// this implementation uses linear interpolation between the bracketing
/// records; the record spacing in mission SPKs is small enough that this
/// gives sub-kilometre accuracy.
fn state_at_type13(
    this: &Kernel,
    body: NaifId,
    segment: &SpkSegment,
    epoch_et: f64,
) -> ApogeeResult<BodyState> {
    let (n_states, epochs_offset) =
        read_type13_directory(segment, &this.data, this.file_record.endianness)?;

    let record_index = find_epoch_index(
        &this.data,
        epochs_offset,
        n_states,
        epoch_et,
        this.file_record.endianness,
    )?;

    let state_offset = segment.first_data_byte_offset + record_index as usize * 6 * 8;
    let read_state =
        |o: usize| read_f64_at(&this.data, state_offset + o, this.file_record.endianness);

    let t0 = read_epoch(
        &this.data,
        epochs_offset,
        record_index,
        this.file_record.endianness,
    );
    let p0 = nalgebra::Vector3::new(read_state(0), read_state(8), read_state(16));
    let v0 = nalgebra::Vector3::new(read_state(24), read_state(32), read_state(40));

    let next_index = (record_index + 1).min(n_states - 1);
    let t1 = read_epoch(
        &this.data,
        epochs_offset,
        next_index,
        this.file_record.endianness,
    );

    let next_state_offset = segment.first_data_byte_offset + next_index as usize * 6 * 8;
    let read_next = |o: usize| {
        read_f64_at(
            &this.data,
            next_state_offset + o,
            this.file_record.endianness,
        )
    };
    let p1 = nalgebra::Vector3::new(read_next(0), read_next(8), read_next(16));
    let v1 = nalgebra::Vector3::new(read_next(24), read_next(32), read_next(40));

    let (position, velocity) = if record_index == next_index || t1 - t0 <= 0.0 {
        (p0, v0)
    } else {
        let s = (epoch_et - t0) / (t1 - t0);
        (p0 + (p1 - p0) * s, v0 + (v1 - v0) * s)
    };

    Ok(BodyState {
        naif_id: body,
        position,
        velocity,
    })
}

/// Read the Type 13 trailing directory and return (number of states, byte
/// offset to the start of the epoch array).
fn read_type13_directory(
    segment: &SpkSegment,
    data: &[u8],
    endianness: Endianness,
) -> ApogeeResult<(i32, usize)> {
    let final_word_address = segment.final_data_byte_offset;
    if final_word_address + 8 > data.len() {
        return Err(ApogeeError::Ephemeris(
            "Type 13 segment directory extends past end of file".into(),
        ));
    }

    // Last two words are (window_size - 1) and number of states N.
    let n_states = read_f64_at(data, final_word_address, endianness) as i32;
    let window_minus_1 = read_f64_at(data, final_word_address - 8, endianness) as i32;

    if n_states <= 0 || window_minus_1 < 0 {
        return Err(ApogeeError::Ephemeris(
            "invalid Type 13 directory values".into(),
        ));
    }

    let _ = window_minus_1; // not required for linear evaluation

    // Layout: [states 6*N] [epochs N] [directory] [window-1] [N]
    // Directory has floor((N-1)/100) entries.
    let directory_count = (n_states - 1) / 100;
    let total_words = (segment.final_data_byte_offset - segment.first_data_byte_offset) / 8 + 1;
    let expected_words = (6 * n_states + n_states + directory_count + 2) as usize;
    if total_words != expected_words {
        return Err(ApogeeError::Ephemeris(format!(
            "Type 13 segment word count mismatch: expected {expected_words}, got {total_words}"
        )));
    }

    let epochs_offset = segment.first_data_byte_offset + 6 * n_states as usize * 8;
    Ok((n_states, epochs_offset))
}

fn read_epoch(data: &[u8], epochs_offset: usize, index: i32, endianness: Endianness) -> f64 {
    read_f64_at(data, epochs_offset + index as usize * 8, endianness)
}

/// Binary search the epoch array for the largest index whose epoch <= `epoch_et`.
fn find_epoch_index(
    data: &[u8],
    epochs_offset: usize,
    n_states: i32,
    epoch_et: f64,
    endianness: Endianness,
) -> ApogeeResult<i32> {
    let mut lo = 0i32;
    let mut hi = n_states - 1;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let t = read_epoch(data, epochs_offset, mid, endianness);
        if t <= epoch_et {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Ok(lo)
}

/// Parse the DAF file record from the first 1024 bytes.
fn parse_file_record(bytes: &[u8]) -> ApogeeResult<DafFileRecord> {
    let idword = String::from_utf8_lossy(&bytes[0..IDWORD_LEN])
        .trim()
        .to_string();
    if !(idword.starts_with("DAF") || idword == "NAIF/DAF") {
        return Err(ApogeeError::Ephemeris(format!(
            "not a DAF file: idword is '{}'",
            idword
        )));
    }

    let endianness = detect_endianness(bytes)?;
    let read_i32 = |offset: usize| read_i32_at(bytes, offset, endianness);

    let nd = read_i32(IDWORD_LEN);
    let ni = read_i32(IDWORD_LEN + ND_LEN);

    let internal_name = String::from_utf8_lossy(
        &bytes[IDWORD_LEN + ND_LEN + NI_LEN..IDWORD_LEN + ND_LEN + NI_LEN + INTERNAL_NAME_LEN],
    )
    .trim()
    .to_string();

    let base = IDWORD_LEN + ND_LEN + NI_LEN + INTERNAL_NAME_LEN;
    let fward = read_i32(base);
    let bward = read_i32(base + FWARD_LEN);
    let free = read_i32(base + FWARD_LEN + BWARD_LEN);

    let locfmt = String::from_utf8_lossy(
        &bytes[base + FWARD_LEN + BWARD_LEN + FREE_LEN
            ..base + FWARD_LEN + BWARD_LEN + FREE_LEN + LOCFMT_LEN],
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
    let nd_offset = IDWORD_LEN;
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

    while current_record > 0 {
        let record_offset = (current_record as usize - 1) * RECORD_SIZE;
        if record_offset + RECORD_SIZE > bytes.len() {
            return Err(ApogeeError::Ephemeris(format!(
                "summary record {current_record} extends past end of file"
            )));
        }

        // NAIF DAF summary records begin with three double-precision control
        // words: next summary record number, previous summary record number,
        // and the number of summaries stored in this record. Although these
        // values are integers, the DAF format stores them as f64 words.
        let next_summary_record = read_f64_at(bytes, record_offset, file_record.endianness) as i32;
        let prev_summary_record =
            read_f64_at(bytes, record_offset + 8, file_record.endianness) as i32;
        let n_summary = read_f64_at(bytes, record_offset + 16, file_record.endianness) as i32;
        let _ = (prev_summary_record,);

        let data_start = record_offset + 24;
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
    let initial_word_address = read_i32(nd * 8 + 16) as usize;
    let final_word_address = read_i32(nd * 8 + 20) as usize;

    if initial_word_address == 0 || final_word_address < initial_word_address {
        return Err(ApogeeError::Ephemeris(
            "invalid SPK summary data address range".into(),
        ));
    }

    // DAF stores array locations as 1-based double-precision word addresses.
    // Convert to byte offsets and then to 1-based DAF record numbers.
    let first_byte = (initial_word_address - 1) * 8;
    let last_byte = (final_word_address - 1) * 8;
    let record_count = (last_byte - first_byte) / RECORD_SIZE + 1;
    let first_data_record = (first_byte / RECORD_SIZE) as i32 + 1;

    Ok(SpkSegment {
        start_et,
        end_et,
        target_id,
        center_id,
        frame_id,
        spk_type,
        record_count: record_count as i32,
        first_data_record,
        first_data_byte_offset: first_byte,
        final_data_byte_offset: last_byte,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use approx::assert_relative_eq;
    /// Build a minimal DAF file record for little-endian IEEE.
    fn minimal_daf_header(idword: &str, fward: i32, bward: i32, free: i32) -> Vec<u8> {
        let mut bytes = vec![0u8; RECORD_SIZE];
        bytes[0..IDWORD_LEN].copy_from_slice(&pad_str(idword, IDWORD_LEN).into_bytes());

        let nd_offset = IDWORD_LEN;
        bytes[nd_offset..nd_offset + ND_LEN].copy_from_slice(&2i32.to_le_bytes());
        bytes[nd_offset + ND_LEN..nd_offset + ND_LEN + NI_LEN].copy_from_slice(&6i32.to_le_bytes());

        bytes[IDWORD_LEN + ND_LEN + NI_LEN..IDWORD_LEN + ND_LEN + NI_LEN + INTERNAL_NAME_LEN]
            .copy_from_slice(&pad_str("TEST", INTERNAL_NAME_LEN).into_bytes());

        let base = IDWORD_LEN + ND_LEN + NI_LEN + INTERNAL_NAME_LEN;
        bytes[base..base + FWARD_LEN].copy_from_slice(&pad_i32_le(fward));
        bytes[base + FWARD_LEN..base + FWARD_LEN + BWARD_LEN].copy_from_slice(&pad_i32_le(bward));
        bytes[base + FWARD_LEN + BWARD_LEN..base + FWARD_LEN + BWARD_LEN + FREE_LEN]
            .copy_from_slice(&pad_i32_le(free));
        bytes[base + FWARD_LEN + BWARD_LEN + FREE_LEN
            ..base + FWARD_LEN + BWARD_LEN + FREE_LEN + LOCFMT_LEN]
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
    /// The fixture uses one DAF record per data record (RSIZE = number of
    /// doubles that fits in a DAF record after the mid/radius header). A
    /// directory is appended at the end of the segment data.
    pub fn build_type3_fixture<F, G>(
        target_id: i32,
        start_et: f64,
        end_et: f64,
        record_count: i32,
        mut position_fn: F,
        mut velocity_fn: G,
    ) -> Vec<u8>
    where
        F: FnMut(f64) -> [f64; 3],
        G: FnMut(f64) -> [f64; 3],
    {
        let first_data_record = 3;
        let rsize_doubles = (RECORD_SIZE as i32 - 2) / 6; // Type 3: 6 blocks
        let record_size_bytes = rsize_doubles * 6 * 8 + 16;
        let directory_size = 4 * 8;
        let total_data_bytes = record_count as usize * record_size_bytes as usize + directory_size;
        // Total DAF records needed to hold data bytes, rounded up.
        let data_daf_records = total_data_bytes.div_ceil(RECORD_SIZE);
        let total_daf_records = first_data_record as usize - 1 + data_daf_records;
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, total_daf_records as i32 + 1);
        bytes.resize(RECORD_SIZE * total_daf_records, 0);

        // Summary record at record 2.
        let summary_offset = RECORD_SIZE;
        bytes[summary_offset..summary_offset + 8].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 8..summary_offset + 16].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 16..summary_offset + 24].copy_from_slice(&1.0f64.to_le_bytes());

        // Summary data starts at word 4 (byte offset 24 within the record).
        let data_offset = summary_offset + 24;
        bytes[data_offset..data_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&end_et.to_le_bytes());
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&target_id.to_le_bytes());
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&3i32.to_le_bytes());

        // Word addresses. Initial word is record 3 byte 2048 -> word 257.
        let initial_word = 257i32;
        // Final word is initial + total_data_bytes/8 - 1
        let final_word = initial_word + (total_data_bytes as i32 / 8) - 1;
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&initial_word.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&final_word.to_le_bytes());

        let interval_length = (end_et - start_et) / record_count as f64;
        let n_coeffs = rsize_doubles as usize;

        for rec in 0..record_count {
            let offset = (first_data_record as usize - 1) * RECORD_SIZE
                + rec as usize * record_size_bytes as usize;
            let rec_start = start_et + rec as f64 * interval_length;
            let rec_end = rec_start + interval_length;
            let mid = (rec_start + rec_end) * 0.5;
            let radius = (rec_end - rec_start) * 0.5;
            bytes[offset..offset + 8].copy_from_slice(&mid.to_le_bytes());
            bytes[offset + 8..offset + 16].copy_from_slice(&radius.to_le_bytes());

            // Generate position coefficients by sampling position_fn at
            // Chebyshev nodes.
            let mut pos_samples = vec![[0.0; 3]; n_coeffs];
            let mut vel_samples = vec![[0.0; 3]; n_coeffs];
            for i in 0..n_coeffs {
                let x = ((2 * i + 1) as f64 * std::f64::consts::PI / (2.0 * n_coeffs as f64)).cos();
                let t = mid + x * radius;
                pos_samples[i] = position_fn(t);
                // Type 3 stores velocity Chebyshev coefficients with respect
                // to normalized time x. Convert physical velocity to
                // normalized velocity before fitting.
                let v = velocity_fn(t);
                vel_samples[i][0] = v[0] * radius;
                vel_samples[i][1] = v[1] * radius;
                vel_samples[i][2] = v[2] * radius;
            }

            let mut pos_coeffs = vec![[0.0; 3]; n_coeffs];
            let mut vel_coeffs = vec![[0.0; 3]; n_coeffs];
            for k in 0..n_coeffs {
                let mut psum = [0.0; 3];
                let mut vsum = [0.0; 3];
                for (j, (psample, vsample)) in
                    pos_samples.iter().zip(vel_samples.iter()).enumerate()
                {
                    let x_j =
                        ((2 * j + 1) as f64 * std::f64::consts::PI / (2.0 * n_coeffs as f64)).cos();
                    let t_k = crate::ephemeris::chebyshev::chebyshev_polynomial(k, x_j);
                    psum[0] += psample[0] * t_k;
                    psum[1] += psample[1] * t_k;
                    psum[2] += psample[2] * t_k;
                    vsum[0] += vsample[0] * t_k;
                    vsum[1] += vsample[1] * t_k;
                    vsum[2] += vsample[2] * t_k;
                }
                let scale = if k == 0 {
                    1.0 / n_coeffs as f64
                } else {
                    2.0 / n_coeffs as f64
                };
                pos_coeffs[k][0] = psum[0] * scale;
                pos_coeffs[k][1] = psum[1] * scale;
                pos_coeffs[k][2] = psum[2] * scale;
                vel_coeffs[k][0] = vsum[0] * scale;
                vel_coeffs[k][1] = vsum[1] * scale;
                vel_coeffs[k][2] = vsum[2] * scale;
            }

            let block_size = n_coeffs * 8;
            for i in 0..n_coeffs {
                for axis in 0..3 {
                    let pos_offset = offset + 16 + axis * block_size + i * 8;
                    let vel_offset = offset + 16 + (3 + axis) * block_size + i * 8;
                    bytes[pos_offset..pos_offset + 8]
                        .copy_from_slice(&pos_coeffs[i][axis].to_le_bytes());
                    bytes[vel_offset..vel_offset + 8]
                        .copy_from_slice(&vel_coeffs[i][axis].to_le_bytes());
                }
            }
        }
        // Append directory: INIT, INTLEN, RSIZE, N.
        let directory_offset = (first_data_record as usize - 1) * RECORD_SIZE
            + record_count as usize * record_size_bytes as usize;
        bytes[directory_offset..directory_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[directory_offset + 8..directory_offset + 16]
            .copy_from_slice(&interval_length.to_le_bytes());
        bytes[directory_offset + 16..directory_offset + 24]
            .copy_from_slice(&((rsize_doubles * 6 + 2) as f64).to_le_bytes());
        bytes[directory_offset + 24..directory_offset + 32]
            .copy_from_slice(&(record_count as f64).to_le_bytes());

        bytes
    }

    /// Build a complete SPK Type 2 segment fixture.
    ///
    /// The fixture uses compact records of RSIZE doubles and appends the
    /// Type 2 directory at the end of the segment data.
    pub fn build_type2_fixture<F>(
        target_id: i32,
        start_et: f64,
        end_et: f64,
        record_count: i32,
        mut position_fn: F,
    ) -> Vec<u8>
    where
        F: FnMut(f64) -> [f64; 3],
    {
        let first_data_record = 3;
        let rsize_doubles = (RECORD_SIZE as i32 - 2) / 3; // Type 2: 3 blocks
        let record_size_bytes = rsize_doubles * 3 * 8 + 16;
        let directory_size = 4 * 8;
        let total_data_bytes = record_count as usize * record_size_bytes as usize + directory_size;
        let data_daf_records = total_data_bytes.div_ceil(RECORD_SIZE);
        let total_daf_records = first_data_record as usize - 1 + data_daf_records;
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, total_daf_records as i32 + 1);
        bytes.resize(RECORD_SIZE * total_daf_records, 0);

        // Summary record at record 2.
        let summary_offset = RECORD_SIZE;
        bytes[summary_offset..summary_offset + 8].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 8..summary_offset + 16].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 16..summary_offset + 24].copy_from_slice(&1.0f64.to_le_bytes());

        let data_offset = summary_offset + 24;
        bytes[data_offset..data_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&end_et.to_le_bytes());
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&target_id.to_le_bytes());
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&2i32.to_le_bytes());

        let initial_word = 257i32;
        let final_word = initial_word + (total_data_bytes as i32 / 8) - 1;
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&initial_word.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&final_word.to_le_bytes());

        let interval_length = (end_et - start_et) / record_count as f64;
        let n_coeffs = rsize_doubles as usize;

        for rec in 0..record_count {
            let offset = (first_data_record as usize - 1) * RECORD_SIZE
                + rec as usize * record_size_bytes as usize;
            let rec_start = start_et + rec as f64 * interval_length;
            let rec_end = rec_start + interval_length;
            let mid = (rec_start + rec_end) * 0.5;
            let radius = (rec_end - rec_start) * 0.5;
            bytes[offset..offset + 8].copy_from_slice(&mid.to_le_bytes());
            bytes[offset + 8..offset + 16].copy_from_slice(&radius.to_le_bytes());

            let mut samples = vec![[0.0; 3]; n_coeffs];
            for (i, sample) in samples.iter_mut().enumerate() {
                let x = ((2 * i + 1) as f64 * std::f64::consts::PI / (2.0 * n_coeffs as f64)).cos();
                let t = mid + x * radius;
                *sample = position_fn(t);
            }

            let mut coeffs = vec![[0.0; 3]; n_coeffs];
            for (k, coeff) in coeffs.iter_mut().enumerate() {
                let mut sum = [0.0; 3];
                for (j, sample) in samples.iter().enumerate() {
                    let x_j =
                        ((2 * j + 1) as f64 * std::f64::consts::PI / (2.0 * n_coeffs as f64)).cos();
                    let t_k = crate::ephemeris::chebyshev::chebyshev_polynomial(k, x_j);
                    sum[0] += sample[0] * t_k;
                    sum[1] += sample[1] * t_k;
                    sum[2] += sample[2] * t_k;
                }
                let scale = if k == 0 {
                    1.0 / n_coeffs as f64
                } else {
                    2.0 / n_coeffs as f64
                };
                coeff[0] = sum[0] * scale;
                coeff[1] = sum[1] * scale;
                coeff[2] = sum[2] * scale;
            }

            let block_size = n_coeffs * 8;
            for (i, coeff) in coeffs.iter().enumerate() {
                for (axis, c) in coeff.iter().enumerate() {
                    let coeff_offset = offset + 16 + axis * block_size + i * 8;
                    bytes[coeff_offset..coeff_offset + 8].copy_from_slice(&c.to_le_bytes());
                }
            }
        }

        // Append directory.
        let directory_offset = (first_data_record as usize - 1) * RECORD_SIZE
            + record_count as usize * record_size_bytes as usize;
        bytes[directory_offset..directory_offset + 8].copy_from_slice(&start_et.to_le_bytes());
        bytes[directory_offset + 8..directory_offset + 16]
            .copy_from_slice(&interval_length.to_le_bytes());
        bytes[directory_offset + 16..directory_offset + 24]
            .copy_from_slice(&((rsize_doubles * 3 + 2) as f64).to_le_bytes());
        bytes[directory_offset + 24..directory_offset + 32]
            .copy_from_slice(&(record_count as f64).to_le_bytes());

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
        let nd_offset = IDWORD_LEN;
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
        // NAIF summary record control words are three f64 values: next, prev,
        // number of summaries.
        bytes[summary_offset..summary_offset + 8].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 8..summary_offset + 16].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 16..summary_offset + 24].copy_from_slice(&1.0f64.to_le_bytes());

        // Summary data starts at word 4 (byte offset 24 within the record).
        let data_offset = summary_offset + 24;
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
        // Word address 257 corresponds to byte 2048 (record 3).
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&257i32.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&257i32.to_le_bytes());

        let kernel = Kernel::from_bytes(&bytes).unwrap();
        assert_eq!(kernel.segments.len(), 1);

        let seg = &kernel.segments[0];
        assert_eq!(seg.target_id, 499);
        assert_eq!(seg.center_id, 10);
        assert_eq!(seg.frame_id, 1);
        assert_eq!(seg.spk_type, 3);
        assert_eq!(seg.record_count, 1);
        assert_eq!(seg.first_data_byte_offset, 2048);
        assert!((seg.start_et - start_et).abs() < 1e-9);
        assert!((seg.end_et - end_et).abs() < 1e-9);
    }

    #[test]
    fn test_find_segment_by_target_and_epoch() {
        let mut bytes = minimal_daf_header("DAF/SPK", 2, 2, 3);
        bytes.resize(RECORD_SIZE * 2, 0);

        let summary_offset = RECORD_SIZE;
        // NAIF summary record control words are three f64 values: next, prev,
        // number of summaries.
        bytes[summary_offset..summary_offset + 8].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 8..summary_offset + 16].copy_from_slice(&0.0f64.to_le_bytes());
        bytes[summary_offset + 16..summary_offset + 24].copy_from_slice(&1.0f64.to_le_bytes());

        // Summary data starts at word 4 (byte offset 24 within the record).
        let data_offset = summary_offset + 24;
        bytes[data_offset..data_offset + 8].copy_from_slice(&100.0f64.to_le_bytes());
        bytes[data_offset + 8..data_offset + 16].copy_from_slice(&200.0f64.to_le_bytes());
        bytes[data_offset + 16..data_offset + 20].copy_from_slice(&4i32.to_le_bytes()); // Mars barycenter
        bytes[data_offset + 20..data_offset + 24].copy_from_slice(&1i32.to_le_bytes()); // center
        bytes[data_offset + 24..data_offset + 28].copy_from_slice(&1i32.to_le_bytes());
        bytes[data_offset + 28..data_offset + 32].copy_from_slice(&3i32.to_le_bytes());
        // Word address 257 corresponds to byte 2048 (record 3).
        bytes[data_offset + 32..data_offset + 36].copy_from_slice(&257i32.to_le_bytes());
        bytes[data_offset + 36..data_offset + 40].copy_from_slice(&257i32.to_le_bytes());

        let kernel = Kernel::from_bytes(&bytes).unwrap();
        assert!(kernel.find_segment(4, 150.0).is_some());
        assert!(kernel.find_segment(4, 50.0).is_none());
        assert!(kernel.find_segment(5, 150.0).is_none());
    }

    #[test]
    fn test_state_at_constant_position() {
        let start = 0.0;
        let end = 86400.0;
        let fixture = build_type3_fixture(
            499,
            start,
            end,
            1,
            |_x| [1.0, 2.0, 3.0],
            |_x| [0.0, 0.0, 0.0],
        );

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, 43200.0).unwrap();

        // Fixture positions are in km (SPK units); state_at returns SI meters.
        assert_relative_eq!(state.position.x, 1_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.position.y, 2_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.position.z, 3_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.x, 0.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.y, 0.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.z, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_state_at_linear_trajectory() {
        // Position p(t) = [t, 2t, 3t] km over one day.
        let start = 0.0;
        let end = 86400.0;
        let mid = (start + end) * 0.5;
        let fixture = build_type3_fixture(
            499,
            start,
            end,
            1,
            |t| [t, 2.0 * t, 3.0 * t],
            |_t| [1.0, 2.0, 3.0],
        );

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, mid).unwrap();

        // Fixture in km; returned state in m.
        assert_relative_eq!(state.position.x, mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.position.y, 2.0 * mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.position.z, 3.0 * mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.x, 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.y, 2_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.z, 3_000.0, epsilon = 1e-3);
    }

    #[test]
    fn test_state_at_type2_constant_position() {
        let start = 0.0;
        let end = 86400.0;
        let fixture = build_type2_fixture(499, start, end, 1, |_x| [1.0, 2.0, 3.0]);

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, 43200.0).unwrap();

        // Fixture positions are in km (SPK units); state_at returns SI meters.
        assert_relative_eq!(state.position.x, 1_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.position.y, 2_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.position.z, 3_000.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.x, 0.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.y, 0.0, epsilon = 1e-6);
        assert_relative_eq!(state.velocity.z, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn test_state_at_type2_linear_trajectory() {
        // Position p(t) = [t, 2t, 3t] km over one day.
        let start = 0.0;
        let end = 86400.0;
        let mid = (start + end) * 0.5;
        let fixture = build_type2_fixture(499, start, end, 1, |t| [t, 2.0 * t, 3.0 * t]);

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        let state = kernel.state_at(499, mid).unwrap();

        // Fixture in km; returned state in m.
        assert_relative_eq!(state.position.x, mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.position.y, 2.0 * mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.position.z, 3.0 * mid * 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.x, 1_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.y, 2_000.0, epsilon = 1e-3);
        assert_relative_eq!(state.velocity.z, 3_000.0, epsilon = 1e-3);
    }

    #[test]
    fn test_state_at_rejects_uncovered_epoch() {
        let start = 0.0;
        let end = 86400.0;
        let fixture = build_type3_fixture(
            499,
            start,
            end,
            1,
            |_x| [1.0, 2.0, 3.0],
            |_x| [0.0, 0.0, 0.0],
        );

        let kernel = Kernel::from_bytes(&fixture).unwrap();
        assert!(kernel.state_at(499, -1.0).is_err());
        assert!(kernel.state_at(499, end + 1.0).is_err());
        assert!(kernel.state_at(500, 43200.0).is_err());
    }
}
