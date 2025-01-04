// Copyright (c) 2023 Decode Detroit
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

//! This module implements structures shared from the backup handler

// Import crate definitions
use crate::definitions::*;

// Import standard library features
use std::time::Duration;

// Import FNV HashMap
use fnv::FnvHashMap;

/// A structure to save a media cue with timing information
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPlayback {
    pub media_cue: MediaCue,  // the media information that was cued
    pub seek_to: Duration,    // the last known position of the media
    pub state: PlaybackState, // the current state of the media
}

/// Implement time updates for the MediaPlayback
impl MediaPlayback {
    /// A method to add time to the current position of the media
    ///
    pub fn update(&mut self, additional_time: Duration) {
        self.seek_to = self
            .seek_to
            .checked_add(additional_time)
            .unwrap_or(self.seek_to); // keep current time if overflow
    }
}

/// A structure to store the media playbacks in a playlist
pub type MediaPlaylist = FnvHashMap<u32, MediaPlayback>;
