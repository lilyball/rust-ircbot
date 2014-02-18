use lua;
use irc::conn::Conn;
use std::{libc, mem, ptr};

pub extern "C" fn lua_require(L: *mut lua::raw::lua_State) -> libc::c_int {
    let mut L = unsafe { lua::State::from_lua_State(L) };
    // 1 argument is passed: modname

    let modname: &str = match unsafe { L.checkstring(1) } {
        None => L.errorstr("modname is not valid utf-8"),
        Some(s) => s
    };

    // we're going to store the Conn in an Option in the registry
    // the key for the registry is lua_require as a lightuserdata
    L.pushlightuserdata(lua_require as *mut libc::c_void);
    let connptr = L.newuserdata(mem::size_of::<*mut Conn>()) as *mut *mut Conn;
    unsafe { *connptr = ptr::mut_null() };
    L.settable(lua::REGISTRYINDEX);

    // register our library functions
    L.registerlib(Some(modname), [
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

    1
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

    // get or create handler table; key is lua_addhandler
    L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
    L.gettable(lua::REGISTRYINDEX);
    if !L.istable(-1) {
        L.pop(1);
        L.pushlightuserdata(lua_addhandler as *mut libc::c_void);
        L.newtable();
        L.pushvalue(-1);
        L.insert(-3);
        L.settable(lua::REGISTRYINDEX);
    }

    L.insert(1); // move table to bottom
    L.settable(1); // table[event] = func
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
