use bevy::prelude::*;

mod connect_screen;

use connect_screen::ConnectScreenPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ConnectScreenPlugin)
        .run();
}
