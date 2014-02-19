//! Plugin manager for Lua plugins

use lua;
use config;
use std::{io, libc, str};

static ERROR_HANDLER: &'static str = "error_handler";

/// Manages the Lua state for plugins
pub struct PluginManager {
    priv state: lua::State,
    priv plugin_dir: Path
}

impl PluginManager {
    /// Creates a new PluginManager and loads all the plugins
    pub fn new(conf: &config::Config) -> PluginManager {
        let L = lua::State::new();

        let mut manager = PluginManager { state: L, plugin_dir: conf.plugin_dir.clone() };
        manager.setup();
        manager
    }

    fn setup(&mut self) {
        let L = &mut self.state;
        L.openlibs();

        // create the error function and store it in the registry
        match L.loadstring("local msg = ...; return debug.traceback(msg, 2)") {
            Ok(()) => (),
            Err(e) => {
                fail!("Error creating error handler: {}: {}", e, L.describe(-1))
            }
        }
        L.setfield(lua::REGISTRYINDEX, ERROR_HANDLER);

        // set up our packages for loading
        L.getfield(lua::REGISTRYINDEX, ERROR_HANDLER);
        L.pushcfunction(lua_setup_packages);
        match L.pcall(0, 0, -2) {
            Ok(()) => (),
            Err(e) => {
                fail!("Error setting up lua packages: {}: {}", e, L.describe(-1));
            }
        }
        L.pop(1); // pop error handler

        match io::fs::readdir(&self.plugin_dir) {
            Err(e) => {
                println!("Warning: Could not read plugin dir `{}': {}",
                         self.plugin_dir.display(), e);
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
                        L.getfield(lua::REGISTRYINDEX, ERROR_HANDLER);
                        match L.loadfile(Some(path)) {
                            Ok(()) => (),
                            Err(_) => {
                                println!("Error loading plugin {}: {}", path.filename_display(),
                                         L.describe(-1));
                                L.pop(2); // pop error, error handler
                                continue;
                            }
                        }
                        // call the plugin's chunk with a single argument, the name of the plugin
                        let name = str::from_utf8_lossy(path.filestem().unwrap());
                        L.pushstring(name.as_slice());
                        match L.pcall(1, 0, -3) {
                            Ok(()) => (),
                            Err(e) => {
                                println!("Error running plugin {}: {}: {}",
                                         path.filename_display(), e, L.describe(-1));
                                L.pop(2); // pop error, error handler
                                continue;
                            }
                        }
                        L.pop(1); // pop error handler
                    }
                }
            }
        }
    }

    /// Reloads all plugins
    pub fn reload_plugins(&mut self, conn: &mut irc::conn::Conn) {
        // do this by setting up a brand new lua::State and re-initializing
        self.state = lua::State::new();
        self.setup();

        // dispatch the RELOADED event
        irc::activate_conn(&mut self.state, conn);
        self.state.getfield(lua::REGISTRYINDEX, ERROR_HANDLER);
        self.state.pushcfunction(irc::lua_dispatch_reloaded);
        match self.state.pcall(0, 0, -2) {
            Ok(()) => (),
            Err(e) => {
                println!("Error dispatching RELOADED event: {}: {}", e, self.state.describe(-1));
                self.state.pop(1);
            }
        }
        self.state.pop(1);
        irc::deactivate_conn(&mut self.state);
    }

    /// Dispatches an IRC event
    pub fn dispatch_irc_event(&mut self, conn: &mut irc::conn::Conn, event: &irc::conn::Event) {
        irc::activate_conn(&mut self.state, conn);
        self.state.getfield(lua::REGISTRYINDEX, ERROR_HANDLER);
        self.state.pushcfunction(irc::lua_dispatch_event);
        self.state.pushlightuserdata(event as *irc::conn::Event as *mut libc::c_void);
        match self.state.pcall(1, 0, -3) {
            Ok(()) => (),
            Err(e) => {
                println!("Error dispatching IRC event: {}: {}", e, self.state.describe(-1));
                self.state.pop(1);
            }
        }
        self.state.pop(1);
        irc::deactivate_conn(&mut self.state);
    }
}

extern "C" fn lua_setup_packages(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };

    // insert our package loaders into package.preload
    L.getglobal("package");
    L.getfield(-1, "preload");

    // irc
    L.pushcfunction(irc::lua_require);
    L.setfield(-2, "irc");

    L.pop(2);
    0
}

mod irc;
