//! GDExtension entry point for the Apogee Godot bridge.

use godot::prelude::*;

mod apogee_world;
mod atmosphere_visualizer;
mod verdant_fire_sampler;

struct ApogeeGodot;

#[gdextension]
unsafe impl ExtensionLibrary for ApogeeGodot {}
