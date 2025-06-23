#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]
#![allow(dead_code)]

mod args;
mod args_config;
mod cli_client;
mod exec;
mod list;
mod list_configs;
mod sync;

use clap::Parser;
use cli_client::CliClient;
use list::output_list;
use vopono_core::config::providers::{NonInteractiveUiClient, UiClient};
use list_configs::print_configs;
use log::{LevelFilter, warn};
use sync::{sync_menu, synch};
use vopono_core::util::clean_dead_locks;
use vopono_core::util::clean_dead_namespaces;
use vopono_core::util::elevate_privileges;

fn main() -> anyhow::Result<()> {
    // Get struct of args using structopt
    let app = args::App::parse();
    // Set up logging
    let mut builder = pretty_env_logger::formatted_timed_builder();
    builder.parse_default_env();
    if app.verbose {
        builder.filter_level(LevelFilter::Debug);
    }
    if app.silent {
        if app.verbose {
            warn!("Verbose and silent flags are mutually exclusive, ignoring verbose flag");
        }
        builder.filter_level(LevelFilter::Off);
    }
    builder.init();

    match app.cmd {
        args::Command::Exec(cmd) => {
            let uiclient = CliClient {}; // Exec command likely still needs interactive client for some scenarios
            clean_dead_locks()?;
            let verbose = app.verbose && !app.silent;
            elevate_privileges(app.askpass)?;
            clean_dead_namespaces()?;
            exec::exec(cmd, &uiclient, verbose, app.silent)?
        }
        args::Command::List(listcmd) => {
            clean_dead_locks()?;
            output_list(listcmd)?;
        }
        args::Command::Synch(synchcmd) => {
            let uiclient: Box<dyn UiClient> = if synchcmd.non_interactive {
                Box::new(NonInteractiveUiClient {})
            } else {
                Box::new(CliClient {})
            };
            // If provider given then sync that, else prompt with menu
            if synchcmd.vpn_provider.is_none() {
                sync_menu(uiclient.as_ref(), synchcmd.protocol.map(|x| x.to_variant()))?;
            } else {
                synch(
                    synchcmd.vpn_provider.unwrap().to_variant(),
                    &synchcmd.protocol.map(|x| x.to_variant()),
                    uiclient.as_ref(),
                )?;
            }
        }
        args::Command::Servers(serverscmd) => {
            print_configs(serverscmd)?;
        }
    }
    Ok(())
}
