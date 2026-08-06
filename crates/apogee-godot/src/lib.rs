//! GDExtension entry point for the Apogee Godot bridge.

use godot::prelude::*;

mod atmosphere_visualizer;
mod star_system;

struct ApogeeGodot;

#[gdextension]
unsafe impl ExtensionLibrary for ApogeeGodot {}
