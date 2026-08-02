//! Player avatar: unified actor body with movement modes.

use apogee_social::actor::Actor;

/// Player avatar component wrapping the shared Actor body.
#[derive(Debug, Clone, Default)]
pub struct PlayerAvatar {
    pub actor: Actor,
    pub locomotion_mode: LocomotionMode,
    pub health: f64,
}

/// Movement mode for an avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocomotionMode {
    #[default]
    FreeFloating,
    SurfaceWalking,
    MagneticBoots,
    EVA,
    Seated,
    Prone,
}
