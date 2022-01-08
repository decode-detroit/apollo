// Copyright (c) 2020 Decode Detroit
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

//! A module to load and play video and audio files on this device

// Import crate definitions
use crate::definitions::*;

// Import standard library features
use std::sync::{Arc, Mutex};

// Import GTK Library
use glib;
use gtk;
use gtk::prelude::*;

// Import Gstreamer Library
use gstreamer as gst;
use gstreamer_video as gst_video;
use gst_video::prelude::*;

// Import FNV HashMap
use fnv::FnvHashMap;

// Import the failure features
use failure::Error;

/// A helper type to store the playbin and loop media uri
struct InternalChannel {
    playbin: gst::Element,                  // the playbin for this channel
    channel_loop: Option<String>,           // the default loop media for this channel
    loop_mutex: Arc<Mutex<Option<String>>>, // the current loop media handle for this channel
}

/// A structure to hold and manipulate the connection to the media backend
///
pub struct MediaPlayback {
    channels: FnvHashMap<u32, InternalChannel>, // the map of channel numbers to internal channels
}

// Implement key functionality for the Media Out structure
impl MediaPlayback {
    /// A function to create a new instance of the MediaPlayback
    ///
    pub fn new() -> Result<MediaPlayback, Error> {
        // Try to initialize GStreamer
        gst::init()?;

        // Return the complete module
        Ok(MediaPlayback {
            channels: FnvHashMap::default(),
        })
    }

    /// A function to stop all playing media
    /// 
    pub fn all_stop(&self) -> Result<(), Error> {
        // Stop the playing media on every channel
        for (_, channel) in self.channels.iter() {
            channel.playbin.set_state(gst::State::Null)?;
        }

        // Indicate success
        Ok(())
    }

    /// A function a create a new video stream
    ///
    pub fn define_channel(&mut self, media_channel: MediaChannel) -> Result<Option<VideoStream>, Error> {
        // Check to see if there is an existing channel
        if self.channels.contains_key(&media_channel.channel) {
            // Return an error
            return Err(format_err!("Channel is already defined."));
        }

        // Create a new playbin
        let playbin = gst::ElementFactory::make("playbin", None)?;

        // Match based on the audio device specified
        match media_channel.audio_device {
            // An ALSA device
            Some(AudioDevice::Alsa { device_name }) => {
                // Create and set the audio sink
                let audio_sink = gst::ElementFactory::make("alsasink", None)?;
                audio_sink.set_property("device", &device_name)?;
                playbin.set_property("audio-sink", &audio_sink)?;
            }

            // A Pulse Audio device
            Some(AudioDevice::Pulse { device_name }) => {
                // Create and set the audio sink
                let audio_sink = gst::ElementFactory::make("pulsesink", None)?;
                audio_sink.set_property("device", &device_name)?;
                playbin.set_property("audio-sink", &audio_sink)?;
            }

            // Ignore all others
            _ => (),
        }

        // If a video window was specified
        let mut video_stream = None;
        if let Some(video_frame) = media_channel.video_frame {
            // Compose the allocation
            let allocation = gtk::Rectangle {
                x: video_frame.left,
                y: video_frame.top,
                width: video_frame.width,
                height: video_frame.height,
            };

            // Try to create the video overlay
            let video_overlay = match playbin.clone().dynamic_cast::<gst_video::VideoOverlay>()
            {
                Ok(overlay) => overlay,
                _ => return Err(format_err!("Unable to create video stream.")),
            };

            // Send the new video stream to the user interface
            video_stream = Some(VideoStream {
                window_number: video_frame.window_number,
                channel: media_channel.channel,
                allocation,
                video_overlay,
            });
        } // Otherwise, any window creation (if needed) is left to gstreamer

        // Create the loop media mutex
        let loop_mutex = Arc::new(Mutex::new(media_channel.loop_media.clone()));

        // Create the loop media callback
        MediaPlayback::create_loop_callback(&playbin, loop_mutex.clone())?;

        // If loop media was specified
        if let Some(loop_uri) = media_channel.loop_media.clone() {
            // Set the playbin to the loop uri
            playbin.set_property("uri", &loop_uri)?;

            // Start playing the media
            playbin.set_state(gst::State::Playing)?;
        }

        // Add the playbin to the channels
        self.channels.insert(
            media_channel.channel,
            InternalChannel {
                playbin,
                channel_loop: media_channel.loop_media.clone(),
                loop_mutex,
            },
        );

        // Return the video stream, if created
        Ok(video_stream)
    }

