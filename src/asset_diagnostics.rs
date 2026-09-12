//! Logs `Image` asset load successes/failures. A first, minimal slice of
//! `docs/issues/observability/logging.md` - added specifically to answer
//! "did the assets actually load" from the console instead of guessing,
//! after a real debugging session where nothing else in the log output
//! indicated a problem despite Characters not rendering in a browser.

use bevy::asset::AssetLoadFailedEvent;
use bevy::prelude::*;

pub struct AssetDiagnosticsPlugin;

impl Plugin for AssetDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (log_image_load_failures, log_image_load_successes));
    }
}

fn log_image_load_failures(mut failures: MessageReader<AssetLoadFailedEvent<Image>>) {
    for failure in failures.read() {
        error!(
            path = %failure.path,
            error = %failure.error,
            "image asset failed to load"
        );
    }
}

fn log_image_load_successes(
    mut events: MessageReader<AssetEvent<Image>>,
    asset_server: Res<AssetServer>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event {
            let path = asset_server.get_path(*id);
            info!(?path, "image asset loaded");
        }
    }
}
