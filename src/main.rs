// Copyright (c) 2019 Decode Detroit
// Author: Patton Doyle
// Based on examples from gtk-rs (MIT License)
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

//! The main module of the minerva program which pulls from the other modules.

// Allow deeper recursion testing for web server
#![recursion_limit="256"]

// Import JSON processing features
#[macro_use]
extern crate serde;

// Define program modules
#[macro_use]
mod definitions;
mod system_interface;
#[macro_use]
mod gtk_interface;
mod web_interface;

// Import crate definitions
use crate::definitions::*;

// Import other structures into this module
use self::gtk_interface::GtkInterface;
use self::system_interface::SystemInterface;
use self::web_interface::WebInterface;

// Import standard library features
use std::thread;
use std::sync::{Arc, Mutex};

// Import failure features
#[macro_use]
extern crate failure;

// Import tracing features
use tracing_subscriber;

// Import GTK and GIO libraries
use gio::prelude::*;
use gtk::prelude::*;

// Import tokio features
use tokio::runtime::Runtime;

/// The Apollo structure to contain the program launching and overall
/// communication code.
///
struct Apollo;

// Implement the Apollo functionality
impl Apollo {
    /// A function to build the main program and the user interface
    ///
    fn build_program(application: &gtk::Application, address: Arc<Mutex<String>>) {
        // Create the tokio runtime
        let runtime = Runtime::new().expect("Unable To Create Tokio Runtime.");

        // Create the interface send
        let (interface_send, gtk_interface_recv) = InterfaceSend::new();

        // Launch the system interface to monitor and handle events
        let (system_interface, web_send) = runtime
            .block_on(async {
                SystemInterface::new(interface_send.clone()).await
            })
            .expect("Unable To Create System Interface.");

        // Create a new web interface
        let mut web_interface = WebInterface::new(web_send, address);

        // Spin the runtime into a native thread
        thread::spawn(move || {
            // Run the system interface in a new thread
            runtime.spawn(async move {
                system_interface.run().await;
            });

            // Block on the web interface
            runtime.block_on(async move {
                web_interface.run().await;
            });
        });

        // Create the gtk interface structure to handle video and media playback
        GtkInterface::spawn_interface(gtk_interface_recv);

        // Create the application window, but do not show it
        let window = gtk::ApplicationWindow::new(application);

        // Set the default parameters for the window FIXME not really needed
        window.set_title(WINDOW_TITLE);
        window.set_icon_from_file(LOGO_SQUARE).unwrap_or(()); // give up if unsuccessful

        // Disable the delete button for the window
        window.set_deletable(false);

        // Show the window
        //window.show();
    }
}

/// The main function of the program, simplified to as high a level as possible
/// to allow GTK+ to work its startup magic.
///
fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Create the gtk application window. Failure results in immediate panic!
    let application = gtk::Application::new(None, gio::ApplicationFlags::empty());

    // Register command line options
    let address = Arc::new(Mutex::new(String::from(DEFAULT_ADDRESS)));
    let addr_clone = address.clone();
    application.add_main_option("address", glib::Char::from(b'a'), glib::OptionFlags::NONE, glib::OptionArg::String, "Optional listening address for the webserver, default is 127.0.0.01:27655", None);
    application.connect_handle_local_options(move |_, dict| {
        // Check to see if port was specified
        if dict.contains("address") {
            // Try to get the value
            let variant = dict.lookup_value("address", None).expect("Invalid parameter for option 'address'.");

            // Try to convert it to a string
            let new_address: String = variant.get().expect("Invalid parameter for option 'address'.");
            
            // Get a lock on the address
            if let Ok(mut lock) = addr_clone.try_lock() {
                // Save the new address (may still be an invalid string)
                *lock = new_address;
            }
        }
        
        // Continue the application
        return -1;
    });

    // Create the program and launch the background thread
    application.connect_startup(move |gtk_app| {
        Apollo::build_program(gtk_app, address.clone());
    });

    // Connect the activate-specific function (as compared with open-specific function)
    application.connect_activate(|_| {});

    // Run the application until all the windows are closed
    application.run();
}
