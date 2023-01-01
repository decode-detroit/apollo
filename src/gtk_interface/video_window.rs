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
use std::ffi::c_void;
use std::rc::Rc;

// Import GTK and GDK libraries
use gdk::Cursor;
use gtk::prelude::*;

// Import Gstreamer Library
use gstreamer_video as gst_video;
use gst_video::prelude::*;

// Import FNV HashMap
use fnv::FnvHashMap;

/// A structure to contain the window for displaying video streams.
///
pub struct VideoWindow {
    overlay_map: FnvHashMap<u32, gtk::Overlay>, // the mapping of the overlay widgets
    channel_map: Rc<RefCell<FnvHashMap<std::string::String, gtk::Rectangle>>>, // the mapping of channel numbers to allocations
    window_map: FnvHashMap<u32, u32>, // the mapping of channel numbers to windows
}

// Implement key features for the video window
impl VideoWindow {
    /// A function to create a new prompt string dialog structure.
    ///
    pub fn new() -> VideoWindow {
        // Create the overlay map and window map
        let overlay_map = FnvHashMap::default();
        let window_map = FnvHashMap::default();

        // Create the channel map
        let channel_map: Rc<RefCell<FnvHashMap<std::string::String, gtk::Rectangle>>> =
            Rc::new(RefCell::new(FnvHashMap::default()));

        // Return the completed Video Window
        VideoWindow {
            overlay_map,
            channel_map,
            window_map,
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

        // Empty the window map
        self.window_map = FnvHashMap::default();
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
        // Create a new video area
        let video_area = gtk::DrawingArea::new();

        // Try to add the video area to the channel map
        match self.channel_map.try_borrow_mut() {
            // Insert the new channel
            Ok(mut map) => {
                map.insert(video_stream.channel.to_string(), video_stream.allocation);
            }

            // Fail silently
            _ => return,
        }
        video_area.set_widget_name(&video_stream.channel.to_string());

        // Extract the window number (for use below)
        let window_number = video_stream.window_number;

        // Save the channel -> window mapping to the map
        self.window_map.insert(video_stream.channel, video_stream.window_number);

        // Draw a black background
        video_area.connect_draw(|_, cr| {
            // Draw the background black
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.paint().unwrap_or(());
            Inhibit(true)
        });

        // Connect the realize signal for the video area
        video_area.connect_realize(move |video_area| {
            // Extract a reference for the video overlay
            let video_overlay = &video_stream.video_overlay;

            // Try to get a copy of the GDk window
            let gdk_window = match video_area.window() {
                Some(window) => window,
                None => {
                    println!("Unable to get current window for video overlay.");
                    return;
                }
            };

            // Check to make sure the window is native
            if !gdk_window.ensure_native() {
                println!("Widget is not located inside a native window.");
                return;
            }

            // Extract the display type of the window
            let display_type = gdk_window.display().type_().name();

            // Switch based on the platform
            #[cfg(target_os = "linux")]
            {
                // Check if we're using X11
                if display_type == "GdkX11Display" {
                    // Connect to the get_xid function
                    extern "C" {
                        pub fn gdk_x11_window_get_xid(
                            window: *mut glib::object::Object,
                        ) -> *mut c_void;
                    }

                    // Connect the video overlay to the correct window handle
                    #[allow(clippy::cast_ptr_alignment)]
                    unsafe {
                        let xid = gdk_x11_window_get_xid(gdk_window.as_ptr() as *mut _);
                        video_overlay.set_window_handle(xid as usize);
                    }
                } else {
                    println!("Unsupported display type: {}", display_type);
                }
            }

            // If on Mac OS
            #[cfg(target_os = "macos")]
            {
                // Check if we're using Quartz
                if display_type_name == "GdkQuartzDisplay" {
                    extern "C" {
                        pub fn gdk_quartz_window_get_nsview(
                            window: *mut glib::object::GObject,
                        ) -> *mut c_void;
                    }

                    #[allow(clippy::cast_ptr_alignment)]
                    unsafe {
                        let window = gdk_quartz_window_get_nsview(gdk_window.as_ptr() as *mut _);
                        video_overlay.set_window_handle(window as usize);
                    }
                } else {
                    println!("Unsupported display type {}", display_type);
                }
            }
        });

        // Check to see if there is already a matching window
        if let Some(overlay) = self.overlay_map.get(&window_number) {
            // Add the video area to the overlay
            overlay.add_overlay(&video_area);

            // Show the video area
            video_area.show();

        // Otherwise, create a new window
        } else {
            // Create the new window
            let (window, overlay) = self.new_window(None);

            // Add the video area to the overlay
            overlay.add_overlay(&video_area);

            // Save the overlay in the overlay map
            self.overlay_map.insert(window_number, overlay);

            // Show the window
            window.show_all();
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
                *allocation = gtk::Rectangle::new(channel_allocation.video_frame.left, channel_allocation.video_frame.top, channel_allocation.video_frame.width, channel_allocation.video_frame.height);

            // Otherwise, warn the user
            } else {
                println!("Unable to get find current settings for channel {}", channel_allocation.channel);
                return
            }
        
        // Fail silently
        } else {
            return;
        }

        // Try to locate the correct window number
        if let Some(window_number) = self.window_map.get(&channel_allocation.channel) {
            // Try to get a copy of the overlay
            if let Some(overlay) = self.overlay_map.get(window_number) {
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
                    Direction::Up => *allocation = gtk::Rectangle::new(allocation.x(), allocation.y() - 1, allocation.width(), allocation.height()),
                    Direction::Down => *allocation = gtk::Rectangle::new(allocation.x(), allocation.y() + 1, allocation.width(), allocation.height()),
                    Direction::Left => *allocation = gtk::Rectangle::new(allocation.x() - 1, allocation.y(), allocation.width(), allocation.height()),
                    Direction::Right => *allocation = gtk::Rectangle::new(allocation.x() + 1, allocation.y(), allocation.width(), allocation.height()),
                }

            // Otherwise, warn the user
            } else {
                println!("Unable to get find current settings for channel {}", channel_realignment.channel);
                return
            }
        
        // Fail silently
        } else {
            return;
        }

        // Try to locate the correct window number
        if let Some(window_number) = self.window_map.get(&channel_realignment.channel) {
            // Try to get a copy of the overlay
            if let Some(overlay) = self.overlay_map.get(window_number) {
                // Trigger a reallocation of the overlay
                overlay.queue_resize();
            }
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

        // Connect the realize signal for the video area
        window.connect_realize(move |window| {
            // Try to get a copy of the GDk window
            let gdk_window = match window.window() {
                Some(new_window) => new_window,
                None => {
                    println!("Unable to get current window for video overlay.");
                    return;
                }
            };

            // Set the window cursor to blank
            let display = gdk_window.display();
            if let Some(cursor) = Cursor::for_display(&display, gdk::CursorType::BlankCursor) {
                gdk_window.set_cursor(Some(&cursor));
            }

            // Check to make sure the window is native
            if !gdk_window.ensure_native() {
                println!("Widget is not located inside a native window.");
                return;
            }
        });

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
