//! M6 spike: does a windowless terminal process appear in macOS Now Playing
//! / Control Center and receive media keys? Plays a stream through
//! `MpvPlayer`, publishes Now Playing info as a live stream and prints
//! every remote command it receives.
//!
//! ```text
//! cargo run -p ttnr-platform --example nowplaying_spike -- [accessory|prohibited|none] [URL]
//! TTNR_NO_PLIST=1 cargo run …   # without the embedded Info.plist
//! ```
//! Stop with Ctrl-C.

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    spike::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macOS only");
}

#[cfg(target_os = "macos")]
mod spike {
    use std::ptr::NonNull;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    use objc2_foundation::{NSDate, NSDictionary, NSNumber, NSRunLoop, NSString};
    use objc2_media_player::{
        MPMediaItemPropertyArtist, MPMediaItemPropertyTitle, MPNowPlayingInfoCenter,
        MPNowPlayingInfoPropertyIsLiveStream, MPNowPlayingInfoPropertyPlaybackRate,
        MPNowPlayingPlaybackState, MPRemoteCommand, MPRemoteCommandCenter, MPRemoteCommandEvent,
        MPRemoteCommandHandlerStatus,
    };
    use ttnr_core::StreamDescriptor;
    use ttnr_engine::{EngineConfig, MpvPlayer, Player};

    #[derive(Debug, Clone, Copy)]
    enum Remote {
        Toggle,
        Play,
        Pause,
        Next,
        Previous,
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let mut args = std::env::args().skip(1);
        let policy = args.next().unwrap_or_else(|| "accessory".into());
        let url = args
            .next()
            .unwrap_or_else(|| "https://ice6.somafm.com/groovesalad-128-mp3".into());
        let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
        let plist = std::env::var_os("TTNR_NO_PLIST").is_none();
        eprintln!("policy={policy} embedded Info.plist={plist} url={url}");

        if policy != "none" {
            let app = NSApplication::sharedApplication(mtm);
            let p = if policy == "prohibited" {
                NSApplicationActivationPolicy::Prohibited
            } else {
                NSApplicationActivationPolicy::Accessory
            };
            app.setActivationPolicy(p);
            app.finishLaunching();
        }

        let mut player = MpvPlayer::new(&EngineConfig::default())?;
        player.play(&StreamDescriptor::direct(url))?;

        let (tx, rx) = mpsc::channel::<Remote>();
        // SAFETY: MediaPlayer calls on the main thread (checked above).
        let center = unsafe { MPRemoteCommandCenter::sharedCommandCenter() };
        let mut targets = Vec::new();
        unsafe {
            for (cmd, remote) in [
                (center.togglePlayPauseCommand(), Remote::Toggle),
                (center.playCommand(), Remote::Play),
                (center.pauseCommand(), Remote::Pause),
                (center.nextTrackCommand(), Remote::Next),
                (center.previousTrackCommand(), Remote::Previous),
            ] {
                targets.push(register(&cmd, remote, tx.clone()));
            }
        }

        let mut playing = true;
        let mut last_publish = Instant::now() - Duration::from_secs(10);
        let run_loop = NSRunLoop::mainRunLoop();
        loop {
            // Service the main run loop (remote commands arrive here).
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.1));
            for remote in rx.try_iter() {
                eprintln!("remote command: {remote:?}");
                match remote {
                    Remote::Toggle => playing = !playing,
                    Remote::Play => playing = true,
                    Remote::Pause => playing = false,
                    Remote::Next | Remote::Previous => {}
                }
                player.pause(!playing)?;
                last_publish = Instant::now() - Duration::from_secs(10);
            }
            // Heartbeat: re-assert the state about once per second.
            if last_publish.elapsed() >= Duration::from_secs(1) {
                publish(playing);
                last_publish = Instant::now();
            }
        }
    }

    /// # Safety
    /// Main thread only.
    unsafe fn register(
        cmd: &MPRemoteCommand,
        remote: Remote,
        tx: mpsc::Sender<Remote>,
    ) -> Retained<AnyObject> {
        let handler = RcBlock::new(move |_event: NonNull<MPRemoteCommandEvent>| {
            let _ = tx.send(remote);
            MPRemoteCommandHandlerStatus::Success
        });
        unsafe {
            cmd.setEnabled(true);
            cmd.addTargetWithHandler(&handler)
        }
    }

    fn publish(playing: bool) {
        let title = NSString::from_str("ttnr spike – Groove Salad");
        let artist = NSString::from_str("SomaFM");
        let live = NSNumber::new_bool(true);
        let rate = NSNumber::new_f64(if playing { 1.0 } else { 0.0 });
        // SAFETY: the keys are MediaPlayer's NSString constants; values are
        // NSString/NSNumber as documented; main thread.
        unsafe {
            let keys: [&NSString; 4] = [
                MPMediaItemPropertyTitle,
                MPMediaItemPropertyArtist,
                MPNowPlayingInfoPropertyIsLiveStream,
                MPNowPlayingInfoPropertyPlaybackRate,
            ];
            let values: [&AnyObject; 4] = [&title, &artist, &live, &rate];
            let info = NSDictionary::from_slices(&keys, &values);
            let center = MPNowPlayingInfoCenter::defaultCenter();
            center.setNowPlayingInfo(Some(&info));
            center.setPlaybackState(if playing {
                MPNowPlayingPlaybackState::Playing
            } else {
                MPNowPlayingPlaybackState::Paused
            });
        }
    }
}
