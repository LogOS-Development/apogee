//! Force aggregator system: collects gravity + drag + SRP forces — stub.

/// Aggregated forces on a body.
#[derive(Debug, Clone, Default)]
pub struct AggregatedForces {
    pub gravity: apogee_common::Position,
    pub drag: apogee_common::Position,
    pub srp: apogee_common::Position,
    pub thrust: apogee_common::Position,
}

impl AggregatedForces {
    /// Sum all force contributions.
    pub fn total(&self) -> apogee_common::Position {
        self.gravity + self.drag + self.srp + self.thrust
    }
}
