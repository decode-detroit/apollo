// Copyright (c) 2024 Decode Detroit
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

//! This module implements the connection to a Redis backup server to maintain
//! a backup of the program state. This handler syncs the current media playlist
//!  to the server. This module does nothing if a Redis server is not connected.
//!
//! WARNING: This module assumes no authorized systems/operators are compromised.

// Import crate definitions
use crate::definitions::*;

// Import standard library features
use std::time::{Duration, Instant};

// Import tracing features
use tracing::{error, warn};

// Imprt redis client library
use redis::{Commands, ConnectionLike, RedisResult};

// Import YAML processing library
use serde_yaml;

/// A structure which holds a reference to the Redis server (if it exists) and
/// syncronizes local data to and from the server.
///
/// # Notes
///
/// When created, the status handler will attempt to connect to the requested
/// redis server. If the status handler cannot make the connection, the status
/// handler will raise an error and return none.
///
pub struct BackupHandler {
    address: String, // the listening address for this instance of the controller for unique identification
    connection: Option<redis::Connection>, // the Redis connection, if it exists
    last_media_update: Instant, // the time of the last update for the media backup
    window_list: WindowList, // the list of all currently defined windows, in the order defined
    channel_list: ChannelList, // the list of all currently  defined channels, in the order defined
    media_playlist: MediaPlaylist, // the current media playback for each channel
}

// Implement key features for the status handler
impl BackupHandler {
    /// A function to create and return a new backup handler.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server provided.
    ///
    /// Like all BackupHandler functions and methods, this function will fail
    /// gracefully by notifying of any errors on the update line and returning
    /// None.
    ///
    pub async fn new(address: String, server_location: Option<String>) -> Self {
        // If a server location was specified
        if let Some(location) = server_location {
            // Try to connect to the Redis server
            if let Ok(client) = redis::Client::open(location.as_str()) {
                // Try to get a copy of the Redis connection
                if let Ok(mut connection) = client.get_connection() {
                    // Set the snapshot settings
                    let result: RedisResult<redis::Value> = connection.req_command(
                        redis::Cmd::new()
                            .arg("CONFIG")
                            .arg("SET")
                            .arg("save")
                            .arg("60 1"),
                    );

                    // Unpack the result from the operation
                    if let Err(..) = result {
                        // Warn that it wasn't possible to update the current scene
                        error!("Unable to set Redis snapshot settings.");
                    }

                    // Return the new backup handler
                    return Self {
                        address,
                        connection: Some(connection),
                        last_media_update: Instant::now(),
                        window_list: Vec::new(),
                        channel_list: Vec::new(),
                        media_playlist: MediaPlaylist::default(),
                    };

                // Indicate that there was a failure to connect to the server
                } else {
                    error!("Unable to connect to backup server: {}.", location);
                }

            // Indicate that there was a failure to connect to the server
            } else {
                error!("Unable to connect to backup server: {}.", location);
            }
        }

        // If a location was not specified or the connection failed, return without a redis connection
        Self {
            address,
            connection: None,
            last_media_update: Instant::now(),
            window_list: Vec::new(),
            channel_list: Vec::new(),
            media_playlist: MediaPlaylist::default(),
        }
    }

    /// A method to backup a new window definition to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_window(&mut self, window_definition: WindowDefinition) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Add the cue to the window list
            self.window_list.push(
                window_definition,
            );

            // Try to serialize the window_list
            let window_string = match serde_yaml::to_string(&self.window_list) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse window list: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:windows", self.address), &window_string);

