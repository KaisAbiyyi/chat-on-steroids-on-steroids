use cos_tools::search::{
    Options,
    engine::{self, Backend, Entry, Kind, Metadata},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    time::{Duration, Instant},
};
struct Tree {
    opened: usize,
    directories: BTreeMap<String, Vec<(String, String)>>,
    files: BTreeMap<String, String>,
}
impl Backend for Tree {
    fn metadata(&mut self, path: &str) -> io::Result<Metadata> {
        if self.directories.contains_key(path) {
            Ok(Metadata {
                kind: Kind::Directory,
                bytes: 0,
            })
        } else if let Some(text) = self.files.get(path) {
            Ok(Metadata {
                kind: Kind::File,
                bytes: text.len() as u64,
            })
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }
    fn open_folder(
        &mut self,
        path: &str,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Entry>> + '_>> {
        self.opened += 1;
        let entries = self
            .directories
            .get(path)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?
            .clone();
        Ok(Box::new(entries.into_iter().map(|(name, kind)| {
            Ok(Entry {
                name,
                kind: match kind.as_str() {
                    "file" => Kind::File,
                    "directory" => Kind::Directory,
                    _ => Kind::Other,
                },
            })
        })))
    }
    fn read_file(
        &mut self,
        path: &str,
        _: usize,
        authorize: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        authorize()?;
        self.files
            .get(path)
            .map(|text| text.as_bytes().to_vec())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}
fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/search-traversal.json"
    ))
    .unwrap()
}
fn projection(outcome: engine::Outcome) -> Value {
    let mut value = serde_json::to_value(outcome).unwrap();
    value.as_object_mut().unwrap().remove("elapsedMs");
    value
}
#[test]
fn ordered_breadth_first_traversal_matches_actual_source_scenarios() {
    let corpus = corpus();
    for row in corpus["trees"].as_array().unwrap() {
        let mut tree = Tree {
            opened: 0,
            directories: serde_json::from_value(corpus["directories"].clone()).unwrap(),
            files: serde_json::from_value(corpus["files"].clone()).unwrap(),
        };
        tree.directories = tree
            .directories
            .into_iter()
            .map(|(key, value)| (key.replacen("/public", "/p", 1), value))
            .collect();
        tree.files = tree
            .files
            .into_iter()
            .map(|(key, value)| (key.replacen("/public", "/p", 1), value))
            .collect();
        let options = &row["options"];
        let exclude: Option<Vec<String>> = options
            .get("exclude")
            .map(|value| serde_json::from_value(value.clone()).unwrap());
        let outcome = engine::search(
            &mut tree,
            "/p",
            &Options {
                query: options["query"].as_str().unwrap(),
                glob: options["include"].as_str(),
                limit: options["maxResults"].as_u64().unwrap_or(50) as usize,
                exclude: exclude.as_deref(),
                case_sensitive: options["caseSensitive"] == true,
                ..Options::new("")
            },
            Instant::now() + Duration::from_secs(10),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(projection(outcome), row["expected"], "{row}");
    }
}
#[test]
fn file_and_directory_admission_match_source_bounds_without_large_disk_fixtures() {
    for row in corpus()["bounds"].as_array().unwrap() {
        let count = row["options"]["files"].as_u64().unwrap() as usize;
        let folders = row["options"]["folders"].as_u64().unwrap() as usize;
        let mut tree = Tree {
            opened: 0,
            directories: BTreeMap::from([("/p".into(), vec![])]),
            files: BTreeMap::new(),
        };
        for i in 0..folders {
            tree.directories
                .get_mut("/p")
                .unwrap()
                .push((format!("dir-{i}"), "directory".into()));
            tree.directories.insert(format!("/p/dir-{i}"), vec![]);
        }
        for i in 0..count {
            tree.directories
                .get_mut("/p")
                .unwrap()
                .push((format!("public-{i}.txt"), "file".into()));
        }
        let result = engine::search(
            &mut tree,
            "/p",
            &Options::new("absent"),
            Instant::now() + Duration::from_secs(10),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(projection(result), row["expected"], "{row}");
    }
}
#[test]
fn an_expired_shared_deadline_does_not_open_a_directory() {
    let mut tree = Tree {
        opened: 0,
        directories: BTreeMap::from([("/p".into(), vec![])]),
        files: BTreeMap::new(),
    };
    let deadline = Instant::now() - Duration::from_millis(1);
    let result = engine::search(&mut tree, "/p", &Options::new("absent"), deadline, &|| {
        Ok(())
    })
    .unwrap();
    assert_eq!(
        projection(result),
        json!({"hits":[], "filesScanned":0, "truncated":true,"stoppedBecause":"time"})
    );
    assert_eq!(tree.opened, 0);
}
