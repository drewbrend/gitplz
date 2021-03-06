extern crate app_dirs;
#[macro_use]
extern crate clap;
//extern crate indicatif;
extern crate num_cpus;
extern crate term_painter;
extern crate threadpool;

extern crate gitlib;
extern crate util;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use app_dirs::{AppInfo, AppDataType};
//use indicatif::{ProgressBar, ProgressStyle};
use term_painter::Color::{BrightCyan, BrightYellow};
use term_painter::ToStyle;
use threadpool::ThreadPool;

use gitlib::{GitError, GitRepo};
use util::{GitRepositories, Manifest};

mod cli;
mod status;

const THREAD_SIGNAL: &str = "Could not signal main thread";

#[derive(Debug, Clone)]
enum RunOption {
    Checkout(String),
    Manifest(ManifestOption),
    Reset,
    Status,
}

#[derive(Debug, Clone)]
enum ManifestOption {
    Clean,
    Preview,
    Update,
}

fn main() {
    let working_dir = env::current_dir().expect("Could not get working directory");
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
                Some(cli::CMD_CLEAN) => RunOption::Manifest(ManifestOption::Clean),
                Some(cli::CMD_UPDATE) => RunOption::Manifest(ManifestOption::Update),
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

    process(option, &working_dir);
}

fn process(option: RunOption, path: &Path) {
    let manifest_path = build_manifest_path();
    let mut manifest = Manifest::open(&manifest_path, &path);

    if let RunOption::Manifest(ref m) = option {
        match *m {
            ManifestOption::Clean => manifest_clean(&manifest_path),
            ManifestOption::Preview => manifest_preview(path),
            ManifestOption::Update => manifest_update(path, &mut manifest),
        }

        return;
    }

    let repos = match manifest.is_empty() {
        true => GitRepositories::new(path),
        false => GitRepositories::from_manifest(&manifest),
    };

    let pool = {
        let thread_count = num_cpus::get();
        ThreadPool::new(thread_count)
    };

    match option {
        RunOption::Reset => {
            let rx = reset(repos, &pool);

            while let Ok((path, head)) = rx.recv() {
                let branch = BrightCyan.paint(head);
                let l_brace = BrightYellow.paint("[");
                let r_brace = BrightYellow.paint("]");

                println!("  {}{}{}  {}", l_brace, branch, r_brace, path.display());
            }
        }
        RunOption::Status => status::process_status(repos, &pool),
        _ => panic!("Unhandled run option"),
    }
}

fn build_manifest_path() -> PathBuf {
    const APP_INFO: AppInfo = AppInfo {
        name: "git-plz",
        author: "devnought",
    };

    let root = app_dirs::get_app_root(AppDataType::UserCache, &APP_INFO)
        .expect("Could not locate app settings directory");
    let mut path = PathBuf::from(root);
    path.push("manifest.json");

    path
}

fn manifest_update<P>(path: P, manifest: &mut Manifest)
    where P: AsRef<Path>
{
    let repos = GitRepositories::new(path);

    manifest.add_repositories(repos);

    println!("{:#?}", &manifest);
}

fn manifest_preview<P>(path: P)
    where P: AsRef<Path>
{
    let repos = GitRepositories::new(path);

    for repo in repos {
        println!("{}", repo.path().display());
    }
}

fn manifest_clean<P>(manifest_path: P)
    where P: AsRef<Path>
{
    let manifest_path = manifest_path.as_ref();
    println!("Attempting to delete: {}", manifest_path.display());

    if manifest_path.exists() {
        fs::remove_file(manifest_path).expect("Could not delete manifest");
    }
}

fn checkout(repo: &GitRepo, branch: &str) -> Result<(), GitError> {
    repo.checkout(branch)?;

    println!("{}", repo.path().display());
    println!("    {}", BrightCyan.paint(branch));

    Ok(())
}

fn reset(repos: GitRepositories, pool: &ThreadPool) -> Receiver<(PathBuf, String)> {
    let (tx, rx) = channel();

    for repo in repos {
        let tx = tx.clone();

        pool.execute(move || {
            match repo.statuses() {
                Ok(s) => {
                    if s.len() == 0 {
                        return;
                    }
                }
                _ => (),
            }

            match repo.remove_untracked() {
                Err(_) => return,
                _ => (),
            }

            let head = match repo.reset() {
                Ok(r) => r,
                _ => return,
            };

            let tuple = (repo.path().to_path_buf(), head.name().to_string());
            tx.send(tuple).expect(THREAD_SIGNAL);
        });
    }

    rx
}
