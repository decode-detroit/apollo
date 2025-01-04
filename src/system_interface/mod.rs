// Copyright (c) 2019-20 Decode Detroit
// Author: Patton Doyle
// Licence: GNU GPLv3
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! A module to create and monitor the user interface and the system inputs.
//! This module links directly to the event handler and sends any updates
//! to the application window.

// Define submodules
mod backup_handler;
mod media_playback;

// Import crate definitions
use crate::definitions::*;

// Import submodute definitions
use backup_handler::BackupHandler;
use media_playback::MediaPlayback;

// Import standard library features
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Import Tokio features
use tokio::sync::mpsc;
use tokio::time::sleep;

// Import FNV HashSet
use fnv::FnvHashSet;

// Import tracing features
use tracing::{error, info};

// Import anyhow features
use anyhow::Result;

/// A structure to contain the system interface and handle all updates to the
/// to the interface.
///
pub struct SystemInterface {
    interface_send: InterfaceSend, // a sending line to pass interface updates
    web_receive: mpsc::Receiver<WebRequest>, // the receiving line for web requests
    media_playback: MediaPlayback, // the structure for controlling media playback
    backup_handler: BackupHandler, // the structure for managing the live system backup
    windows: FnvHashSet<u32>,      // a set of already-defined windows (to avoid duplication)
}

// Implement key SystemInterface functionality
impl SystemInterface {
    /// A function to create a new, blank instance of the system interface.
    ///
    pub async fn new(
        interface_send: InterfaceSend,
        user_address: Arc<Mutex<String>>,
        user_server_location: Arc<Mutex<Option<String>>>,
    ) -> Result<(Self, WebSend)> {
        // Create the web send for the web interface
        let (web_send, web_receive) = WebSend::new();

        // Try to initialize the media playback module
        let media_playback = MediaPlayback::new()?;

        // Try to extract the user defined address
        let mut address = DEFAULT_ADDRESS.to_string();
        if let Ok(lock) = user_address.try_lock() {
            // Copy the address
            address = lock.clone();
        }

        // Try to extract the user defined server_location
        let mut server_location = None;
        if let Ok(lock) = user_server_location.try_lock() {
            // Copy the address
            server_location = lock.clone();
        }

        // Initialize the backup handler
        let backup_handler =
            BackupHandler::new(address, server_location, interface_send.clone()).await;

        // Create the new system interface instance
        let sys_interface = SystemInterface {
            interface_send,
            web_receive,
            media_playback,
            backup_handler,
            windows: FnvHashSet::default(),
        };

        // Regardless, return the new SystemInterface and general send line
        Ok((sys_interface, web_send))
    }

    /// A method to run one iteration of the system interface to update the underlying system of any event changes.
    ///
    async fn run_once(&mut self) -> bool {
        // Check for updates on any line
        tokio::select! {
            // Updates from the Web Interface
            Some(request) = self.web_receive.recv() => {
                // Match the request subtype
                match request.request {
                    // If realigning the channel
                    Request::AlignChannel { channel_realignment } => {
                        // Pass the new video location to the gtk interface
                        self.interface_send.send(InterfaceUpdate::Align { channel_realignment: channel_realignment.clone()});

                        // Backup the change to the channel
                        self.backup_handler.backup_channel_align(channel_realignment).await;

                        // Reply success to the web interface
                        request.reply_to.send(WebReply::success()).unwrap_or(());
                    }

                    // If stopping all the media
                    Request::AllStop => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.all_stop() {
                            // If there was an error, trace the error and reply with the error
                            error!("{}", error);
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());

                        // Otherwise, indicate success
                        } else {
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If defining a new window
                    Request::DefineWindow { window } => {
                        // If the window isn't already defined, add it
                        if self.windows.insert(window.window_number) {
                            // Send the window definition to the gtk interface
                            self.interface_send.send(InterfaceUpdate::Window { window: window.clone() });

                            // Backup the window definition
                            self.backup_handler.backup_window(window).await;

                            // Reply success to the web interface
                            request.reply_to.send(WebReply::success()).unwrap_or(());

                        // Trace the error and reply with the error
                        } else {
                            error!("Window is already defined.");
                            request.reply_to.send(WebReply::failure(format!("Window was already defined."))).unwrap_or(());
                        }
                    }

                    // If defining a new channel
                    Request::DefineChannel { media_channel } => {
                        // Add the channel definition
                        match self.media_playback.define_channel(media_channel.clone()) {
                            // If successful
                            Ok(possible_stream) => {
                                // If a stream was created
                                if let Some(video_stream) = possible_stream {
                                    // Pass the new video stream to the gtk interface
                                    self.interface_send.send(InterfaceUpdate::Video { video_stream });
                                }

                                // Backup the window definition
                                self.backup_handler.backup_channel(media_channel).await;

                                // Reply success to the web interface
                                request.reply_to.send(WebReply::success()).unwrap_or(());
                            }

                            // If there was an error, trace the error and reply with the error
                            Err(error) => {
                                error!("{}", error);
                                request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());
                            }

                        }
                    }

                    // If cuing a new media selection
                    Request::CueMedia { media_cue } => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.cue_media(media_cue.clone()) {
                            // If there was an error, trace the error and reply with the error
                            error!("{}", error);
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());

                        // Otherwise, backup the media and indicate success
                        } else {
                            // Backup the media
                            self.backup_handler.backup_media(media_cue).await;

                            // Indicate success
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If changing the state of a channel
                    Request::ChangeState { channel_state } => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.change_state(channel_state.clone()) {
                            // If there was an error, trace the error and reply with the error
                            error!("{}", error);
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());

                        // Otherwise, backup the change and indicate success
                        } else {
                            // Backup the change
                            self.backup_handler.backup_media_state(channel_state).await;

                            // Indicate success
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If resizing a channel
                    Request::ResizeChannel { channel_allocation } => {
                        // Pass the new video location to the gtk interface
                        self.interface_send.send(InterfaceUpdate::Resize { channel_allocation: channel_allocation.clone() });

                        // Backup the change to the channel
                        self.backup_handler.backup_channel_resize(channel_allocation).await;

                        // Reply success to the web interface
                        request.reply_to.send(WebReply::success()).unwrap_or(());
                    }

                    // If seeking media on a channel
                    Request::Seek { channel_seek } => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.seek(channel_seek.clone()) {
                            // If there was an error, trace the error and reply with the error
                            error!("{}", error);
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());

                        // Otherwise, backup the seek and indicate success
                        } else {
                            // Backup the change
                            self.backup_handler.backup_media_seek(channel_seek).await;

                            // Indicate success
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If closing the program
                    Request::Close => {
                        // End the loop
                        return false;
                    }
                }
            }
        }

