//! ClockService — time scale conversions.
//!
//! Wraps hifitime 4.x for TDB/TAI/UTC/TT/GPS conversions and adds
//! UT1-UTC support via EOP data and an external leap-second table.
//!
//! # References
//!
//! Time scale conversions (TDB, TAI, UTC, TT, GPS):
//! - IAU 2006 Resolution B3: "Redefinition of Barycentric Dynamical Time, TDB"
//!   (adopted formulation in IERS Conventions 2010, Ch. 1)
//! - IERS Conventions (2010), Petit & Luzum, IERS Technical Note 36, Ch. 1 & Ch. 5
//!   https://iers-conventions.obspm.fr/content/tn36.pdf
//! - hifitime documentation (time scale implementation):
//!   https://docs.rs/hifitime/latest/hifitime/enum.TimeScale.html
//!
//! UT1-UTC via EOP:
//! - IERS EOP 08 C04 data and format:
//!   https://hpiers.obspm.fr/iers/eop/eopc04/
//! - IERS Earth Orientation Parameters:
//!   https://hpiers.obspm.fr/iers/eop/

use hifitime::{Epoch, TimeScale, Unit};
use std::path::Path;

use apogee_common::{ApogeeError, ApogeeResult};

use super::{EopData, LeapSecondTable};

/// Clock service for time scale conversions.
#[derive(Debug, Default)]
pub struct ClockService {
    eop: Option<EopData>,
    leaps: Option<LeapSecondTable>,
}

impl ClockService {
    /// Create a ClockService without EOP or leap-second data (UT1 ≈ UTC,
    /// leap-second conversions rely on hifitime's embedded table).
    pub fn new() -> Self {
        Self {
            eop: None,
            leaps: None,
        }
    }

    /// Create a ClockService with EOP data for UT1-UTC corrections.
    pub fn with_eop(eop: EopData) -> Self {
        Self {
            eop: Some(eop),
            leaps: None,
        }
    }

    /// Create a ClockService with both EOP and leap-second data loaded from
    /// the project `data/` directory.
    ///
    /// Loads:
    ///   - `data_dir/time/Leap_Second.dat`
    ///   - `data_dir/eop/eopc04.txt`
    ///
    /// Missing files produce an error so callers know data has not been fetched.
    pub fn from_data_dir<P: AsRef<Path>>(data_dir: P) -> ApogeeResult<Self> {
        let data_dir = data_dir.as_ref();
        let leap_path = data_dir.join("time").join("Leap_Second.dat");
        let eop_path = data_dir.join("eop").join("eopc04.txt");

        let leaps = LeapSecondTable::load(&leap_path)
            .map_err(|e| ApogeeError::Data(format!("failed to load leap seconds: {e}")))?;
        let eop = EopData::load(&eop_path)
            .map_err(|e| ApogeeError::Data(format!("failed to load EOP data: {e}")))?;

        Ok(Self {
            eop: Some(eop),
            leaps: Some(leaps),
        })
    }

    /// True if the service was configured with EOP data.
    pub fn has_eop(&self) -> bool {
        self.eop.is_some()
    }

    /// True if the service was configured with an external leap-second table.
    pub fn has_leap_table(&self) -> bool {
        self.leaps.is_some()
    }

    /// Return the TAI-UTC offset from the loaded leap-second table at the
    /// given UTC epoch. Falls back to hifitime's internal table if no external
    /// table was loaded.
    pub fn tai_utc_offset(&self, utc: Epoch) -> i32 {
        if let Some(ref table) = self.leaps {
            let mjd = utc.to_mjd_utc_days().round() as u64;
            return table.tai_utc_at_mjd(mjd);
        }
        // hifitime already tracks leap seconds internally; this is the
        // difference between TAI and UTC for the embedded table.
        let tai = utc.to_time_scale(TimeScale::TAI);
        (tai - utc).to_seconds().round() as i32
    }

    /// Convert TAI to UTC.
    pub fn tai_to_utc(&self, tai: Epoch) -> Epoch {
        tai.to_time_scale(TimeScale::UTC)
    }

    /// Convert UTC to TAI.
    pub fn utc_to_tai(&self, utc: Epoch) -> Epoch {
        utc.to_time_scale(TimeScale::TAI)
    }

    /// Convert TDB to TAI.
    pub fn tdb_to_tai(&self, tdb: Epoch) -> Epoch {
        tdb.to_time_scale(TimeScale::TAI)
    }

    /// Convert TAI to TDB.
    pub fn tai_to_tdb(&self, tai: Epoch) -> Epoch {
        tai.to_time_scale(TimeScale::TDB)
    }

    /// Convert TT to TAI (TT = TAI + 32.184s).
    pub fn tt_to_tai(&self, tt: Epoch) -> Epoch {
        tt.to_time_scale(TimeScale::TAI)
    }

    /// Convert UTC to TDB.
    pub fn utc_to_tdb(&self, utc: Epoch) -> Epoch {
        utc.to_time_scale(TimeScale::TDB)
    }

    /// Convert TAI to GPS time (GPS = TAI - 19s).
    pub fn tai_to_gps(&self, tai: Epoch) -> Epoch {
        tai.to_time_scale(TimeScale::GPST)
    }

    /// Convert GPS time to TAI (TAI = GPS + 19s).
    pub fn gps_to_tai(&self, gps: Epoch) -> Epoch {
        gps.to_time_scale(TimeScale::TAI)
    }

    /// Convert UTC to UT1 using EOP data.
    /// UT1 = UTC + (UT1-UTC) where the offset comes from EOP C04.
    /// Without EOP data, returns UTC unchanged.
    ///
    /// # References
    ///
    /// - IERS Conventions (2010), §5.4: UT1 from EOP data
    ///   https://iers-conventions.obspm.fr/content/tn36.pdf (p. 45)
    /// - McCarthy & Petit (2004), "IERS Conventions (2003)", IERS TN 32
    ///   https://ui.adsabs.harvard.edu/abs/2004ITN....32.....M (§4.3)
    pub fn utc_to_ut1(&self, utc: Epoch) -> Epoch {
        if let Some(ref eop) = self.eop {
            let mjd = utc.to_mjd_utc_days();
            if let Some(entry) = eop.at_mjd(mjd) {
                return utc + Unit::Second * entry.ut1_utc;
            }
        }
        utc
    }
}
