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

//! A module to create the video window to diplay the videos triggered
//! by the user.

// Define public submodules
#[macro_use]
pub mod utils;

// Define private submodules
mod video_window;

// Import crate definitions
use crate::definitions::*;

// Import other definitions
use self::video_window::VideoWindow;

// Import standard library features
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

// Import GTK and GDK libraries
use glib;
use gtk;
use gtk::prelude::*;

// Define user interface constants
const REFRESH_RATE: u64 = 10; // the display refresh rate in milliseconds

/// A structure to contain the user interface and handle all updates to the
/// to the interface.
///
#[derive(Clone)]
pub struct GtkInterface {
    video_window: Rc<RefCell<VideoWindow>>, // the video window, wrapped in a refcell and rc for multi-referencing
    empty_window: gtk::ApplicationWindow, // Empty GTK window to keep the program running while there are no video videos open
}

// Implement key GtkInterface functionality
impl GtkInterface {
    /// A function to create a new instance of the gtk interface.
    ///
    pub fn spawn_interface(
        application: &gtk::Application,
        interface_receive: mpsc::Receiver<InterfaceUpdate>,
    ) {
        // Create the empty placeholder window
        let empty_window = gtk::ApplicationWindow::new(application);

        // Create the video window
        let video_window = VideoWindow::new();

        // Wrap the video window in an rc and refcell
        let video_window = Rc::new(RefCell::new(video_window));

        // Create the GtkInterface
        let gtk_interface = GtkInterface {
            video_window,
            empty_window,
        };

        // Launch the interface monitoring interrupt, currently set to ten times a second FIXME make this async
        let update_interface = clone!(gtk_interface => move || {
            gtk_interface.check_updates(&interface_receive);
            glib::ControlFlow::Continue // continue looking for updates indefinitely
        });
        glib::timeout_add_local(Duration::from_millis(REFRESH_RATE), update_interface);
        // triggers once every 10ms
    }

    /// A method to listen for modifications to the gtk interface.
    ///
    /// This method listens on the provided interface_update line for any changes
    /// to the interface. The method then processes any/all of these updates
    /// in the order that they were received.
    ///
    pub fn check_updates(&self, interface_update: &mpsc::Receiver<InterfaceUpdate>) {
        // Look for any updates and act upon them
        loop {
            // Attempt to get a mutable copy of the video_window
            let mut video_window = match self.video_window.try_borrow_mut() {
                Ok(window) => window,
                Err(_) => return, // If unable, exit immediately
            };

            // Check to see if there are any more updatess
            let update = match interface_update.try_recv() {
                Ok(update) => update,
                _ => return, // exit when there are no updates left
            };

            // Unpack the updates of every type
            match update {
                // Launch the video window
                InterfaceUpdate::Window { window } => {
                    // Add the new video stream
                    video_window.define_window(window);
                }

                // Load the new video stream
                InterfaceUpdate::Video { video_stream } => {
                    // Add the new video stream
                    video_window.add_new_video(video_stream);
                }

                // Resize a video stream
                InterfaceUpdate::Resize { channel_allocation } => {
                    // Change the location of the video stream
                    video_window.change_allocation(channel_allocation);
                }

                // Realign a video stream
                InterfaceUpdate::Align {
                    channel_realignment,
                } => {
                    // Change the location of the video stream
                    video_window.change_alignment(channel_realignment);
                }

                // Clear all the video channels and exit
                InterfaceUpdate::Quit => {
                    // Otherwise, destroy the video window
                    video_window.clear_all();
                    unsafe {
                        self.empty_window.destroy();
                    }
                    break;
                }
            }
        }
    }
}
