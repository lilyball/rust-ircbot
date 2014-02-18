//! Lua IRC library
//!
//! Vends a package named 'irc' with a set of functions that manipulate the
//! current connection. Also provides event handling.
//!
//! Lua functions registered with irc.addhandler(event, f) are called with a
//! string argument representing the event, followed by the sender, then the
//! event's arguments.  Regular commands provide their arguments in the
//! expected fashion. CTCP commands are a bit different. CTCP actions provide 2
//! arguments: dst and text. CTCP commands and replies provide 2 or 3
//! arguments: CTCP command name, destionation, and text if provided.
//!
//! Note that the arguments of any arbitrary IRC command should not be assumed.
//! e.g. PRIVMSG should have 2 arguments: dst, and text. But the actual arguments
//! are provided by the IRC server and are not validated by the bot before being
//! passed to Lua. The only argument guarantees are made by the CTCP commands.
//!
//! Note: if the prefix was not provided for a given command, it will be given
//! to Lua as nil. Otherwise, it will be a table representation of the User.
//!
//! There are 5 special events that can be registered:
//!
//! irc.CONNECTED: No args
//! irc.DISCONNECTED: No args
//! irc.ACTION: Sender, destination, text
//! irc.CTCP: Sender, CTCP command, destination, optionally text
//! irc.CTCPREPLY: Sender, CTCP command, destination, optionally text
//!
//! A User (the sender value) is a table with the following values:
//!
//! raw: The raw text comprising the user
//! nick: The nickname of the user
//! user: The username of the user, if any (optional, may be nil)
//! host: The hostname of the user, if any (optional, may be nil)

use lua;
use irc::conn;
use irc::conn::{Conn, Event};
use std::{libc, mem, ptr, str};
use std::iter::range_inclusive;
use std::vec::MutableCloneableVector;

static EVT_CONNECTED: &'static str = "-CONNECTED";
static EVT_DISCONNECTED: &'static str = "-DISCONNECTED";
static EVT_ACTION: &'static str = "-ACTION";
static EVT_CTCP: &'static str = "-CTCP";
static EVT_CTCPREPLY: &'static str = "-CTCPREPLY";

pub extern "C" fn lua_require(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };
    // 1 argument is passed: modname

    // we're going to store the Conn in an Option in the registry
    // the key for the registry is lua_require as a lightuserdata
    L.pushlightuserdata(lua_require as *mut libc::c_void);
    let connptr = L.newuserdata(mem::size_of::<*mut Conn>()) as *mut *mut Conn;
    unsafe { *connptr = ptr::mut_null() };
    L.settable(lua::REGISTRYINDEX);

    // register our library functions
    L.newtable();
    L.registerlib(None, [
        ("addhandler", lua_addhandler),
        //("host", lua_host),
        //("me", lua_me),
        //("send_raw", lua_send_raw),
        //("set_nick", lua_set_nick),
        //("quit", lua_quit),
        ("privmsg", lua_privmsg),
        //("notice",  lua_notice),
        //("join", lua_join),
        //("quit", lua_quit)
    ]);

    // set a few constant values into the table
    L.pushstring(EVT_CONNECTED);
    L.setfield(-2, "CONNECTED");
    L.pushstring(EVT_DISCONNECTED);
    L.setfield(-2, "DISCONNECTED");
    L.pushstring(EVT_ACTION);
    L.setfield(-2, "ACTION");
    L.pushstring(EVT_CTCP);
    L.setfield(-2, "CTCP");
    L.pushstring(EVT_CTCPREPLY);
    L.setfield(-2, "CTCPREPLY");

    1
}

