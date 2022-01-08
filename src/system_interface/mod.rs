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
mod media_playback;

// Import crate definitions
use crate::definitions::*;

// Import submodute definitions
use media_playback::MediaPlayback;

// Import Tokio features
use tokio::sync::mpsc;

// Import the failure features
use failure::Error as FailureError;

/// A structure to contain the system interface and handle all updates to the
/// to the interface.
///
/// # Note
///
/// This structure is still under rapid development and may change operation
/// in the near future.
///
pub struct SystemInterface {
    interface_send: InterfaceSend,  // a sending line to pass interface updates
    web_receive: mpsc::Receiver<WebRequest>, // the receiving line for web requests
    media_playback: MediaPlayback,  // the structure for controlling media playback
}

// Implement key SystemInterface functionality
impl SystemInterface {
    /// A function to create a new, blank instance of the system interface.
    ///
    pub async fn new(
        interface_send: InterfaceSend,
    ) -> Result<(Self, WebSend), FailureError> {
        // Create the web send for the web interface
        let (web_send, web_receive) = WebSend::new();

        // Try to initialize the media playback module
        let media_playback = MediaPlayback::new()?;

        // Create the new system interface instance
        let sys_interface = SystemInterface {
            interface_send,
            web_receive,
            media_playback,
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
                    // If stopping all the media
                    Request::AllStop => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.all_stop() {
                            // If there was an error, reply with the error
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());
                        
                        // Otherwise, indicate success
                        } else {
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If defining a new window
                    Request::DefineWindow { window } => {
                        // Send the window definition to the gtk interface
                        self.interface_send.send(InterfaceUpdate::Window { window });

                        // Reply success to the web interface
                        request.reply_to.send(WebReply::success()).unwrap_or(());
                    }

                    // If defining a new channel
                    Request::DefineChannel { media_channel } => {
                        // Add the channel definition
                        match self.media_playback.define_channel(media_channel) {
                            // If successful
                            Ok(possible_stream) => {
                                // If a stream was created
                                if let Some(video_stream) = possible_stream {
                                    // Pass the new video stream to the gtk interface
                                    self.interface_send.send(InterfaceUpdate::Video { video_stream });
                                }

                                // Reply success to the web interface
                                request.reply_to.send(WebReply::success()).unwrap_or(());
                            }

                            // If there was an error, reply with the error
                            Err(error) => request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(()),
                        }
                    }

                    // If cuing a new media selection
                    Request::CueMedia { media_cue } => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.cue_media(media_cue) {
                            // If there was an error, reply with the error
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());
                        
                        // Otherwise, indicate success
                        } else {
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
                    }

                    // If changing the state of a channel
                    Request::ChangeState { channel_state } => {
                        // Try to cue the new media
                        if let Err(error) = self.media_playback.change_state(channel_state) {
                            // If there was an error, reply with the error
                            request.reply_to.send(WebReply::failure(format!("{}", error))).unwrap_or(());
                        
                        // Otherwise, indicate success
                        } else {
                            request.reply_to.send(WebReply::success()).unwrap_or(());
                        }
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
    /// When this loop completes, it will consume the system interface and drop
    /// all associated data.
    ///
    pub async fn run(mut self) {
        // Loop the structure indefinitely
        loop {
            // Repeat endlessly until run_once reaches close
            if !self.run_once().await {
                break;
            }
        }

        // Drop all associated data in system interface
        drop(self);
    }
}

// Implement the drop trait for SystemInterface
impl Drop for SystemInterface {
    /// This method removes any active video windows
    ///
    fn drop(&mut self) {
        // Destroy the video windows
        self.interface_send.send(InterfaceUpdate::Close);
    }
}


// Tests of the system_interface module
#[cfg(test)]
mod tests {
    //use super::*;

    // FIXME Define tests of this module
    #[test]
    fn missing_tests() {
        // FIXME: Implement this
        unimplemented!();
    }
}
