use cos_core::persistence::ConfigStore;
use cos_mcp::kernel::{Kernel, Surface};
use serde_json::{Value, json};
use std::{fs, sync::Arc};
struct Fixture {
    _profile: tempfile::TempDir,
    left: tempfile::TempDir,
    right: tempfile::TempDir,
    config: Arc<ConfigStore>,
    kernel: Arc<Kernel>,
}
impl Fixture {
    fn new() -> Self {
        let profile = tempfile::tempdir().unwrap();
        let left = tempfile::tempdir().unwrap();
        let right = tempfile::tempdir().unwrap();
        let left_path = fs::canonicalize(left.path()).unwrap();
        let right_path = fs::canonicalize(right.path()).unwrap();
        let config = Arc::new(
            ConfigStore::open(
                profile.path(),
                if cfg!(windows) { "win32" } else { "linux" },
            )
            .unwrap(),
        );
        config.update(|value| {
            value["roots"] = json!([{"name":"left","path":left_path},{"name":"right","path":right_path}]);
            value["multiAgent"]["allowUnattributedCalls"] = json!(true);
            value["capabilities"]["command"] = json!(false);
            value["capabilities"]["read"] = json!(false);
        }).unwrap();
        let kernel = Kernel::new(config.clone()).unwrap();
        Self {
            _profile: profile,
            left,
            right,
            config,
            kernel,
        }
    }
    fn find(&self, args: Value) -> Value {
        self.kernel
            .invoke(Surface::Core, "public-find", "find", args)
    }
}
#[test]
fn omitted_path_searches_every_approved_root_with_source_text_metadata() {
    let f = Fixture::new();
    fs::write(f.left.path().join("needle-left.txt"), "public").unwrap();
    fs::write(f.right.path().join("needle-right.txt"), "public").unwrap();
    let result = f.find(json!({"query":"needle"}));
    assert_ne!(result["isError"], true, "{result}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("2 matches\n/left/needle-left.txt\n/right/needle-right.txt\n\nfiles_scanned: 2\nelapsed_ms: "), "{result}");
    assert!(text.ends_with("\nresults_returned: 2"), "{result}");
    assert!(result.get("structuredContent").is_none(), "{result}");
}
#[test]
fn explicit_file_has_source_content_response_and_oversized_file_is_refused() {
    let f = Fixture::new();
    fs::write(f.right.path().join("public.txt"), " public needle \r\n").unwrap();
    let result = f.find(json!({"query":"needle","mode":"content","path":"/right/public.txt"}));
    assert_eq!(
        result,
        json!({"content":[{"type":"text","text":"1 matches\n/right/public.txt:1: public needle"}]})
    );
    fs::File::create(f.left.path().join("large.txt"))
        .unwrap()
        .set_len(2 * 1024 * 1024 + 1)
        .unwrap();
    let result = f.find(json!({"query":"needle","mode":"content","path":"/left/large.txt"}));
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(
        result["content"][0]["text"],
        "File was not searched: content search skips files over 2.0 MB. Narrow it with read start_line/end_line, or use exec_command when command execution is available."
    );
}
#[test]
fn source_find_results_keep_current_search_permission_and_requested_scope() {
    let f = Fixture::new();
    fs::write(f.left.path().join("needle-left.txt"), "public").unwrap();
    fs::write(f.right.path().join("needle-right.txt"), "public").unwrap();
    let result = f.find(json!({"query":"needle", "path":"/right"}));
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.starts_with("1 matches\n/right/needle-right.txt\n\nfiles_scanned: 1\n"),
        "{result}"
    );
    f.config
        .update(|value| value["capabilities"]["search"] = json!(false))
        .unwrap();
    let result = f.find(json!({"query":"needle"}));
    assert_eq!(result["isError"], true, "{result}");
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("TOOL_DISABLED"),
        "{result}"
    );
}

#[test]
fn native_find_matches_the_actual_registered_source_handler() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../fixtures/upstream/phase1-6-closure/search-tool.json"
    ))
    .unwrap();
    let f = Fixture::new();
    for (path, descriptor) in corpus["files"].as_object().unwrap() {
        let (root, name) = path.trim_start_matches('/').split_once('/').unwrap();
        let path = if root == "left" {
            f.left.path()
        } else {
            f.right.path()
        }
        .join(name);
        if let Some(size) = descriptor["zeroBytes"].as_u64() {
            fs::File::create(path).unwrap().set_len(size).unwrap();
        } else {
            fs::write(path, descriptor["text"].as_str().unwrap()).unwrap();
        }
    }
    let elapsed = regex::Regex::new(r"elapsed_ms: \d+").unwrap();
    // Directory order is an input to the source's scan count. Select the exact
    // captured provider input, rather than rewriting a count or sorting product results.
    let order = fs::read_dir(f.left.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    let order = json!(order);
    let variant = corpus["fileOrderVariants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["leftEntries"] == order)
        .expect("Both native two-entry permutations are captured from source");
    for row in variant["cases"].as_array().unwrap() {
        let mut output = f.find(row["args"].clone());
        for item in output["content"].as_array_mut().unwrap() {
            if let Some(text) = item["text"].as_str() {
                item["text"] = json!(elapsed.replace_all(text, "elapsed_ms: <elapsed>"));
            }
        }
        assert_eq!(output, row["expected"], "{row}");
    }
}
