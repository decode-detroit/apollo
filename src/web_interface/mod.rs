// Copyright (c) 2020-2021 Decode Detroit
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

//! A module to create the web interface to interface to connect the web UI
//! and endpoints to the program.

// Import crate definitions
use crate::definitions::*;

// Import Tokio and warp features
use tokio::sync::oneshot;
use warp::{http, Filter};

// Import serde feaures
use serde::de::DeserializeOwned;

// Define conversions from data types into a Request
impl From<MediaChannel> for Request {
    fn from(media_channel: MediaChannel) -> Self {
        Request::DefineChannel {
            media_channel,
        }
    }
}
impl From<MediaCue> for Request {
    fn from(media_cue: MediaCue) -> Self {
        Request::CueMedia {
            media_cue,
        }
    }
}
impl From<ChannelState> for Request {
    fn from(channel_state: ChannelState) -> Self {
        Request::ChangeState {
            channel_state,
        }
    }
}

/// A structure to contain the web interface and handle all updates to the
/// to the interface.
///
pub struct WebInterface {
    web_send: WebSend,         // send line to the system interface
}

// Implement key Web Interface functionality
impl WebInterface {
    /// A function to create a new web interface. The send channel should
    /// connect directly to the system interface.
    ///
    pub fn new(web_send: WebSend) -> Self {
        // Return the new web interface and runtime handle
        WebInterface {
            web_send,
        }
    }

    /// A method to listen for connections from the internet
    ///
    pub async fn run(&mut self) {
        // Create the all stop filter
        let all_stop = warp::post()
            .and(warp::path("cueMedia"))
            .and(warp::path::end())
            .and(WebInterface::with_clone(self.web_send.clone()))
            .and(WebInterface::with_clone(Request::AllStop))
            .and_then(WebInterface::handle_request);

        // Create the define channel filter
        let define_channel = warp::post()
            .and(warp::path("defineChannel"))
            .and(warp::path::end())
            .and(WebInterface::with_clone(self.web_send.clone()))
            .and(WebInterface::with_json::<MediaChannel>())
            .and_then(WebInterface::handle_request);

        // Create the cue media filter
        let cue_media = warp::post()
            .and(warp::path("cueMedia"))
            .and(warp::path::end())
            .and(WebInterface::with_clone(self.web_send.clone()))
            .and(WebInterface::with_json::<MediaCue>())
            .and_then(WebInterface::handle_request);

        // Create the change state filter
        let change_state = warp::post()
            .and(warp::path("changeState"))
            .and(warp::path::end())
            .and(WebInterface::with_clone(self.web_send.clone()))
            .and(WebInterface::with_json::<ChannelState>())
            .and_then(WebInterface::handle_request);

        // Combine the filters
        let routes = all_stop
            .or(define_channel)
            .or(cue_media)
            .or(change_state);

        // Handle incoming requests on the media port FIXME accept command-line argument
        warp::serve(routes).run(([127, 0, 0, 1], 27655)).await;
    }

    /// A function to handle define channel requests
    /// 
    async fn handle_request<R>(
        web_send: WebSend,
        request: R,
    ) -> Result<impl warp::Reply, warp::Rejection>
    where
        R: Into<Request>,
    {
        // Send the message and wait for the reply
        let (reply_to, rx) = oneshot::channel();
        web_send.send(reply_to, request.into()).await;

        // Wait for the reply
        if let Ok(reply) = rx.await {
            // If the reply is a success
            if reply.is_success() {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&reply),
                    http::StatusCode::OK,
                ));

            // Otherwise, note the error
            } else {
                return Ok(warp::reply::with_status(
                    warp::reply::json(&reply),
                    http::StatusCode::BAD_REQUEST,
                ));
            }

        // Otherwise, note the error
        } else {
            return Ok(warp::reply::with_status(
                warp::reply::json(&WebReply::failure("Unable to process request.")),
                http::StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    }

    // A function to extract a helper type from the body of the message
    fn with_json<T>() -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone
    where T: Send + DeserializeOwned {
        // When accepting a body, we want a JSON body (reject large payloads)
        warp::body::content_length_limit(1024 * 16).and(warp::body::json())
    }

    // A function to add the web send to the filter
    fn with_clone<T>(
        item: T,
    ) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone
    where T: Send + Clone {
        warp::any().map(move || item.clone())
    }
}