pub extern "C" fn lua_dispatch_event(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };

    // 1 arg: event

    let evtptr = L.touserdata(1) as *mut Event;
    L.argcheck(evtptr.is_not_null(), 1, "expected Event");
    let event = unsafe { &*evtptr };

    L.settop(0); // clear the stack

    // get the event name
    match *event {
        conn::Connected => {
            L.pushstring(EVT_CONNECTED);
        }
        conn::Disconnected => {
            L.pushstring(EVT_DISCONNECTED);
        }
        conn::LineReceived(ref line) => {
            let conn::Line{ref command, ref args, ref prefix} = *line;

            match *command {
                conn::IRCCode(code) => {
                    // construct our string on the stack
                    let mut buf = [0u8, ..64];
                    let n = {
                        // sadly MemWriter can't take a stack buffer
                        // so let's just create the string here
                        // then copy it to the stack
                        let s = code.to_str();
                        buf.copy_from(s.as_bytes())
                    };
                    let s = match str::from_utf8(buf.slice_to(n)) {
                        None => {
                            L.errorstr("could not format IRCCode");
                        }
                        Some(s) => s
                    };
                    L.pushstring(s);
                }
                conn::IRCCmd(ref cmd) => {
                    L.pushstring(cmd.as_slice());
                }
                conn::IRCAction(ref dst) => {
                    L.pushstring(EVT_ACTION);
                    L.pushbytes(dst.as_slice());
                }
                conn::IRCCTCP(ref cmd, ref dst) => {
                    L.pushstring(EVT_CTCP);
                    L.pushbytes(cmd.as_slice());
                    L.pushbytes(dst.as_slice());
                }
                conn::IRCCTCPReply(ref cmd, ref dst) => {
                    L.pushstring(EVT_CTCPREPLY);
                    L.pushbytes(cmd.as_slice());
                    L.pushbytes(dst.as_slice());
                }
            }

            // ensure we actually have a handler for this event before proceeding
            L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
            L.gettable(lua::REGISTRYINDEX);
            if !L.istable(-1) {
                return 0;
            }
            L.pushvalue(1); // event name
            L.gettable(-2);
            if !L.istable(-1) || L.objlen(-1) == 0 {
                return 0;
            }
            // we have at least one handler; continue on
            L.pop(2);

            // construct the sender
            match *prefix {
                None => {
                    L.pushnil();
                }
                Some(ref user) => {
                    L.createtable(0, 4);
                    L.pushbytes(user.raw());
                    L.setfield(-2, "raw");
                    L.pushbytes(user.nick());
                    L.setfield(-2, "nick");
                    match user.user() {
                        None => L.pushnil(),
                        Some(v) => L.pushbytes(v)
                    }
                    L.setfield(-2, "user");
                    match user.user() {
                        None => L.pushnil(),
                        Some(v) => L.pushbytes(v)
                    }
                    L.setfield(-2, "host");
                }
            }
            // move sender just after the event name
            L.insert(2);
            // add any arguments
            for arg in args.iter() {
                L.pushbytes(*arg);
            }
        }
    }

    // our event arguments are all on the stack
    let nargs = L.gettop();
    // get the handler list and call each one with a copy of the arguments
    L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
    L.gettable(lua::REGISTRYINDEX);
    if !L.istable(-1) {
        return 0; // no handlers
    }
    L.pushvalue(1); // event name
    L.gettable(-2);
    if !L.istable(-1) {
        return 0;
    }
    L.pushnil(); // first key
    while L.next(-2) {
        // key is -2, value is -1
        // copy all the arguments; deep-copy the sender table
        for i in range_inclusive(1, nargs) {
            if L.istable(i) {
                // copy it
                L.newtable();
                L.pushnil();
                while L.next(i) {
                    L.pushvalue(-2); // copy the key
                    L.insert(-2); // move it behind the value
                    L.settable(-4); // set key=value in the new table
                    // leave behind the key for next
                }
            } else {
                L.pushvalue(i);
            }
        }
        match L.pcall(nargs, 0, 0) {
            Ok(()) => (),
            Err(_) => {
                println!("Error dispatching IRC event: {}", L.describe(-1));
                L.pop(1);
            }
        }
    }
    // we're done
    0
}

// unsafe because the Conn isn't really 'static
unsafe fn getconn(L: &mut lua::State) -> &'static mut Conn<'static> {
    L.pushlightuserdata(lua_require as *mut libc::c_void);
    L.gettable(lua::REGISTRYINDEX);
    let ptr = L.touserdata(-1) as *mut *mut Conn<'static>;
    if ptr.is_null() {
        L.errorstr("could not retrieve connection information");
    }
    let ptr = *ptr;
    if ptr.is_null() {
        L.errorstr("no active connection");
    }
    &mut *ptr
}

extern "C" fn lua_addhandler(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };

    // 2 args: event, func

    unsafe { L.checkbytes(1) };
    L.checktype(2, lua::Type::Function);

    L.settop(2); // throw away any extra values

    // get or create handler table; key is lua_addhandler
    L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
    L.gettable(lua::REGISTRYINDEX);
    if !L.istable(3) {
        L.pop(1);
        L.newtable();
        L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
        L.pushvalue(3);
        L.settable(lua::REGISTRYINDEX);
    }
    // table is stack entry 3

    // get or create the array
    L.pushvalue(1); // copy the event to the top
    L.gettable(3);
    if !L.istable(4) {
        L.pop(1);
        L.newtable();
        L.pushvalue(1); // copy event to top
        L.pushvalue(4);
        L.settable(3);
    }
    // array is stack entry 4

    L.objlen(4); // get table length
    let len = L.tointeger(5) + 1;
    L.pop(1);
    L.pushinteger(len);
    L.pushvalue(2); // copy function to top
    L.settable(4); // set ary[len+1]=func
    // and return
    0
}

extern "C" fn lua_privmsg(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };

    // 2 args: dst, message

    let dst = unsafe { L.checkbytes(1) };
    let msg = unsafe { L.checkbytes(2) };

    let conn = unsafe { getconn(&mut L) };

    conn.privmsg(dst, msg);
    0
}
