//! Flight-mode state machine for spacecraft GNC.
//!
//! Modes represent coarse operational states. Transitions are guarded by
//! safety conditions (e.g. rate limits, fault flags, manual override).

use std::fmt;

/// High-level spacecraft flight mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FlightMode {
    /// No active attitude command; hold current attitude if possible.
    #[default]
    Idle,
    /// Execute a pre-planned attitude maneuver (slew).
    Maneuver,
    /// Track a target in body or inertial frame.
    Point,
    /// Spin-stabilized or torque-free coasting.
    Coast,
    /// Fault-driven safe mode: sun-point, slow spin, minimal actuation.
    Safe,
}

impl fmt::Display for FlightMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlightMode::Idle => write!(f, "Idle"),
            FlightMode::Maneuver => write!(f, "Maneuver"),
            FlightMode::Point => write!(f, "Point"),
            FlightMode::Coast => write!(f, "Coast"),
            FlightMode::Safe => write!(f, "Safe"),
        }
    }
}

/// Conditions required to enter or remain in a mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModeGuard {
    /// Maximum allowed body angular rate (rad/s).
    pub max_rate_rad_s: f64,
    /// Whether the attitude estimator reports a valid solution.
    pub attitude_valid: bool,
    /// Whether actuators are available and not saturated.
    pub actuators_healthy: bool,
}

impl ModeGuard {
    /// Conservative default for normal operations (LEO small sat).
    pub fn nominal() -> Self {
        Self {
            max_rate_rad_s: 0.5,
            attitude_valid: true,
            actuators_healthy: true,
        }
    }

    /// Safe-mode guard: permissive rate, does not require attitude valid.
    pub fn safe() -> Self {
        Self {
            max_rate_rad_s: 5.0,
            attitude_valid: false,
            actuators_healthy: false,
        }
    }
}

/// Transition request plus the guard conditions evaluated by the state machine.
#[derive(Debug, Clone, Copy)]
pub enum ModeCommand {
    /// Manually command a mode.
    Set(FlightMode),
    /// Enter safe mode because of fault(s).
    Fault,
    /// Resume normal operations from safe mode once conditions are met.
    Resume,
}

/// State machine result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionResult {
    /// Transition accepted.
    Accepted,
    /// Transition rejected by guard; current mode remains.
    Rejected,
    /// Already in target mode.
    NoOp,
}

/// Simple deterministic state machine for spacecraft flight modes.
#[derive(Debug, Clone)]
pub struct FlightModeMachine {
    mode: FlightMode,
}

impl FlightModeMachine {
    pub fn new(mode: FlightMode) -> Self {
        Self { mode }
    }

    pub fn current(&self) -> FlightMode {
        self.mode
    }

    /// Request a transition. `guard` is the current vehicle health estimate.
    pub fn command(
        &mut self,
        cmd: ModeCommand,
        guard: &ModeGuard,
        rate_rad_s: f64,
    ) -> TransitionResult {
        let target = match cmd {
            ModeCommand::Set(m) => m,
            ModeCommand::Fault => FlightMode::Safe,
            ModeCommand::Resume => {
                if self.mode != FlightMode::Safe {
                    return TransitionResult::NoOp;
                }
                FlightMode::Idle
            }
        };

        if self.mode == target {
            return TransitionResult::NoOp;
        }

        if !self.guard_ok(target, guard, rate_rad_s) {
            return TransitionResult::Rejected;
        }

        self.mode = target;
        TransitionResult::Accepted
    }

    fn guard_ok(&self, target: FlightMode, guard: &ModeGuard, rate_rad_s: f64) -> bool {
        match target {
            // Safe mode always reachable from any other mode when faulted.
            FlightMode::Safe => true,
            // Idle/Point/Maneuver require healthy actuators, valid attitude, and
            // rate within bounds. Safe mode is intentionally permissive.
            FlightMode::Idle | FlightMode::Point | FlightMode::Maneuver | FlightMode::Coast => {
                guard.actuators_healthy
                    && guard.attitude_valid
                    && rate_rad_s <= guard.max_rate_rad_s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_to_safe_always_allowed() {
        let mut sm = FlightModeMachine::new(FlightMode::Idle);
        let guard = ModeGuard::safe();
        assert_eq!(
            sm.command(ModeCommand::Fault, &guard, 100.0),
            TransitionResult::Accepted
        );
        assert_eq!(sm.current(), FlightMode::Safe);
    }

    #[test]
    fn test_resume_requires_valid_guard() {
        let mut sm = FlightModeMachine::new(FlightMode::Safe);
        let bad = ModeGuard {
            attitude_valid: false,
            actuators_healthy: true,
            ..ModeGuard::nominal()
        };
        assert_eq!(
            sm.command(ModeCommand::Resume, &bad, 0.0),
            TransitionResult::Rejected
        );
        let good = ModeGuard::nominal();
        assert_eq!(
            sm.command(ModeCommand::Resume, &good, 0.0),
            TransitionResult::Accepted
        );
    }

    #[test]
    fn test_maneuver_rejected_if_rate_too_high() {
        let mut sm = FlightModeMachine::new(FlightMode::Idle);
        let guard = ModeGuard::nominal();
        assert_eq!(
            sm.command(ModeCommand::Set(FlightMode::Maneuver), &guard, 1.0),
            TransitionResult::Rejected
        );
        assert_eq!(
            sm.command(ModeCommand::Set(FlightMode::Maneuver), &guard, 0.1),
            TransitionResult::Accepted
        );
    }
}
