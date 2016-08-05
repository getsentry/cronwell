use std::env;
use std::mem;
use std::process;
use std::time;
use std::process::{Command, Stdio};

use error::Error;
use monitorid::MonitorId;
use processtools::{ProcessIterator, LineBuffer, get_unix_exit_status};

use clap::{App, AppSettings, Arg};


pub fn make_app<'a, 'b: 'a>() -> App<'a, 'b> {
    App::new("cronwell")
        .about("Sentry cron monitoring utility")
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::AllowExternalSubcommands)
        .arg(Arg::with_name("monitor_id")
             .value_name("MONITOR")
             .long("monitor")
             .short("m")
             .help("The monitor identifier"))
        .arg(Arg::with_name("info")
             .long("info")
             .help("Print basic information about the monitor quit"))
        .arg(Arg::with_name("start")
             .long("start")
             .help("Report the start of a monitor job"))
        .arg(Arg::with_name("complete")
             .long("complete")
             .help("Report a successful completion for this monitor"))
        .arg(Arg::with_name("fail")
             .long("fail")
             .help("Report a fail for this monitor"))
        .arg(Arg::with_name("quiet")
             .long("quiet")
             .short("q")
             .help("Disable output from the process"))
}

fn get_monitor_id(s: Option<&str>) -> Result<MonitorId, Error> {
    if let Some(val) = s {
        return val.parse();
    }
    if let Ok(val) = env::var("CRONWELL_MONITOR") {
        if !val.is_empty() {
            return val.parse();
        }
    }
    fail!("No monitor token provided.");
}

fn print_monitor_info(id: &MonitorId) -> Result<(), Error> {
    println!("API Endpoint: {}", id.api_url());
    println!("Token: {}", id.token());
    println!("Secure reporting: {}", if id.is_secure() { "yes" } else { "no" });
    Ok(())
}

fn run_command(id: &MonitorId, cmd: &str, args: &[&str],
               quiet: bool) -> Result<(), Error> {
    id.report_start(cmd, args).ok();

    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut buf = LineBuffer::new(200);

    // while there is output, output it
    {
        let iter = ProcessIterator::new(&mut child);
        for chunk in iter {
            if !quiet {
                chunk.echo().ok();
            }
            buf.append_chunk(&chunk);
        }
    }

    let status = child.wait().ok().and_then(get_unix_exit_status).unwrap_or(255);

    if status == 0 {
        id.report_complete().ok();
    } else {
        id.report_failure(buf.into_iter(), status).ok();
    }

    process::exit(status);
}

pub fn execute() -> Result<(), Error> {
    let args : Vec<String> = env::args().collect();

    let matches = make_app().get_matches_from_safe(args)?;
    let id = get_monitor_id(matches.value_of("monitor_id"))?;

    if matches.is_present("info") {
        print_monitor_info(&id)?;
    } else if matches.is_present("start") {
        //id.report_start()?;
    } else if matches.is_present("complete") {
        //id.report_status(true)?;
    } else if matches.is_present("fail") {
        //id.report_status(false)?;
    } else {
        let quiet = matches.is_present("quiet");
        match matches.subcommand() {
            (exe, Some(exe_matches)) => {
                let args = match exe_matches.values_of("") {
                    Some(args) => args.collect(),
                    None => vec![],
                };
                run_command(&id, exe, &args[..], quiet)?;
            }
            _ => {}
        }
    }

    Ok(())
}
