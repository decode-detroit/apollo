// Copyright (c) 2017 Decode Detroit
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

//! A module to create, hold, and handle special windows for the user interface.
//! These additional dialog windows are typically launched from the system menu.

// Import crate definitions
use crate::definitions::*;

// Import standard library features
use std::cell::RefCell;
use std::rc::Rc;

// Import GTK and GIO libraries
use gtk4::prelude::*;

// Import FNV HashMap
use fnv::FnvHashMap;

// Import tracing features
use tracing::error;

/// A structure to contain the window for displaying video streams.
///
pub struct VideoWindow {
    window_widgets: FnvHashMap<WindowNumber, gtk4::Window>, // the hashmap of the window widget for each window
    overlay_widgets: FnvHashMap<WindowNumber, gtk4::Overlay>, // the hashmap of the overlay widget for each window
    window_map: FnvHashMap<ChannelNumber, WindowNumber>, // the mapping of channel numbers to windows
    channel_map: Rc<RefCell<FnvHashMap<std::string::String, gdk4::Rectangle>>>, // the hashmap of channel numbers (as a string) to allocations
}

// Implement key features for the video window
impl VideoWindow {
    /// A function to create a new prompt string dialog structure.
    ///
    pub fn new() -> VideoWindow {
        // Return the completed Video Window
        VideoWindow {
            window_widgets: FnvHashMap::default(),
            overlay_widgets: FnvHashMap::default(),
            window_map: FnvHashMap::default(),
            channel_map: Rc::new(RefCell::new(FnvHashMap::default())),
        }
    }

    /// A method to clear all video windows
    ///
    pub fn clear_all(&mut self) {
        // Empty the overlay widgets
        self.overlay_widgets.clear();

        // Destroy any open windows
        for (_, window) in self.window_widgets.drain() {
            window.destroy();
        }

        // Empty the channel map
        if let Ok(mut map) = self.channel_map.try_borrow_mut() {
            map.clear();
        }

        // Empty the window map
        self.window_map.clear();
    }

    /// A method to define a new window
    ///
    pub fn define_window(&mut self, definition: WindowDefinition) {
        // Copy the window number
        let window_number = definition.window_number;

        // Create the new window and pass dimensions if specified
        let (window, overlay) = self.new_window(Some(definition));

        // Show the window and overlay
        window.set_visible(true);
        overlay.set_visible(true);

        // Save the window in the window widgets
        self.window_widgets.insert(window_number, window);

        // Save the overlay in the overlay widgets
        self.overlay_widgets.insert(window_number, overlay);
    }

    /// A method to add a new video to the video window
    ///
    pub fn add_new_video(&mut self, video_stream: VideoStream) {
        // Wrap the video sink into a widget
        let video_widget = gstgtk4::RenderWidget::new(&video_stream.video_sink);

        // Set the widgets name according to the channel number
        video_widget.set_widget_name(&video_stream.channel.to_string());

        // Try to add the video allocation to the channel map
        match self.channel_map.try_borrow_mut() {
            // Insert the new channel
            Ok(mut map) => {
                map.insert(video_stream.channel.to_string(), video_stream.allocation);
            }

            // Fail silently
            _ => return,
        }

        // Extract the window number (for use below)
        let window_number = video_stream.window_number;

        // Save the channel -> window mapping to the map
        self.window_map
            .insert(video_stream.channel, video_stream.window_number);

        // Check to see if there is already a matching window
        if let Some(overlay) = self.overlay_widgets.get(&window_number) {
            // Add the video area to the overlay
            overlay.add_overlay(&video_widget);

        // Otherwise, create a new window
        } else {
            // Create the new window
            let (window, overlay) = self.new_window(None);

            // Add the video area to the overlay
            overlay.add_overlay(&video_widget);

            // Show the window and overlay
            window.set_visible(true);
            overlay.set_visible(true);

            // Save the window in the window widgets
            self.window_widgets.insert(window_number, window);

            // Save the overlay in the overlay widgets
            self.overlay_widgets.insert(window_number, overlay);
        }
    }

