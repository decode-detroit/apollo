// Copyright (c) 2019-2021 Decode Detroit
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

//! This module implements shared communication structures for communicating
//! across the modules of the system.

// Import crate definitions
use crate::definitions::*;

// Import Tokio features
use tokio::sync::{mpsc, oneshot};

// Import standard library features
use std::sync::{mpsc as std_mpsc, Arc, Mutex};

/// The stucture and methods to send WebRequests to the system interface
///
#[derive(Clone, Debug)]
pub struct WebSend {
    web_send: mpsc::Sender<WebRequest>, // the mpsc sending line to pass web requests
}

// Implement the key features of the web send struct
impl WebSend {
    /// A function to create a new WebSend
    ///
    /// The function returns the the Web Sent structure and the system
    /// receive channel which will return the provided updates.
    ///
    pub fn new() -> (Self, mpsc::Receiver<WebRequest>) {
        // Create the new channel
        let (web_send, receive) = mpsc::channel(256);

        // Create and return both new items
        (WebSend { web_send }, receive)
    }

    /// A method to send a web request. This method fails silently.
    ///
    pub async fn send(&self, reply_to: oneshot::Sender<WebReply>, request: Request) {
        self.web_send
            .send(WebRequest { reply_to, request })
            .await
            .unwrap_or(());
    }
}

/// A structure for carrying requests from the web interface
///
pub struct WebRequest {
    pub reply_to: oneshot::Sender<WebReply>, // the handle for replying to the reqeust
    pub request: Request,                    // the request
}

/// An enum to carry requests
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    /// A variant to change location of a video frame by one pixel in one direction.
    /// The size of the video frame remains the constant.
    AlignChannel {
        channel_realignment: ChannelRealignment,
    },

    /// A variant to stop all playing media
    AllStop,

    /// A variant to define a new window
    DefineWindow {
        window: WindowDefinition, // the new application window definition
    },

    /// A variant to define a new channel
    DefineChannel {
        media_channel: MediaChannel, // the new media channel definition
    },

    /// A variant to cue media to play on a specific channel
    CueMedia { media_cue: MediaCue },

    /// A variant to change the playback state of a channel
    ChangeState { channel_state: ChannelState },

    /// A variant to change location and/or size of a video frame
    ResizeChannel {
        channel_allocation: ChannelAllocation,
    },

    /// A variant to seek within the media of a channel
    Seek { channel_seek: ChannelSeek },

    /// A variant to close the program and unload all the data
    Close,
}

/// A type to cover all web replies
///
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebReply {
    // A variant for replies with no specific content
    #[serde(rename_all = "camelCase")]
    Generic {
        is_valid: bool,  // a flag to indicate the result of the request
        message: String, // a message describing the success or failure
    },
}

// Implement key features of the web reply
impl WebReply {
    /// A function to return a new, successful web reply
    ///
    pub fn success() -> WebReply {
        WebReply::Generic {
            is_valid: true,
            message: "Request completed.".to_string(),
        }
    }

    /// A function to return a new, failed web reply
    ///
    pub fn failure<S>(reason: S) -> WebReply
    where
        S: Into<String>,
    {
        WebReply::Generic {
            is_valid: false,
            message: reason.into(),
        }
    }

    /// A method to check if the reply is a success
    ///
    pub fn is_success(&self) -> bool {
        match self {
            &WebReply::Generic { ref is_valid, .. } => is_valid.clone(),
        }
    }
}

/// An enum type to provide updates to the user interface thread.
///
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum InterfaceUpdate {
    /// A variant to define window properties
    Window { window: WindowDefinition },

    /// A variant to create a new video channel
    Video { video_stream: VideoStream },

    /// A variant to resize the video frame
    Resize {
        channel_allocation: ChannelAllocation,
    },

    /// A variant to realign the video frame
    Align {
        channel_realignment: ChannelRealignment,
    },

    /// A variant to close all the windows and exit
    Close,
}

/// The stucture and methods to send updates to the user interface.
///
#[derive(Clone, Debug)]
pub struct InterfaceSend {
    gtk_interface_send: Arc<Mutex<std_mpsc::Sender<InterfaceUpdate>>>, // the line to pass updates to the gtk user interface
}

// Implement the key features of interface send
impl InterfaceSend {
    /// A function to create a new InterfaceSend
    ///
    /// The function returns the InterfaceSend structure and the interface
    /// receive channels which will return the provided updates.
    ///
    pub fn new() -> (Self, std_mpsc::Receiver<InterfaceUpdate>) {
        // Create one or two new channels
        let (gtk_interface_send, gtk_receive) = std_mpsc::channel();

        // Create and return the new items
        return (
            InterfaceSend {
                gtk_interface_send: Arc::new(Mutex::new(gtk_interface_send)),
            },
            gtk_receive,
        );
    }

    /// A method to send an interface update. This method fails silently.
    ///
    pub fn send(&self, update: InterfaceUpdate) {
        // Get a lock on the gtk send line
        if let Ok(gtk_send) = self.gtk_interface_send.lock() {
            // Send the update to the gtk interface
            gtk_send.send(update.clone()).unwrap_or(());
        }
    }
}
