use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/settings.png")
}

fn seed_index_for_file(path: &std::path::Path, title: &str) {
    if let Ok(mut idx) = gyotaku_core::Index::open_default() {
        let mtime = path
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            })
            .unwrap_or(0);
        let _ = idx.insert(
            &gyotaku_core::Shot {
                path: path.to_path_buf(),
                mtime,
                width: 500,
                height: 500,
                look: None,
            },
            &[gyotaku_core::Line {
                text: title.to_string(),
                rect: gyotaku_core::Rect {
                    x: 0.1,
                    y: 0.05,
                    w: 0.8,
                    h: 0.1,
                },
                score: 0.95,
            }],
        );
    }
}

#[test]
fn test_batch_name_missing_directory_fails_closed() {
    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg("non_existent_folder_xyz_12345")
        .output()
        .expect("failed to execute binary");

    assert!(!output.status.success(), "must fail on missing folder");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "{stderr}");
}

#[test]
fn test_batch_name_dry_run_leaves_files_untouched() {
    let temp_dir = std::env::temp_dir().join("gyotaku_dry_run_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let test_file = temp_dir.join("test_settings_input.png");
    fs::copy(&fixture, &test_file).expect("copy fixture");

    seed_index_for_file(&test_file, "Docker vs Podman Comparison");

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .arg("--dry-run")
        .arg("--format")
        .arg("json")
        .output()
        .expect("execute gyotaku name");

    assert!(output.status.success(), "dry-run should succeed");

    // Verify original file was NOT renamed or deleted
    assert!(
        test_file.exists(),
        "original file must still exist untouched in dry-run"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test_settings_input.png"), "{stdout}");
    assert!(stdout.contains("comparison_docker_vs_podman"), "{stdout}");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_limit_selects_first_images_in_path_order() {
    let temp_dir = std::env::temp_dir().join("gyotaku_limit_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    for name in ["c.png", "a.png", "b.png"] {
        let path = temp_dir.join(name);
        fs::copy(&fixture, &path).expect("copy fixture");
        let seconds = match name {
            "a.png" => 1_700_000_000,
            "b.png" => 1_700_000_100,
            _ => 1_700_000_200,
        };
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
            )
            .unwrap();
        seed_index_for_file(&path, "Modern Git Cheat Sheet");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .args(["--limit", "2", "--sort", "name", "--format", "json"])
        .output()
        .expect("execute gyotaku name --limit");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items[0]["source_path"].as_str().unwrap().ends_with("a.png"));
    assert!(items[1]["source_path"].as_str().unwrap().ends_with("b.png"));

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .args(["--limit", "2", "--format", "json"])
        .output()
        .expect("execute gyotaku name with newest-first default");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items[0]["source_path"].as_str().unwrap().ends_with("c.png"));
    assert!(items[1]["source_path"].as_str().unwrap().ends_with("b.png"));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_csv_format() {
    let temp_dir = std::env::temp_dir().join("gyotaku_csv_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let test_file = temp_dir.join("test_csv_input.png");
    fs::copy(&fixture, &test_file).expect("copy fixture");

    seed_index_for_file(&test_file, "Modern Git Cheat Sheet");

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&test_file)
        .arg("--format")
        .arg("csv")
        .output()
        .expect("execute gyotaku name csv");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "failed: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("source_path,recommended_filename,confidence,nature,data_gaps"),
        "{stdout}"
    );
    assert!(stdout.contains("cheat_sheet_modern_git"), "{stdout}");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_script_format() {
    let temp_dir = std::env::temp_dir().join("gyotaku_script_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let test_file = temp_dir.join("test_script_input.png");
    fs::copy(&fixture, &test_file).expect("copy fixture");

    seed_index_for_file(&test_file, "Clean Architecture in Rust");

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&test_file)
        .arg("--format")
        .arg("script")
        .output()
        .expect("execute gyotaku name script");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "failed: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[System.IO.File]::Move") || stdout.contains("mv -n"),
        "{stdout}"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_apply_renames_without_copying() {
    let temp_dir = std::env::temp_dir().join("gyotaku_apply_rename_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let original_file = temp_dir.join("original_diagram.png");
    fs::copy(&fixture, &original_file).expect("copy fixture");

    seed_index_for_file(&original_file, "Clean Architecture in Rust");

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&original_file)
        .arg("--apply")
        .arg("-y")
        .output()
        .expect("execute gyotaku name --apply -y");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "failed: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Successfully renamed 1 files"), "{stdout}");

    let expected_target = temp_dir.join("architecture_clean_architecture_in_rust.png");
    assert!(!original_file.exists(), "old filename must be gone");
    assert!(expected_target.exists(), "renamed file must exist");
    assert_eq!(
        fs::read(&expected_target).unwrap(),
        fs::read(&fixture).unwrap()
    );
    assert_eq!(fs::read_dir(&temp_dir).unwrap().count(), 1);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_repeated_runs_are_idempotent() {
    let temp_dir = std::env::temp_dir().join("gyotaku_idempotent_runs_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let original_file = temp_dir.join("original_diagram.png");
    fs::copy(&fixture, &original_file).expect("copy fixture");

    seed_index_for_file(&original_file, "Clean Architecture in Rust");

    // Run 1: initial apply
    let output1 = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .arg("--apply")
        .arg("-y")
        .output()
        .expect("execute gyotaku name --apply -y");
    assert!(output1.status.success());
    let target = temp_dir.join("architecture_clean_architecture_in_rust.png");
    assert!(target.exists(), "renamed file must exist after run 1");
    assert!(!original_file.exists());

    // Seed index for the newly created target so OCR doesn't run offline in tests
    seed_index_for_file(&target, "Clean Architecture in Rust");

    // Run 2: repeat on the same folder
    let output2 = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .arg("--apply")
        .arg("-y")
        .output()
        .expect("execute gyotaku name --apply -y repeat");
    assert!(output2.status.success());
    assert!(
        String::from_utf8_lossy(&output2.stdout).contains("Successfully renamed 0 files"),
        "second run should report no new renames"
    );

    // Critical assertion: NO duplicate _2.png or _3.png files should be created!
    let target_2 = temp_dir.join("architecture_clean_architecture_in_rust_2.png");
    let target_3 = temp_dir.join("architecture_clean_architecture_in_rust_3.png");
    assert!(
        !target_2.exists(),
        "repeated runs must not create _2 duplicate file"
    );
    assert!(
        !target_3.exists(),
        "repeated runs must not create _3 duplicate file"
    );

    let script = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&temp_dir)
        .args(["--format", "script"])
        .output()
        .expect("generate script after rename");
    assert!(script.status.success());
    assert!(
        !String::from_utf8_lossy(&script.stdout).contains("original_diagram.png"),
        "script must not rename an image already named"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_occupied_target_refuses_overwrite() {
    let temp_dir = std::env::temp_dir().join("gyotaku_occupied_target_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let original_file = temp_dir.join("original_diagram.png");
    fs::copy(&fixture, &original_file).expect("copy fixture");

    // Pre-create the expected base target file with sentinel content
    let occupied_target = temp_dir.join("architecture_clean_architecture_in_rust.png");
    fs::write(&occupied_target, b"occupied destination sentinel data").unwrap();

    seed_index_for_file(&original_file, "Clean Architecture in Rust");

    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&original_file)
        .arg("--apply")
        .arg("-y")
        .output()
        .expect("execute gyotaku name --apply -y");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "run should succeed: {stderr}");
    assert!(stdout.contains("Successfully renamed 1 files"), "{stdout}");

    // Verify sentinel content was NOT overwritten
    let target_data = fs::read(&occupied_target).unwrap();
    assert_eq!(
        target_data, b"occupied destination sentinel data",
        "pre-existing occupied target file must not be overwritten"
    );

    // Verify non-colliding numbered file was created instead
    let resolved_target = temp_dir.join("architecture_clean_architecture_in_rust_2.png");
    assert!(
        resolved_target.exists(),
        "numbered non-colliding file must be created"
    );
    assert!(!original_file.exists(), "source name must be removed");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_fallback_error_fails() {
    let temp_dir = std::env::temp_dir().join("gyotaku_fallback_error_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let fixture = fixture_path();
    let test_file = temp_dir.join("unindexed_image.png");
    fs::copy(&fixture, &test_file).expect("copy fixture");

    // Do NOT seed the index. Running with --fallback error should fail closed.
    let output = Command::new(env!("CARGO_BIN_EXE_gyotaku"))
        .arg("name")
        .arg(&test_file)
        .arg("--fallback")
        .arg("error")
        .output()
        .expect("execute gyotaku name --fallback error");

    assert!(
        !output.status.success(),
        "--fallback error must exit with failure code on unreadable or low confidence images"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
