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
    assert!(test_file.exists(), "original file must still exist untouched in dry-run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test_settings_input.png"), "{stdout}");
    assert!(stdout.contains("comparison_docker_vs_podman"), "{stdout}");

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
    assert!(stdout.contains("source_path,recommended_filename,confidence,nature,data_gaps"), "{stdout}");
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
    #[cfg(windows)]
    assert!(stdout.contains("Rename-Item"), "{stdout}");
    #[cfg(not(windows))]
    assert!(stdout.contains("mv --"), "{stdout}");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_batch_name_apply_renames_with_yes_flag() {
    let temp_dir = std::env::temp_dir().join("gyotaku_apply_test");
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

    // Original file should now be gone, and new file should exist!
    let expected_target = temp_dir.join("architecture_clean_architecture_in_rust.png");
    assert!(!original_file.exists(), "original file should have been moved");
    assert!(expected_target.exists(), "new renamed file must exist");

    let _ = fs::remove_dir_all(&temp_dir);
}
