/**
- cmd line tools
    - cmds:
        - init - todo: toml config?
        - new - advanced/not needed yet
        - build - flags:optimise, -input and output
        - clean - advanced maybe? depends on how much we define things
        - server ADVANCED

*/
use argh::FromArgs;
use std::path::PathBuf;
use log::{info};

use mole;

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
pub enum SubCommands {
    INIT(InitCommand),
    BUILD(BuildCommand),
    // CLEAN(CleanCommand),
    // NEW(NewCommand),
    // SERVE(ServeCommand)
}

impl SubCommands {
    pub fn run(self) {
        match self {
            SubCommands::INIT(x) => x.run(),
            SubCommands::BUILD(x) => x.run(),
        }
    }
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "init", description = "setups project")]
pub struct InitCommand {}

impl InitCommand {
    pub fn run(self) {
        unimplemented!("just working on build for now")
    }
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(
    subcommand,
    name = "build",
    description = "builds the static site from source"
)]
pub struct BuildCommand {
    #[argh(option, default = "PathBuf::from(\"./_output/\")")]
    /// path to output too
    dest: PathBuf,

    #[argh(option, default = "PathBuf::from(\"_source/\")")]
    /// path to build from
    source: PathBuf,

    #[argh(option, default = "PathBuf::from(\"_include/\")")]
    /// path from 'source' to include folder
    include: PathBuf,

    #[argh(option, default = "PathBuf::from(\"_layouts/\")")]
    /// path from 'source' to layouts folder
    layouts: PathBuf,

    #[argh(option, default = "PathBuf::from(\"_articles/\")")]
    /// path from 'source' to articles folder
    articles: PathBuf,

    // TODO: is this going to be a thing
    #[argh(option, default = "PathBuf::from(\"_js/\")")]
    /// path from 'source' to js folder
    js: PathBuf,
}

impl BuildCommand {
    pub fn run(self) {
        info!("building");
        mole::Build::new()
            .include(&self.include)
            .layouts(&self.layouts)
            .articles(&self.articles)
            .source(&self.source)
            .compile(&self.dest).unwrap();
    }
}