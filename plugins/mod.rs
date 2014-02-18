//! Plugin manager for Lua plugins

use lua;
use config;
use std::{io, libc, str};

/// Manages the Lua state for plugins
pub struct PluginManager {
    priv state: lua::State
}

impl PluginManager {
    /// Creates a new PluginManager and loads all the plugins
    pub fn new(conf: &config::Config) -> PluginManager {
        let mut L = lua::State::new();
        L.openlibs();

        // set up our packages for loading
        L.pushcfunction(lua_setup_packages);
        match L.pcall(0, 0, 0) {
            Ok(()) => (),
            Err(_) => {
                fail!("Error setting up lua packages: {}", L.describe(-1));
            }
        }

        match io::fs::readdir(&conf.plugin_dir) {
            Err(e) => {
                println!("Warning: Could not read plugin dir `{}': {}",
                         conf.plugin_dir.display(), e);
            }
            Ok(paths) => {
                for path in paths.iter() {
                    if path.as_vec() == bytes!(".") || path.as_vec() == bytes!("..") {
                        continue;
                    }
                    if !path.is_file() { continue; }
                    if path.extension() == Some(bytes!("lua")) {
                        // found a plugin
                        debug!("Loading plugin {}", path.filename_display());
                        match L.loadfile(Some(path)) {
                            Ok(()) => (),
                            Err(_) => {
                                println!("Error loading plugin {}: {}", path.filename_display(),
                                         L.describe(-1));
                                L.pop(1);
                                continue;
                            }
                        }
                        // call the plugin's chunk with a single argument, the name of the plugin
                        let name = str::from_utf8_lossy(path.filestem().unwrap());
                        L.pushstring(name.as_slice());
                        match L.pcall(1, 0, 0) {
                            Ok(()) => (),
                            Err(_) => {
                                println!("Error running plugin {}: {}", path.filename_display(),
                                         L.describe(-1));
                                L.pop(1);
                                continue;
                            }
                        }
                    }
                }
            }
        }

        PluginManager { state: L }
    }

    /// Dispatches an IRC event
    pub fn dispatch_irc_event(&mut self, event: &irc::conn::Event) {
        self.state.pushcfunction(irc::lua_dispatch_event);
        self.state.pushlightuserdata(event as *irc::conn::Event as *mut libc::c_void);
        match self.state.pcall(1, 0, 0) {
            Ok(()) => (),
            Err(_) => {
                println!("Error dispatching IRC event: {}", self.state.describe(-1));
                self.state.pop(1);
            }
        }
    }
}

extern "C" fn lua_setup_packages(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };

    // insert our package loaders into package.preload

    // irc
    L.getglobal("package");
    L.getfield(-1, "preload");
    L.pushcfunction(irc::lua_require);
    L.setfield(-1, "irc");
    L.pop(2);
    0
}

mod irc;
