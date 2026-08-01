//! ClockService — time scale conversions.
//!
//! Wraps hifitime 4.x for TDB/TAI/UTC/TT/GPS conversions and adds
//! UT1-UTC support via EOP data.
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

use super::EopData;

/// Clock service for time scale conversions.
#[derive(Debug, Default)]
pub struct ClockService {
    eop: Option<EopData>,
}

impl ClockService {
    /// Create a ClockService without EOP data (UT1 ≈ UTC).
    pub fn new() -> Self {
        Self { eop: None }
    }

    /// Create a ClockService with EOP data for UT1-UTC corrections.
    pub fn with_eop(eop: EopData) -> Self {
        Self { eop: Some(eop) }
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
