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

// Import GTK and GDK libraries
use gdk::Cursor;
use gtk::prelude::*;

// Import FNV HashMap
use fnv::FnvHashMap;

/// A structure to contain the window for displaying video streams.
///
pub struct VideoWindow {
    overlay_map: FnvHashMap<u32, gtk::Overlay>, // the overlay widget
    channel_map: Rc<RefCell<FnvHashMap<std::string::String, gtk::Rectangle>>>, // the mapping of channel numbers to allocations
}

// Implement key features for the video window
impl VideoWindow {
    /// A function to create a new prompt string dialog structure.
    ///
    pub fn new() -> VideoWindow {
        // Create the overlay map
        let overlay_map = FnvHashMap::default();

        // Create the channel map
        let channel_map: Rc<RefCell<FnvHashMap<std::string::String, gtk::Rectangle>>> =
            Rc::new(RefCell::new(FnvHashMap::default()));

        // Return the completed Video Window
        VideoWindow {
            overlay_map,
            channel_map,
        }
    }

    /// A method to clear all video windows
    ///
    pub fn clear_all(&mut self) {
        // Destroy any open windows
        for (_, overlay) in self.overlay_map.drain() {
            if let Some(window) = overlay.parent() {
                unsafe {
                    window.destroy();
                }
            }
        }

        // Empty the channel map
        if let Ok(mut map) = self.channel_map.try_borrow_mut() {
            map.clear();
        }
    }

    /// A method to define a new application window
    /// 
    pub fn define_window(&mut self, definition: WindowDefinition) {
        // Copy the window number
        let window_number = definition.window_number;

        // Create the new window and pass dimensions if specified
        let (window, overlay) = self.new_window(Some(definition));

        // Save the overlay in the overlay map
        self.overlay_map.insert(window_number, overlay);

        // Show the window
        window.show_all();
    }

    /// A method to add a new video to the video window
    ///
    pub fn add_new_video(&mut self, video_stream: VideoStream) {
        // Extract the video widget from the gst element
        let video_widget = video_stream.video_element.property::<gtk::Widget>("widget");

        // Try to add the video area to the channel map
        match self.channel_map.try_borrow_mut() {
            // Insert the new channel
            Ok(mut map) => {
                map.insert(video_stream.channel.to_string(), video_stream.allocation);
            }

            // Fail silently
            _ => return,
        }
        video_widget.set_widget_name(&video_stream.channel.to_string());

        // Extract the window number (for use below)
        let window_number = video_stream.window_number;

        // Connect the realize signal for the video area
        video_widget.connect_realize(move |video_widget| {
            // Try to get a copy of the GDk window
            let gdk_window = match video_widget.window() {
                Some(window) => window,
                None => {
                    println!("Unable to get current window for the video.");
                    return;
                }
            };

            // Set the window cursor to blank
            let display = gdk_window.display();
            if let Some(cursor) = Cursor::for_display(&display, gdk::CursorType::BlankCursor) {
                gdk_window.set_cursor(Some(&cursor));
            }
        });

        // Check to see if there is already a matching window
        if let Some(overlay) = self.overlay_map.get(&window_number) {
            // Add the video area to the overlay
            overlay.add_overlay(&video_widget);

            // Show the video area
            video_widget.show();

        // Otherwise, create a new window
        } else {
            // Create the new window
            let (window, overlay) = self.new_window(None);

            // Add the video area to the overlay
            overlay.add_overlay(&video_widget);

            // Save the overlay in the overlay map
            self.overlay_map.insert(window_number, overlay);

            // Show the window
            window.show_all();
        }
    }

    /// A method to change the location of a video within the window
    ///
    pub fn change_allocation(&mut self, video_allocation: VideoAllocation) {
        // Try to change the video area within the channel map
        if let Ok(mut map) = self.channel_map.try_borrow_mut() {
            // If the current video was found
            if let Some(allocation) = map.get_mut(&video_allocation.channel.to_string()) {
                // Update the allocation
                *allocation = video_allocation.allocation;

            // Otherwise, warn the user
            } else {
                println!("Unable to get find current settings for channel {}", video_allocation.channel);
                return
            }
        
        // Fail silently
        } else {
            return;
        }

        // Try to get a copy of the overlay
        if let Some(overlay) = self.overlay_map.get(&video_allocation.window_number) {
            // Trigger a reallocation of the overlay
            overlay.queue_resize();
        }
    }

    // A helper method to create a new video window and return the window and overlay
    //
    fn new_window(&self, definition: Option<WindowDefinition>) -> (gtk::Window, gtk::Overlay) {
        // Create the new window
        let window = gtk::Window::new(gtk::WindowType::Toplevel);

        // Set window parameters
        window.set_decorated(false);
        window.set_title(WINDOW_TITLE);
        window.set_icon_from_file(LOGO_SQUARE).unwrap_or(()); // give up if unsuccessful
        
        // Disable the delete button for the window
        window.set_deletable(false);

        // Create black background TODO allow other colors for the background
        let background = gtk::DrawingArea::new();
        background.connect_draw(|_, cr| {
            // Draw the background black
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.paint().unwrap_or(());
            Inhibit(true)
        });

        // If there is a definition 
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
        let overlay = gtk::Overlay::new();
        overlay.add(&background);

        // Connect the get_child_position signal
        let channel_map = self.channel_map.clone();
        overlay.connect_get_child_position(move |_, widget| {
            // Try to get the channel map
            if let Ok(map) = channel_map.try_borrow() {
                // Look up the name in the channel map
                if let Some(allocation) = map.get(&widget.widget_name().to_string()) {
                    // Return the completed allocation
                    return Some(allocation.clone());
                }
            }

            // Return None on failure
            None
        });

        // Add the overlay to the window
        window.add(&overlay);

        // Return the overlay
        (window, overlay)
    }
}
