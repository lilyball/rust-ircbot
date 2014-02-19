/// Handle stdin commands

use {Cmd, State};
use std::{io,task};
use sync::MutexArc;
use irc::conn::Conn;

/// Spawns a new (unwatched) task to handle stdin
pub fn spawn_stdin_listener(arc: MutexArc<Option<Chan<Cmd>>>) {
    task::task().named("stdin listener").spawn(proc() {
        handle_stdin(arc);
    });
}

fn handle_stdin(arc: MutexArc<Option<Chan<Cmd>>>) {
    let mut stdin = io::BufferedReader::new(io::stdin());
    for line in stdin.lines() {
        match parse_line(line) {
            None => (),
            Some(cmd) => {
                let mut cmd = Some(cmd);
                if !arc.access(|chan| {
                    match *chan {
                        None => true,
                        Some(ref c) => c.try_send(cmd.take_unwrap())
                    }
                }) {
                    println!("Error: no active connection");
                }
            }
        }
    }
}

fn parse_line(line: &str) -> Option<Cmd> {
    if !line.starts_with("/") {
        return None;
    }
    let line = line.trim_right_chars(& &['\r', '\n']);
    let mut iter = line.slice_from(1).splitn(' ', 1);
    let cmd = iter.next().unwrap();
    let line = iter.next().unwrap_or("");
    match cmd {
        "msg" => cmd_msg(line),
        "join" => cmd_join(line),
        "part" => cmd_part(line),
        "quit" => cmd_quit(line),
        "raw" => cmd_raw(line),
        "reload" => cmd_reload(line),
        _ => None
    }
}

fn parse_word<'a>(line: &'a str) -> (&'a str, &'a str) {
    let line = line.trim_left();
    match line.find(' ') {
        None => (line, ""),
        Some(i) => {
            (line.slice_to(i), line.slice_from(i+1))
        }
    }
}

fn cmd_msg(line: &str) -> Option<Cmd> {
    let (dst, msg) = parse_word(line);
    if dst == "" || msg == "" {
        return None;
    }

    let dst = dst.to_owned();
    let msg = msg.to_owned();
    Some(proc(conn: &mut Conn, _state: &mut State) {
        conn.privmsg(dst.as_bytes(), msg.as_bytes());
    })
}

fn cmd_join(line: &str) -> Option<Cmd> {
    let (chans, line) = parse_word(line);
    let line = line.trim_left();
    if chans == "" {
        return None;
    }

    let chans = chans.to_owned();
    let keys = if line == "" { None } else { Some(line.to_owned()) };
    Some(proc(conn: &mut Conn, _state: &mut State) {
        conn.join(chans.as_bytes(), keys.as_ref().map_or(&[], |s| s.as_bytes()));
    })
}

fn cmd_part(line: &str) -> Option<Cmd> {
    let (chans, msg) = parse_word(line);
    if chans == "" {
        return None;
    }

    let chans = chans.to_owned();
    let msg = if msg == "" { None } else { Some(msg.to_owned()) };
    Some(proc(conn: &mut Conn, _state: &mut State) {
        conn.part(chans.as_bytes(), msg.as_ref().map_or(&[], |s| s.as_bytes()));
    })
}

fn cmd_quit(line: &str) -> Option<Cmd> {
    let line = line.trim_left();
    let line = if line == "" { None } else { Some(line.to_owned()) };
    Some(proc(conn: &mut Conn, _state: &mut State) {
        conn.quit(line.as_ref().map_or(&[], |s| s.as_bytes()));
    })
}

fn cmd_raw(line: &str) -> Option<Cmd> {
    let line = line.to_owned();
    Some(proc(conn: &mut Conn, _state: &mut State) {
        conn.send_raw(line.as_bytes());
    })
}

fn cmd_reload(_line: &str) -> Option<Cmd> {
    Some(proc(conn: &mut Conn, state: &mut State) {
        println!("Reloading plugins...");
        state.plugins.reload_plugins(conn);
    })
}
