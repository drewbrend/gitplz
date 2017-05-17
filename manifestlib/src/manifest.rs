use serde_json;
use gitlib::{GitRepo, GitRepositories};
use manifest_iter::ManifestIterator;

use std::path::{PathBuf, Path};
use std::fs::{File, DirBuilder};
use std::io::Write;
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestData {
    root_path: PathBuf,
    repositories: BTreeSet<PathBuf>,
}

impl ManifestData {
    fn empty(path: &Path) -> Self {
        ManifestData {
            repositories: BTreeSet::new(),
            root_path: path.to_path_buf(),
        }
    }

    fn add(&mut self, repo: &GitRepo) {
        let path_strip = match repo.path().strip_prefix(&self.root_path) {
            Ok(p) => p,
            Err(e) => {
                println!("Root path: {:?}", &self.root_path);
                println!("Path: {:?}", repo.path());
                println!("Error: {:?}", e);
                println!("##########################################");
                return;
            }
        };
        let path = PathBuf::from(path_strip.to_str().unwrap());
        self.repositories.insert(path);
    }

    pub fn root(&self) -> &Path {
        &self.root_path
    }

    pub fn repos(&self) -> &BTreeSet<PathBuf> {
        &self.repositories
    }
}

#[derive(Debug)]
pub enum ManifestError {
    BuildPath,
    PathNotFound,
}

#[derive(Debug)]
pub struct Manifest<'a> {
    path: &'a Path,
    data: ManifestData,
}

impl<'a> Manifest<'a> {
    pub fn open<P: AsRef<Path>>(path: &'a P, root: &'a P) -> Result<Self, ManifestError>
    {
        let path_ref = path.as_ref();

        let manifest_data = {
            let root_ref = root.as_ref();

            match path_ref.exists() {
                true => {
                    let file = File::open(path_ref).unwrap();

                    match serde_json::from_reader(&file) {
                        Ok(m) => m,
                        Err(_) => ManifestData::empty(root_ref),
                    }
                }
                false => ManifestData::empty(root_ref),
            }
        };

        Ok(Manifest {
               data: manifest_data,
               path: path_ref,
           })
    }

    pub fn add_repositories(&mut self, repos: GitRepositories) {
        for repo in repos {
            self.data.add(&repo);
        }

        let ser_data = serde_json::to_string_pretty(&self.data).unwrap();
        let mut file = self.get_file();

        match write!(file, "{}", ser_data) {
            Ok(_) => (),
            Err(e) => println!("{:#?}", e),
        }
    }

    fn get_file(&self) -> File {
        if !self.path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(self.path.parent().unwrap())
                .unwrap();
        }

        File::create(&self.path).unwrap()
    }

    pub fn paths(&self) -> ManifestIterator {
        ManifestIterator::new(&self.data)
    }

    pub fn path_in_manifest<P: AsRef<Path>>(&self, path: P) -> bool {
        true
    }
}