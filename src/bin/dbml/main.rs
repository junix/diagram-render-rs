//! Bounded authored DBML JSON provider; this does not accept DBML source text.
#![forbid(unsafe_code)]
use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;

mod provider_io;
mod provider_json;
mod provider_model;
mod provider_paths;
mod provider_publication;
mod provider_receipt;
mod provider_render;

const PROVIDER_ID: &str = "plot-provider-dbml";
const PROFILE: &str = "dbml-authored-svg/1";
const RENDERER_REVISION: &str = "8203dbe909d588ad0239f0bd77f9b439f48111e5";
const PARSER_REVISION: &str = "1bb33fbd22b6a51580021a157f30e1484221f0f5";
const THEME_REVISION: &str = "0d6530f0ccea5a61b0d2fef9a3e164ff816ca852";

#[derive(Parser)]
#[command(name = PROVIDER_ID, version = version(), about = "Render a closed authored DBML JSON subset through the native card renderer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Describe this fixed contract without inspecting input files.
    Describe {
        #[arg(long)]
        json: bool,
    },
    /// Report the linked renderer's readiness, not input validity.
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Render explicit authored JSON to SVG and a mandatory typed receipt.
    RenderSvg(provider_render::Options),
}
fn version() -> String {
    match option_env!("PM_BUILD_SHA") {
        Some(stamp) => format!("{}+{}", env!("CARGO_PKG_VERSION"), stamp),
        None => env!("CARGO_PKG_VERSION").into(),
    }
}
fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Describe { json: machine } => {
            let command: serde_json::Value = serde_json::from_str(include_str!(
                "../../../assets/dbml-render-svg-command-v1.json"
            ))?;
            let description = json!({
                "schema_version":"plot-provider-dbml.describe/v1",
                "provider":{"id":PROVIDER_ID,"version":version(),"protocol_versions":[1]},
                "source":{"local_code_path":env!("CARGO_MANIFEST_DIR")},
                "operations":["render-svg"],
                "commands":[command,{"name":"describe"},{"name":"doctor"}]
            });
            if machine {
                println!("{}", serde_json::to_string(&description)?);
            } else {
                println!("{PROVIDER_ID} {} ({PROFILE}): render-svg", version());
            }
        }
        Commands::Doctor { json: machine } => {
            if machine {
                println!(
                    "{}",
                    json!({
                        "schema_version":"plot-provider-dbml.doctor/v1",
                        "provider":{"id":PROVIDER_ID,"version":version()},
                        "ok":true,"checks":[{"name":"linked-native-svg-renderer","ok":true}],
                        "notes":["Closed authored DBML JSON only. Input, exact byte pin, labels and identities are validated at render time. No source parsing, imports, network, font discovery or external rendering process. Filesystem checks are not an OS sandbox."]
                    })
                );
            } else {
                println!(
                    "linked native SVG renderer ready; authored input requires render-time validation"
                );
            }
        }
        Commands::RenderSvg(options) => provider_render::run(options)?,
    }
    Ok(())
}
