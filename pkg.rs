#[crate_id="github.com/kballard/rust-ircbot#rustirc:0.1"];
#[crate_type="bin"];

extern mod extra;
extern mod lua;
extern mod irc;
extern mod toml;
extern mod getopts;

use std::os;
use std::io::signal::{Listener, Interrupt};
use std::task;
use irc::conn::{Conn, Line, Event, IRCCmd, IRCCode, IRCAction};

pub mod config;
pub mod stdin;

fn main() {
    let conf = match config::parse_args() {
        Ok(c) => c,
        Err(_) => {
            os::set_exit_status(2);
            return;
        }
    };

    if conf.servers.is_empty() {
        println!("No servers are specified");
        println!("Exiting...");
        return;
    }

    // TODO: eventually we should support multiple servers
    let server = &conf.servers[0];
    let mut opts = irc::conn::Options::new(server.host, server.port);
    opts.nick = server.nick.as_slice();
    opts.user = server.user.as_slice();
    opts.real = server.real.as_slice();

    let (cmd_port, cmd_chan) = Chan::new();
    opts.commands = Some(cmd_port);

    // read stdin to control the bot
    stdin::spawn_stdin_listener(cmd_chan.clone());

    // intercept ^C and use it to quit gracefully
    let mut listener = Listener::new();
    if listener.register(Interrupt).is_ok() {
        let cmd_chan2 = cmd_chan.clone();
        let mut t = task::task();
        t.unwatched();
        t.name("signal handler");
        t.spawn(proc() {
            let mut listener = listener;
            let cmd_chan = cmd_chan2;
            loop {
                match listener.port.recv() {
                    Interrupt => {
                        cmd_chan.try_send(proc(conn: &mut Conn) {
                            conn.quit([]);
                        });
                        listener.unregister(Interrupt);
                        break;
                    }
                    _ => ()
                }
            }
        });
    } else {
        warn!("Couldn't register ^C signal handler");
    }

    let autojoin = server.autojoin.as_slice();

    println!("Connecting to {}...", opts.host);
    match irc::conn::connect(opts, |conn, event| handler(conn, event, autojoin)) {
        Ok(()) => println!("Exiting..."),
        Err(err) => println!("Connection error: {}", err)
    }

    // some task is keeping us alive, so kill it
    unsafe { ::std::libc::exit(0); }
}

fn handler(conn: &mut Conn, event: Event, autojoin: &[config::Channel]) {
    match event {
        irc::conn::Connected => println!("Connected"),
        irc::conn::Disconnected => println!("Disconnected"),
        irc::conn::LineReceived(line) => {
            let Line{command, args, prefix} = line;
            match command {
                IRCCode(1) => {
                    println!("Logged in");
                    for chan in autojoin.iter() {
                        println!("Joining {}", chan.name);
                        conn.join(chan.name.as_bytes(), []);
                    }
                }
                IRCCmd(~"PRIVMSG") if prefix.is_some() && !args.is_empty() => {

                }
                IRCAction(_dst) => {
                    if prefix.is_none() || args.is_empty() { return; }

                }
                _ => ()
            }
        }
    }
}
