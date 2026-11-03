//! macOS host: NSApplication (accessory, no Dock icon) on the main thread,
//! MPRemoteCommandCenter targets, MPNowPlayingInfoCenter updates with a
//! 1 s heartbeat (macOS drops a claim it does not see refreshed). Verified
//! with the owner on macOS 27 from kitty (spike, RESEARCH.md).

use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSDate, NSDictionary, NSNumber, NSRunLoop, NSString};
use objc2_media_player::{
    MPMediaItemPropertyAlbumTitle, MPMediaItemPropertyArtist, MPMediaItemPropertyTitle,
    MPNowPlayingInfoCenter, MPNowPlayingInfoPropertyIsLiveStream,
    MPNowPlayingInfoPropertyPlaybackRate, MPNowPlayingPlaybackState, MPRemoteCommand,
    MPRemoteCommandCenter, MPRemoteCommandEvent, MPRemoteCommandHandlerStatus,
};

use crate::{MediaLink, NowPlayingInfo, RemoteCommand};

/// How long the run loop waits per turn (also the exit latency).
const TURN: f64 = 0.1;
const HEARTBEAT: Duration = Duration::from_secs(1);
/// The application thread gets the main thread's usual stack size.
const APP_STACK: usize = 8 * 1024 * 1024;

pub(crate) fn host<R, F>(app: F) -> R
where
    F: FnOnce(Option<MediaLink>) -> R + Send + 'static,
    R: Send + 'static,
{
    let Some(mtm) = MainThreadMarker::new() else {
        // Not on the main thread: AppKit cannot be hosted here.
        return app(None);
    };
    let (updates_tx, updates) = mpsc::channel();
    let (commands_tx, commands) = mpsc::channel();
    let link = MediaLink {
        updates: updates_tx,
        commands,
    };
    let worker = match std::thread::Builder::new()
        .name("ttnr-app".into())
        .stack_size(APP_STACK)
        .spawn(move || app(Some(link)))
    {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!(error = %e, "cannot start the app thread; no media controls");
            // `app` moved into the failed spawn; nothing else can run it.
            std::process::exit(1);
        }
    };

    let app_kit = NSApplication::sharedApplication(mtm);
    app_kit.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app_kit.finishLaunching();
    // SAFETY: MediaPlayer is used on the main thread only (`mtm`).
    let _targets = unsafe { register_commands(&commands_tx) };
    let mut can_skip = None;

    let run_loop = NSRunLoop::mainRunLoop();
    let mut current: Option<NowPlayingInfo> = None;
    let mut last_publish: Option<Instant> = None;
    while !worker.is_finished() {
        autoreleasepool(|_| {
            run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(TURN));
            let mut changed = false;
            for info in updates.try_iter() {
                changed |= info != current;
                current = info;
            }
            let due = last_publish.is_none_or(|t| t.elapsed() >= HEARTBEAT);
            if changed || (due && current.is_some()) {
                let skip = current.as_ref().is_some_and(|i| i.can_skip);
                if can_skip != Some(skip) {
                    // SAFETY: main thread (`host`).
                    unsafe { set_skip_enabled(skip) };
                    can_skip = Some(skip);
                }
                publish(current.as_ref());
                last_publish = Some(Instant::now());
            }
        });
    }
    publish(None);
    match worker.join() {
        Ok(r) => r,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// # Safety
/// Main thread only.
unsafe fn register_commands(tx: &mpsc::Sender<RemoteCommand>) -> Vec<Retained<AnyObject>> {
    // SAFETY: main thread (caller).
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        [
            (center.togglePlayPauseCommand(), RemoteCommand::TogglePause),
            (center.playCommand(), RemoteCommand::Play),
            (center.pauseCommand(), RemoteCommand::Pause),
            (center.nextTrackCommand(), RemoteCommand::Next),
            (center.previousTrackCommand(), RemoteCommand::Previous),
        ]
        .into_iter()
        .map(|(cmd, remote)| register(&cmd, remote, tx.clone()))
        .collect()
    }
}

/// Enable or grey out next/previous (Control Center, media keys).
///
/// # Safety
/// Main thread only.
unsafe fn set_skip_enabled(on: bool) {
    // SAFETY: main thread (caller).
    unsafe {
        let center = MPRemoteCommandCenter::sharedCommandCenter();
        center.nextTrackCommand().setEnabled(on);
        center.previousTrackCommand().setEnabled(on);
    }
}

/// # Safety
/// Main thread only.
unsafe fn register(
    cmd: &MPRemoteCommand,
    remote: RemoteCommand,
    tx: mpsc::Sender<RemoteCommand>,
) -> Retained<AnyObject> {
    let handler = RcBlock::new(move |_event: NonNull<MPRemoteCommandEvent>| {
        let _ = tx.send(remote);
        MPRemoteCommandHandlerStatus::Success
    });
    // SAFETY: main thread (caller); the handler is retained by MediaPlayer.
    unsafe {
        cmd.setEnabled(true);
        cmd.addTargetWithHandler(&handler)
    }
}

/// Publish `info` (or clear Now Playing), always with the playback state –
/// on macOS it must be set on every change.
fn publish(info: Option<&NowPlayingInfo>) {
    // SAFETY: called on the main thread only (from `host`); keys are
    // MediaPlayer's NSString constants with NSString/NSNumber values as
    // documented.
    unsafe {
        let center = MPNowPlayingInfoCenter::defaultCenter();
        let Some(info) = info else {
            center.setNowPlayingInfo(None);
            center.setPlaybackState(MPNowPlayingPlaybackState::Stopped);
            return;
        };
        let title = NSString::from_str(&info.title);
        let artist = info.artist.as_deref().map(NSString::from_str);
        let album = info.album.as_deref().map(NSString::from_str);
        let live = NSNumber::new_bool(true);
        let rate = NSNumber::new_f64(if info.playing { 1.0 } else { 0.0 });
        let mut keys: Vec<&NSString> = vec![
            MPMediaItemPropertyTitle,
            MPNowPlayingInfoPropertyIsLiveStream,
            MPNowPlayingInfoPropertyPlaybackRate,
        ];
        let mut values: Vec<&AnyObject> = vec![&title, &live, &rate];
        if let Some(a) = &artist {
            keys.push(MPMediaItemPropertyArtist);
            values.push(a);
        }
        if let Some(a) = &album {
            keys.push(MPMediaItemPropertyAlbumTitle);
            values.push(a);
        }
        let dict = NSDictionary::from_slices(&keys, &values);
        center.setNowPlayingInfo(Some(&dict));
        center.setPlaybackState(if info.playing {
            MPNowPlayingPlaybackState::Playing
        } else {
            MPNowPlayingPlaybackState::Paused
        });
    }
}