            // Alert that the window list was not set
            if let Err(..) = result {
                error!("Unable to backup window list onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to backup a new channel definition to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_channel(&mut self, media_channel: MediaChannel) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Add the channel to the channel list
            self.channel_list.push(
                media_channel,
            );

            // Try to serialize the channel list
            let channel_string = match serde_yaml::to_string(&self.channel_list) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse channel list: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:channels", self.address), &channel_string);

            // Alert that the channel list was not set
            if let Err(..) = result {
                error!("Unable to backup channel list onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to update a channel alignment and backup to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_channel_align(&mut self, new_alignment: ChannelRealignment) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Find the channel in the channel list
            for channel in self.channel_list.iter_mut() {
                // If we found the correct channel (checked elsewhere for uniqueness)
                if channel.channel == new_alignment.channel {
                    // See if the channel had a video frame defined
                    let mut frame = match channel.video_frame.clone() {
                        Some(frame) => frame,
                        None => {
                            error!("Unable to backup realign: channel {} doesn't have existing frame.", new_alignment.channel);

                            // Put the connection back
                            self.connection = Some(connection);
                            return;
                        }
                    };

                    // Change the frame based on the direction change
                    match new_alignment.direction {
                        Direction::Up => frame.top -= 1,
                        Direction::Down => frame.top += 1,
                        Direction::Left => frame.left -= 1,
                        Direction::Right => frame.left += 1,
                    }

                    // Update the video frame
                    channel.video_frame = Some(frame);
                }
            }

            // Try to serialize the channel list
            let channel_string = match serde_yaml::to_string(&self.channel_list) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse channel list: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:channels", self.address), &channel_string);

            // Alert that the channel list was not set
            if let Err(..) = result {
                error!("Unable to backup channel list onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to update a channel definition and backup to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_channel_resize(&mut self, new_size: ChannelAllocation) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Find the channel in the channel list
            for channel in self.channel_list.iter_mut() {
                // If we found the correct channel (checked elsewhere for uniqueness)
                if channel.channel == new_size.channel {
                    // See if the channel had a video frame defined
                    let old_frame = match channel.video_frame.clone() {
                        Some(frame) => frame,
                        None => {
                            error!("Unable to backup resize: channel {} doesn't have existing frame.", new_size.channel);

                            // Put the connection back
                            self.connection = Some(connection);
                            return;
                        }
                    };

                    // Recompose the video frame to include the window
                    let new_frame = VideoFrameWithWindow {
                        window_number: old_frame.window_number,
                        top: new_size.video_frame.top,
                        left: new_size.video_frame.left,
                        height: new_size.video_frame.height,
                        width: new_size.video_frame.width,
                    };

                    // Update the video frame
                    channel.video_frame = Some(new_frame);
                }
            }

            // Try to serialize the channel list
            let channel_string = match serde_yaml::to_string(&self.channel_list) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse channel list: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:channels", self.address), &channel_string);

            // Alert that the channel list was not set
            if let Err(..) = result {
                error!("Unable to backup channel list onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to backup the currently playing media to the backup server.
    /// It assumes the media started playing as this function was called.
    ///
    /// # Note
    ///
    /// As the backup handler does not understand the media selected, this
    /// method does not verify the validity of the media cue values in any way.
    /// It is expected that the calling module will perform this check.
    ///
    /// The media interface only waits half a second for media to load before
    /// seeking to the corrent position of the media. This delay may not be
    /// sufficient for network-loaded media which can take several seconds
    /// to load. If the media takes too long to load, the media with resume
    /// playback from the start rather than its correct position.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_media(&mut self, media_cue: MediaCue) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Update the media seek positions
            self.update_media();

            // Add the cue to the media playlist
            self.media_playlist.insert(
                media_cue.channel,
                MediaPlayback {
                    media_cue,
                    seek_to: Duration::from_secs(0),
                    state: PlaybackState::Playing,
                },
            ); // replaces an existing media playback, if it exists

            // Try to serialize the media playlist
            let media_string = match serde_yaml::to_string(&self.media_playlist) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse media playlist: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:media", self.address), &media_string);

            // Alert that the media playlist was not set
            if let Err(..) = result {
                error!("Unable to backup media onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to backup the state of media to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_media_state(&mut self, new_state: ChannelState) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Update the media seek positions
            self.update_media();

            // Try to find the current media
            if let Some(media) = self.media_playlist.get_mut(&new_state.channel) {
                // Upate the media
                media.state = new_state.state;
            
            // Otherwise, warn the media wasn't found
            } else {
                error!("Unable to backup media state: channel {} not defined.", new_state.channel);

                // Put the connection back
                self.connection = Some(connection);
                return;
            }

            // Try to serialize the media playlist
            let media_string = match serde_yaml::to_string(&self.media_playlist) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse media playlist: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:media", self.address), &media_string);

            // Alert that the media playlist was not set
            if let Err(..) = result {
                error!("Unable to backup media onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to backup the seek position of media to the backup server.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub async fn backup_media_seek(&mut self, new_seek: ChannelSeek) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Update the media seek positions
            self.update_media();
            
            // Try to find the current media
            if let Some(media) = self.media_playlist.get_mut(&new_seek.channel) {
                // Upate the media seek location
                media.seek_to = Duration::from_millis(new_seek.position);
            
            // Otherwise, warn the media wasn't found
            } else {
                error!("Unable to backup media state: channel {} not defined.", new_seek.channel);

                // Put the connection back
                self.connection = Some(connection);
                return;
            }

            // Try to serialize the media playlist
            let media_string = match serde_yaml::to_string(&self.media_playlist) {
                Ok(string) => string,
                Err(error) => {
                    error!("Unable to parse media playlist: {}.", error);

                    // Put the connection back
                    self.connection = Some(connection);
                    return;
                }
            };

            // Try to copy the data to the server
            let result: RedisResult<bool> = connection.set(&format!("apollo:{}:media", self.address), &media_string);

            // Alert that the media playlist was not set
            if let Err(..) = result {
                error!("Unable to backup media onto backup server.");
            }

            // Put the connection back
            self.connection = Some(connection);
        }
    }

    /// A method to reload an existing backup from the backup server. If the
    /// data exists, this function returns the existing backup data.
    ///
    /// # Errors
    ///
    /// This function will raise an error if it is unable to connect to the
    /// Redis server.
    ///
    pub fn reload_backup(
        &mut self,
    ) -> Option<(
        WindowList,
        ChannelList,
        MediaPlaylist,
    )> {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Check to see if there is a media playlist
            let result: RedisResult<String> = connection.get(&format!("apollo:{}:media", self.address));

            // If something was received
            if let Ok(media_string) = result {
                // Warn that existing data was found
                warn!("Detected lingering backup data. Reloading ...");

                // Try to parse the data
                let mut media_playlist = MediaPlaylist::default();
                if let Ok(playlist) = serde_yaml::from_str(media_string.as_str()) {
                    media_playlist = playlist;
                }

                // Save the media playlist
                self.media_playlist = media_playlist.clone();

                // Try to read the existing window list
                let mut window_list = WindowList::new();
                let result: RedisResult<String> =
                    connection.get(&format!("apollo:{}:windows", self.address));

                // If something was received
                if let Ok(window_string) = result {
                    // Try to parse the data
                    if let Ok(windows) = serde_yaml::from_str(window_string.as_str()) {
                        window_list = windows;
                    }
                }

                // Save the window list
                self.window_list = window_list.clone();

                // Try to read the existing channel list
                let mut channel_list = ChannelList::new();
                let result: RedisResult<String> =
                    connection.get(&format!("apollo:{}:channels", self.address));

                // If something was received
                if let Ok(channel_string) = result {
                    // Try to parse the data
                    if let Ok(channels) = serde_yaml::from_str(channel_string.as_str()) {
                        channel_list = channels;
                    }
                }

                // Save the channel list
                self.channel_list = channel_list.clone();

                // Put the connection back
                self.connection = Some(connection);

                // Return all the media information
                return Some((
                    window_list,
                    channel_list,
                    media_playlist,
                ));
            }

            // Put the connection back
            self.connection = Some(connection);
        }

        // Silently return nothing if the connection does not exist or there was not any data
        None
    }

    /// A helper function to advance the media seek positions.
    /// This function can be called any time, but it is only useful
    /// if the media playlist is subsequently backed up.
    ///
    fn update_media(&mut self) {
        // Advance the seek position of all the currently playing media
        for media in self.media_playlist.values_mut() {
            media.update(self.last_media_update.elapsed());
        }

        // Save the new update time
        self.last_media_update = Instant::now();
    }
}

// Implement the drop trait for the backup handler struct.
impl Drop for BackupHandler {
    /// This method removes all the the existing statuses from the status server.
    ///
    /// # Errors
    ///
    /// This method will ignore any errors as it is called only when the backup
    /// connection is being closed.
    ///
    fn drop(&mut self) {
        // If the redis connection exists
        if let Some(mut connection) = self.connection.take() {
            // Try to delete the media backup if it exists
            let _: RedisResult<bool> = connection.del(&format!("apollo:{}:media", self.address));

            // Try to delete the channel backup if it exists
            let _: RedisResult<bool> = connection.del(&format!("apollo:{}:channels", self.address));

            // Try to delete the window backup if it exists
            let _: RedisResult<bool> = connection.del(&format!("apollo:{}:windows", self.address));
        }
    }
}

// Tests of the status module
#[cfg(test)]
mod tests {
    use super::*;

    // Test the backup module
    #[tokio::test]
    async fn backup_game() {
        // Create the backup handler
        let mut backup_handler = BackupHandler::new(
            String::from("127.0.0.1:27655"),
            Some("redis://127.0.0.1:6379".to_string()),
        )
        .await;

        // Make sure there is no existing backup
        if backup_handler.reload_backup().is_some() {
            panic!("Backup already existed before beginning of the test.");
        }

        // Load a window, channel, and media cues
        backup_handler
            .backup_window(WindowDefinition {
                window_number: 1,
                fullscreen: true,
                dimensions: None,
            })
            .await;
        backup_handler
            .backup_channel(MediaChannel {
                channel: 1,
                video_frame: None,
                audio_device: None,
                loop_media: None,
            })
            .await;
        backup_handler
            .backup_media(MediaCue {
                channel: 1,
                uri: "video.mp4".to_string(),
                loop_media: None,
            })
            .await;
        backup_handler
            .backup_media(MediaCue {
                channel: 1,
                uri: "new_video.mp4".to_string(),
                loop_media: None,
            })
            .await;

        // Reload the backup
        if let Some((window_list, channel_list, media_playlist)) =
            backup_handler.reload_backup()
        {
            assert_eq!(
                WindowDefinition {
                    window_number: 1,
                    fullscreen: true,
                    dimensions: None,
                },
                window_list[0]);
            assert_eq!(
                MediaChannel {
                    channel: 1,
                    video_frame: None,
                    audio_device: None,
                    loop_media: None,
                },
                channel_list[0]);
            assert_eq!(
                MediaCue {
                    channel: 1,
                    uri: "new_video.mp4".to_string(),
                    loop_media: None
                },
                media_playlist.get(&1).unwrap().media_cue
            );

        // If the backup doesn't exist, throw the error
        } else {
            panic!("Backup was not reloaded.");
        }
    }
}
