use bevy::prelude::*;

mod character_rig;
mod connect_screen;

use character_rig::CharacterRigPlugin;
use connect_screen::ConnectScreenPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ConnectScreenPlugin)
        .add_plugins(CharacterRigPlugin)
        .run();
}
