#[cfg(feature = "sound")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "sound")]
static PLAYING: AtomicBool = AtomicBool::new(false);

/// Plays the bundled bell once. Concurrent requests are coalesced so a burst of
/// new problems does not stack several sounds on top of each other.
#[cfg(feature = "sound")]
pub fn play_bell() {
    if PLAYING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("observo-bell".to_string())
        .spawn(|| {
            if let Err(error) = play_bell_blocking() {
                log::warn!("could not play notification sound: {error}");
            }
            PLAYING.store(false, Ordering::SeqCst);
        })
        .ok();
}

#[cfg(feature = "sound")]
fn play_bell_blocking() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bytes = include_bytes!("../resources/bell.wav");
    let device = rodio::DeviceSinkBuilder::open_default_sink()?;
    let mixer = device.mixer();
    let player = rodio::Player::connect_new(mixer);
    let source = rodio::Decoder::new(std::io::Cursor::new(bytes))?;
    player.append(source);
    player.sleep_until_end();
    Ok(())
}

#[cfg(not(feature = "sound"))]
pub fn play_bell() {}
