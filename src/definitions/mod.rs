// Copyright (c) 2021 Decode Detroit
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

//! This module defines all structures and types used across modules.

// Import tracing features
use tracing::Level;

// Define program constants
pub const LOGO_SQUARE: &str = "logo_square.png";
pub const WINDOW_TITLE: &str = "Apollo";
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:27655";
pub const DEFAULT_LOGLEVEL: Level = Level::WARN;

// Define submodules
mod backup;
mod communication;
mod media;

// Reexport all the definitions from the submodules
pub use self::backup::*;
pub use self::communication::*;
pub use self::media::*;