        // In most cases, indicate to continue normally
        true
    }

    /// A method to run an infinite number of interations of the system
    /// interface to update the underlying system of any media changes.
    ///
    /// At startup, this loop looks for any exsting backup data and loads
    /// that data, if it's found. Media stored remotely may load too slowly
    /// to be resumed correctly.
    ///
    /// When this loop completes, it will consume the system interface and drop
    /// all associated data.
    ///
    pub async fn run(mut self) {
        // Check for an existing backup
        if let Some((mut window_list, mut channel_list, media_playlist)) =
            self.backup_handler.reload_backup()
        {
            // Reload the window list (reloaded in the order they were defined)
            for window in window_list.drain(..) {
                // If the window isn't already defined, add it
                if self.windows.insert(window.window_number) {
                    self.interface_send
                        .send(InterfaceUpdate::Window { window: window });
                }
            }

            // Reload the channel list (reloaded in the order they were defined)
            for channel in channel_list.drain(..) {
                // If the channel is successfully defined
                if let Ok(possible_stream) = self.media_playback.define_channel(channel) {
                    // If a stream was created
                    if let Some(video_stream) = possible_stream {
                        // Pass the new video stream to the gtk interface
                        self.interface_send
                            .send(InterfaceUpdate::Video { video_stream });
                    }
                }
            }

            // Reload the media playlist, one for each channel
            self.restore_playlist(media_playlist).await;
        }

        // Loop the structure indefinitely
        loop {
            // Repeat endlessly until run_once reaches close
            if !self.run_once().await {
                break;
            }
        }
    }

    // A helper method to reload the media playlist from a backup
    async fn restore_playlist(&mut self, mut playlist: MediaPlaylist) {
        // Look through the playlist for media
        for (channel, playback) in playlist.iter() {
            // For each channel, cue the media
            info!("Playing media on channel {}.", channel);

            // Alert the user if the media failed to play
            if let Err(error) = self.media_playback.cue_media(playback.media_cue.clone()) {
                error!("Unable to restart media on channel {}: {}", channel, error);
            }
        }

        // Wait for all the media to start playing and count the delay
        sleep(Duration::from_millis(500)).await;
        let delay_millis: u64 = 500; // the delay above

        // Look through the playlist for seek position
        for (channel, playback) in playlist.iter() {
            // Calculate the new seek position
            let position = playback.seek_to.as_millis() as u64 + delay_millis; // compensate for our additional delays
            info!(
                "Seeking channel {} to {}.{:0>3}.",
                channel,
                (position / 1000 as u64),
                (position % 1000)
            );

            // Alert the user if seeking media failed
            if let Err(error) = self.media_playback.seek(ChannelSeek {
                channel: channel.clone(),
                position,
            }) {
                error!("Unable to seek media on channel {}: {}", channel, error);
            }
        }

        // Look through the playlist for state
        for (channel, playback) in playlist.drain() {
            // If the state is not playing, change the state
            if playback.state != PlaybackState::Playing {
                // for each channel, change the state
                info!("Changing state on channel {}.", channel);

                // Alert the user if changing the state failed
                if let Err(error) = self.media_playback.change_state(ChannelState {
                    channel,
                    state: playback.state,
                }) {
                    error!(
                        "Unable to change media state on channel {}: {}",
                        channel, error
                    );
                }
            }
        }
    }
}
