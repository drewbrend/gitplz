extern crate app_dirs;
#[macro_use]
extern crate clap;
extern crate gitlib;
extern crate pbr;
extern crate term_painter;

use std::error::Error;
use std::env;
use std::path::{Path, PathBuf};
use std::fs::{File, DirBuilder};
use std::io::Write;
use std::process::exit;

use term_painter::Color::{BrightRed, BrightCyan, BrightGreen, BrightMagenta, BrightYellow};
use term_painter::ToStyle;

use gitlib::{FileStatus, GitError, GitRepo, GitRepositories};

use app_dirs::{AppInfo, AppDataType};

mod cli;

#[derive(Debug)]
enum RunOption {
    Checkout(String),
    Manifest(ManifestOption),
    Reset,
    Status,
}

#[derive(Debug)]
enum ManifestOption {
    Generate(PathBuf),
    Preview,
}


fn main() {
    const APP_INFO: AppInfo = AppInfo {
        name: "git-plz",
        author: "git-plz",
    };

    let working_dir = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            println!("Error getting working directory: {}", err.description());
            exit(1);
        }
    };

    let matches = cli::build_cli().get_matches();

    let option = match matches.subcommand_name() {
        Some(cli::CMD_CHECKOUT) => {
            let branch_match = matches.subcommand_matches(cli::CMD_CHECKOUT).unwrap();
            let branch = value_t!(branch_match, cli::BRANCH, String).unwrap();
            RunOption::Checkout(branch)
        }
        Some(cli::CMD_MANIFEST) => {
            let matches = matches.subcommand_matches(cli::CMD_MANIFEST).unwrap();

            match matches.subcommand_name() {
                Some(cli::CMD_GENERATE) => {
                    let root = match app_dirs::get_app_root(AppDataType::UserCache, &APP_INFO) {
                        Ok(d) => d,
                        Err(e) => {
                            println!("Could not locate app settings directory: {}", e.description());
                            exit(1);
                        }
                    };

                    let mut manifest = PathBuf::from(root);

                    if !manifest.exists() {
                        let mut builder = DirBuilder::new();
                        builder.recursive(true);
                        
                        if let Err(e) = builder.create(&manifest) {
                            println!("Could not create app settings directory: {}", e.description());
                            exit(1);
                        }
                    }

                    manifest.push("manifest.txt");

                    RunOption::Manifest(ManifestOption::Generate(manifest))
                }
                _ => RunOption::Manifest(ManifestOption::Preview),
            }
        }
        Some(cli::CMD_COMPLETIONS) => {
            if let Some(ref matches) = matches.subcommand_matches(cli::CMD_COMPLETIONS) {
                let shell = value_t!(matches, cli::SHELL, clap::Shell).unwrap();
                cli::build_cli().gen_completions_to(cli::APP_NAME, shell, &mut std::io::stdout());
            }

            return;
        }
        Some(cli::CMD_RESET) => RunOption::Reset,

        // By default, just show status.
        _ => RunOption::Status,
    };

    process(&option, &working_dir);
}

fn process(option: &RunOption, path: &Path) {
    let repos = GitRepositories::new(path);

    if let RunOption::Manifest(ref m) = *option {
        match *m {
            ManifestOption::Generate(ref path) => manifest_generate(repos, path).unwrap(),
            ManifestOption::Preview => manifest_preview(repos),
        }
        return;
    }

    for repo in repos {
        match *option {
            RunOption::Checkout(ref branch) => {
                checkout(&repo, branch).unwrap_or_else(|_| println!("Error on checkout"))
            }
            RunOption::Reset => reset(&repo).unwrap(),
            RunOption::Status => status(&repo).unwrap(),
            _ => panic!("Unhandled run option"),
        }
    }
}

fn checkout(repo: &GitRepo, branch: &str) -> Result<(), GitError> {
    repo.checkout(branch)?;

    println!("{}", repo.path().to_str().unwrap());
    //println!("    {}", BrightCyan.paint(branch));

    Ok(())
}

fn manifest_generate(repos: GitRepositories, path: &Path) -> Result<(), GitError> {
    let mut file = File::create(path).unwrap();

    for repo in repos {
        match writeln!(file, "{}", repo.path().to_str().unwrap()) {
            Ok(_) => (),
            Err(_) => (),
        }
    }

    Ok(())
}

fn manifest_preview(repos: GitRepositories) {
    for repo in repos {
        println!("{}", repo.path().to_str().unwrap());
    }
}

fn reset(repo: &GitRepo) -> Result<(), GitError> {
    let head = repo.reset()?;
    let branch = BrightCyan.paint(head.name().unwrap());
    let l_brace = BrightYellow.paint("[");
    let r_brace = BrightYellow.paint("]");

    println!("{}  {}{}{}",
             repo.path().to_str().unwrap(),
             l_brace,
             branch,
             r_brace);

    Ok(())
}

fn status(repo: &GitRepo) -> Result<(), GitError> {
    let statuses = repo.statuses()?;

    if statuses.len() == 0 {
        return Ok(());
    }

    println!("{}", repo.path().to_str().unwrap());

    for entry in statuses.iter() {
        let (pre, colour) = match entry.status() {
            FileStatus::Deleted => ("    Deleted", BrightRed),
            FileStatus::Modified => ("   Modified", BrightCyan),
            FileStatus::New => ("        New", BrightGreen),
            FileStatus::Renamed => ("    Renamed", BrightCyan),
            FileStatus::Typechanged => ("Typechanged", BrightCyan),
            FileStatus::Unknown => ("    Unknown", BrightMagenta),
        };

        println!("  {} {}", colour.paint(pre), entry.path().unwrap());
    }

    Ok(())
}