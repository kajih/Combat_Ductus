use bevy::prelude::*;

mod character_rig;
mod connect_screen;
mod idle_character;
mod stage;

use character_rig::CharacterRigPlugin;
use connect_screen::ConnectScreenPlugin;
use idle_character::IdleCharacterPlugin;
use stage::StagePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(StagePlugin)
        .add_plugins(ConnectScreenPlugin)
        .add_plugins(CharacterRigPlugin)
        .add_plugins(IdleCharacterPlugin)
        .run();
}
