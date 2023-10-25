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
#![recursion_limit = "256"]

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
use std::sync::{Arc, Mutex};
use std::thread;

// Import tracing features
use tracing::{error, Level};

// Import anyhow macro
#[macro_use]
extern crate anyhow;

// Import GTK and GIO libraries
use gio::prelude::*;

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
        let (system_interface, web_send) =
            match runtime.block_on(async { SystemInterface::new(interface_send.clone()).await }) {
                Ok(result) => result,
                Err(error) => {
                    // Trace the error
                    error!("{}", error);

                    // Panic and exit
                    panic!("Unable to create System Interface: {}", error);
                }
            };

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
        GtkInterface::spawn_interface(application, gtk_interface_recv);
    }
}

/// The main function of the program, simplified to as high a level as possible
/// to allow GTK+ to work its startup magic.
///
fn main() {
    // Create the gtk application window. Failure results in immediate panic!
    let application = gtk::Application::new(None, gio::ApplicationFlags::empty());

    // Create the default address and log level
    let address = Arc::new(Mutex::new(String::from(DEFAULT_ADDRESS)));

    // Register command line options
    let addr_clone = address.clone();
    application.add_main_option(
        "address",
        glib::Char::from(b'a'),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Optional listening address for the webserver, default is 127.0.0.1:27655",
        None,
    );
    application.add_main_option(
        "logLevel",
        glib::Char::from(b'l'),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Optional logging level for tracing. Options are Trace, Info, Debug, Warn, Error",
        None,
    );

    // Handle command line options
    application.connect_handle_local_options(move |_, dict| {
        // Check to see if port was specified
        if dict.contains("address") {
            // Try to get the value
            let variant = dict
                .lookup_value("address", None)
                .expect("Invalid parameter for option 'address'.");

            // Try to convert it to a string
            let new_address: String = variant
                .get()
                .expect("Invalid parameter for option 'address'.");

            // Get a lock on the address
            if let Ok(mut lock) = addr_clone.try_lock() {
                // Save the new address (may still be an invalid string)
                *lock = new_address;
            }
        }

        // Check to see if port was specified
        if dict.contains("logLevel") {
            // Try to get the value
            let variant = dict
                .lookup_value("logLevel", None)
                .expect("Invalid parameter for option 'logLevel'.");

            // Try to convert it to a string
            let log_string: String = variant
                .get()
                .expect("Invalid parameter for option 'logLevel'.");

            // Try to convert the string to a log level
            let log_level = match log_string.as_str() {
                "Trace" => Level::TRACE,
                "Info" => Level::INFO,
                "Debug" => Level::DEBUG,
                "Warn" => Level::WARN,
                "Error" => Level::ERROR,
                _ => panic!("Unable to parse parameter for option 'logLevel."),
            };

            // Initialize tracing
            tracing_subscriber::fmt()
                .with_max_level(log_level)
                .with_target(false)
                .init();

        // Otherwise, use the default log level
        } else {
            // Initialize tracing
            tracing_subscriber::fmt()
                .with_max_level(DEFAULT_LOGLEVEL)
                .with_target(false)
                .init();
        }

        // Don't continue the application
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
