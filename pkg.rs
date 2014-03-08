#[crate_id="github.com/kballard/rust-ircbot#rustirc:0.1"];
#[crate_type="bin"];

#[feature(phase)];
#[feature(default_type_params)];

#[phase(syntax,link)]
extern crate lua = "github.com/kballard/rust-lua#lua:0.1";
extern crate irc = "github.com/kballard/rust-irclib#irc:0.1";
extern crate toml = "github.com/mneumann/rust-toml#toml:0.1";
extern crate extra;
extern crate getopts;
extern crate sync;

use std::os;
use std::io;
use std::io::signal::{Listener, Interrupt};
use std::task;
use irc::conn;
use irc::conn::{Conn, Line, Event, IRCCode};

pub mod config;
pub mod stdin;

pub mod plugins;

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

    // use a MutexArc to hold the channel for stdin
    // This way we can swap it out on reconnections and stdin will work
    let arc = sync::MutexArc::new(None);

    // spawn the stdin listener now to control the bot
    stdin::spawn_stdin_listener(arc.clone());

    // create the reconnect timer, later used to sleep between connections
    let mut recon_timer = io::timer::Timer::new().ok()
                          .expect("could not create reconnection timer");
    // reconnect time, used for exponential backoff
    let mut recon_delay = conf.reconnect_time;

    // connect in a loop, based on the reconnection config
    println!("Connecting...");
    loop {
        match connect(&conf, &arc) {
            Ok(()) => {
                // bot quit gracefully
                println!("Exiting...");
                break;
            }
            Err(err) => {
                // some error occurred
                println!("Connection error: {}", err);
                match err {
                    conn::ErrIO(_) => {
                        // reset the reconnect delay, we successfully connected
                        recon_delay = conf.reconnect_time;
                    }
                    _ => ()
                }
            }
        }

        arc.access(|c| *c = None);

        match recon_delay {
            None => break,
            Some(mut secs) => {
                recon_timer.sleep(secs as u64 * 1000);
                if conf.reconnect_backoff {
                    // ad-hoc backoff
                    secs = match secs {
                        0   .. 4   => 5,
                        5   .. 9   => 10,
                        10  .. 19  => 20,
                        20  .. 29  => 30,
                        30  .. 59  => 60,
                        61  .. 149 => 150,
                        151 .. 299 => 300,
                        s => s + 60
                    };
                    recon_delay = Some(secs);
                }
            }
        }
        println!("Reconnecting...");
    }

    // some task is keeping us alive, so kill it
    unsafe { ::std::libc::exit(0); }
}

/// Payload for the Conn
pub struct State {
    plugins: plugins::PluginManager
}

pub type Cmd = conn::Cmd<State>;

fn connect(conf: &config::Config, arc: &sync::MutexArc<Option<Chan<Cmd>>>) -> conn::Result {
    // TODO: eventually we should support multiple servers
    let server = &conf.servers[0];
    let mut opts = irc::conn::Options::new(server.host, server.port);
    opts.nick = server.nick.as_slice();
    opts.user = server.user.as_slice();
    opts.real = server.real.as_slice();

    let (cmd_port, cmd_chan) = Chan::new();
    opts.commands = Some(cmd_port);

    // give stdin the new channel
    arc.access(|c| *c = Some(cmd_chan.clone()));

    // intercept ^C and use it to quit gracefully
    let mut listener = Listener::new();
    if listener.register(Interrupt).is_ok() {
        let cmd_chan2 = cmd_chan.clone();
        task::task().named("signal handler").spawn(proc() {
            let mut listener = listener;
            let cmd_chan = cmd_chan2;
            loop {
                match listener.port.recv() {
                    Interrupt => {
                        cmd_chan.try_send(proc(conn: &mut Conn, _state: &mut State) {
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

    let state = State { plugins: plugins::PluginManager::new(conf) };

    let autojoin = server.autojoin.as_slice();

    println!("Connecting to {}...", opts.host);
    irc::conn::connect(opts, state, |conn, event, state| handler(conn, event, state, autojoin))
}

fn handler(conn: &mut Conn, event: Event, state: &mut State, autojoin: &[config::Channel]) {
    match event {
        irc::conn::Connected => println!("Connected"),
        irc::conn::Disconnected => println!("Disconnected"),
        irc::conn::LineReceived(ref line) => {
            let Line{ref command, args: _, prefix: _} = *line;
            match *command {
                IRCCode(1) => {
                    println!("Logged in");
                    for chan in autojoin.iter() {
                        println!("Joining {}", chan.name);
                        conn.join(chan.name.as_bytes(), []);
                    }
                }
                _ => ()
            }
        }
    }
    state.plugins.dispatch_irc_event(conn, &event);
}
