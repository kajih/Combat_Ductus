use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;

mod asset_diagnostics;
mod character_rig;
mod connect_screen;
mod health_hud;
mod input;
mod match_characters;
mod match_end_screen;
mod stage;

use asset_diagnostics::AssetDiagnosticsPlugin;
use character_rig::CharacterRigPlugin;
use connect_screen::ConnectScreenPlugin;
use health_hud::HealthHudPlugin;
use input::PlayerInputPlugin;
use match_characters::MatchCharactersPlugin;
use match_end_screen::MatchEndScreenPlugin;
use stage::StagePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // We ship no .meta files at all. Bevy's default (`Always`) probes
            // for one alongside every asset; on the wasm/Trunk dev server
            // that probe doesn't get a clean 404 for a missing file (it
            // appears to get the dev server's SPA-style index.html fallback
            // instead), which Bevy then fails to parse as asset metadata and
            // treats as a load failure - so every image silently "failed to
            // load" despite being served fine. `Never` skips the probe
            // entirely and just uses default meta, which is all we need.
            meta_check: AssetMetaCheck::Never,
            ..default()
        }))
        .add_plugins(AssetDiagnosticsPlugin)
        .add_plugins(StagePlugin)
        .add_plugins(ConnectScreenPlugin)
        .add_plugins(CharacterRigPlugin)
        .add_plugins(MatchCharactersPlugin)
        .add_plugins(PlayerInputPlugin)
        .add_plugins(HealthHudPlugin)
        .add_plugins(MatchEndScreenPlugin)
        .run();
}
