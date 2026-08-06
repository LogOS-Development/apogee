//! GDExtension entry point for the Apogee Godot bridge.

use godot::prelude::*;

mod atmosphere_visualizer;

struct ApogeeGodot;

#[gdextension]
unsafe impl ExtensionLibrary for ApogeeGodot {}