    /// A function to cue new media on an existing channel
    /// 
    pub fn cue_media(&self, media_cue: MediaCue) -> Result<(), Error> {
        // Make sure there is an existing channel
        if let Some(channel) = self.channels.get(&media_cue.channel) {
            // Stop the previous media
            channel.playbin.set_state(gst::State::Null)?;

            // Add the uri to this channel
            channel.playbin.set_property("uri", &media_cue.uri)?;

            // Make sure the new media is playing
            channel.playbin.set_state(gst::State::Playing)?;

            // Try to get a lock on the loop mutex
            if let Ok(mut media) = channel.loop_mutex.lock() {
                // Replace the media with the local loop or channel loop
                *media = media_cue.loop_media.or(channel.channel_loop.clone());

            // Otherwise, throw an error
            } else {
                return Err(format_err!("Unable to change loop media."));
            }

        // Otherwise, throw an error
        } else {
            return Err(format_err!("Unable to cue media: Channel not defined."));
        }

        // Indicate success
        Ok(())
    }

    /// A function to change the state of a existing channel
    /// 
    pub fn change_state(&self, channel_state: ChannelState) -> Result<(), Error> {
        // Make sure there is an existing channel
        if let Some(channel) = self.channels.get(&channel_state.channel) {
            // Match the new state
            match channel_state.state {
                // Switch to playing
                PlaybackState::Playing => {
                    channel.playbin.set_state(gst::State::Playing)?;
                }

                // Switch to Paused
                PlaybackState::Paused => {
                    channel.playbin.set_state(gst::State::Paused)?;
                }
            }

        // Otherwise, throw an error
        } else {
            return Err(format_err!("Unable to cue media: Channel not defined."));
        }

        // Indicate success
        Ok(())
    }

    // A helper function to create a signal watch to handle looping media
    fn create_loop_callback(
        playbin: &gst::Element,
        loop_mutex: Arc<Mutex<Option<String>>>,
    ) -> Result<(), Error> {
        // Try to access the playbin bus
        let bus = match playbin.bus() {
            Some(bus) => bus,
            None => return Err(format_err!("Unable to set loop media: Invalid bus.")),
        };

        // Create a week reference to the playbin
        let channel_weak = playbin.downgrade();

        // Connect the signal handler for the end of stream notification
        if let Err(_) = bus.add_watch(move |_, msg| {
            // If the end of stream message is received
            if let gst::MessageView::Eos(..) = msg.view() {
                // Wait for access to the current loop media
                if let Ok(possible_media) = loop_mutex.lock() {
                    // If the media was specified
                    if let Some(media) = possible_media.clone() {
                        // Try to get a strong reference to the channel
                        let channel = match channel_weak.upgrade() {
                            Some(channel) => channel,
                            None => return glib::Continue(true), // Fail silently, but try again
                        };
                        
                        // Try to stop any playing media
                        channel
                            .set_state(gst::State::Null)
                            .unwrap_or(gst::StateChangeSuccess::Success);

                        // If media was specified, add the loop uri to this channel
                        channel.set_property("uri", &media).unwrap_or(());

                        // Try to start playing the media
                        channel
                            .set_state(gst::State::Playing)
                            .unwrap_or(gst::StateChangeSuccess::Success);
                    }
                }
            }

            // Continue with other signal handlers
            glib::Continue(true)

            // Warn the user of failure
        }) {
            return Err(format_err!("Unable to set loop media: Duplicate watch."));
        }

        // Indicate success
        Ok(())
    }
}

// Implement the drop trait for MediaPlayback
impl Drop for MediaPlayback {
    /// This method sets any active playbins to NULL
    ///
    fn drop(&mut self) {
        // For every playbin in the active channels
        for (_, channel) in self.channels.drain() {
            // Try to remove the bus signal watch
            if let Some(bus) = channel.playbin.bus() {
                bus.remove_watch().unwrap_or(());
            }
        }
    }
}