    /// A method to resize  a video within the window
    ///
    pub fn change_allocation(&mut self, channel_allocation: ChannelAllocation) {
        // Try to change the video area within the channel map
        if let Ok(mut map) = self.channel_map.try_borrow_mut() {
            // If the current video was found
            if let Some(allocation) = map.get_mut(&channel_allocation.channel.to_string()) {
                // Update the allocation
                *allocation = gdk4::Rectangle::new(
                    channel_allocation.video_frame.left,
                    channel_allocation.video_frame.top,
                    channel_allocation.video_frame.width,
                    channel_allocation.video_frame.height,
                );

            // Otherwise, warn the user
            } else {
                error!(
                    "Unable to find current settings for channel {}.",
                    channel_allocation.channel
                );
                return;
            }

        // Fail silently
        } else {
            return;
        }

        // Try to locate the correct window number
        if let Some(window_number) = self.window_map.get(&channel_allocation.channel) {
            // Try to get a copy of the overlay
            if let Some(overlay) = self.overlay_widgets.get(window_number) {
                // Trigger a reallocation of the overlay
                overlay.queue_resize();
            }
        }
    }

    /// A method to change the alignment a video within the window
    ///
    pub fn change_alignment(&mut self, channel_realignment: ChannelRealignment) {
        // Try to change the video area within the channel map
        if let Ok(mut map) = self.channel_map.try_borrow_mut() {
            // If the current video was found
            if let Some(allocation) = map.get_mut(&channel_realignment.channel.to_string()) {
                // Switch based on the direction
                match channel_realignment.direction {
                    // Adjust the direction accordingly
                    Direction::Up => {
                        *allocation = gdk4::Rectangle::new(
                            allocation.x(),
                            allocation.y() - 1,
                            allocation.width(),
                            allocation.height(),
                        )
                    }
                    Direction::Down => {
                        *allocation = gdk4::Rectangle::new(
                            allocation.x(),
                            allocation.y() + 1,
                            allocation.width(),
                            allocation.height(),
                        )
                    }
                    Direction::Left => {
                        *allocation = gdk4::Rectangle::new(
                            allocation.x() - 1,
                            allocation.y(),
                            allocation.width(),
                            allocation.height(),
                        )
                    }
                    Direction::Right => {
                        *allocation = gdk4::Rectangle::new(
                            allocation.x() + 1,
                            allocation.y(),
                            allocation.width(),
                            allocation.height(),
                        )
                    }
                }

            // Otherwise, warn the user
            } else {
                error!(
                    "Unable to find current settings for channel {}.",
                    channel_realignment.channel
                );
                return;
            }

        // Fail silently
        } else {
            return;
        }

        // Try to locate the correct window number
        if let Some(window_number) = self.window_map.get(&channel_realignment.channel) {
            // Try to get a copy of the overlay
            if let Some(overlay) = self.overlay_widgets.get(window_number) {
                // Trigger a reallocation of the overlay
                overlay.queue_resize();
            }
        }
    }

    // A helper method to create a new video window and return the window and overlay
    //
    fn new_window(&self, definition: Option<WindowDefinition>) -> (gtk4::Window, gtk4::Overlay) {
        // Create the new window
        let window = gtk4::Window::new();

        // Set window parameters
        window.set_decorated(false);
        window.set_title(Some(WINDOW_TITLE));

        // Disable the delete button for the window
        window.set_deletable(false);

        // Set the window cursor to blank
        window.set_cursor_from_name(Some("none"));

        // Create black background
        let background = gtk4::DrawingArea::new();
        background.set_draw_func(|_, cr, _, _| {
            // Draw the background black
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.paint().unwrap_or(());
        });

        // If there is a window definition
        if let Some(detail) = definition {
            // And it is set to fullscreen, change the window setting
            if detail.fullscreen {
                window.fullscreen();
            }

            // Set the minimum window dimensions, if specified
            if let Some((height, width)) = detail.dimensions {
                background.set_size_request(height, width);
            }

        // Otherwise, default to fullscreen
        } else {
            window.fullscreen();
        }

        // Create the overlay and add the background
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&background));

        // Connect the get_child_position signal
        let channel_map = self.channel_map.clone();
        overlay.connect_get_child_position(move |_, widget| {
            // Try to get the channel map
            if let Ok(map) = channel_map.try_borrow() {
                // Look up the name in the channel map
                if let Some(allocation) = map.get(&widget.widget_name().to_string()) {
                    // Return the completed allocation
                    return Some(*allocation);
                }
            }

            // Return None on failure
            None
        });

        // Add the overlay to the window
        window.set_child(Some(&overlay));

        // Return the window and overlay
        (window, overlay)
    }
}
