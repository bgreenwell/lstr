use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

// Platform-specific import for unix permissions
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_nonexistent_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("nonexistent/path/for/testing");
    cmd.assert().failure().stderr(predicate::str::contains("is not a directory"));
    Ok(())
}

#[test]
fn test_simple_view() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("a.txt"))?;
    fs::create_dir(temp_dir.path().join("dir1"))?;
    fs::File::create(temp_dir.path().join("dir1/b.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg(temp_dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("a.txt"))
        .stdout(predicate::str::contains("dir1"))
        .stdout(predicate::str::contains("b.txt"));
    Ok(())
}

#[test]
fn test_all_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join(".hidden"))?;

    let mut cmd_no_all = Command::cargo_bin("lstr")?;
    cmd_no_all.arg(temp_dir.path());
    cmd_no_all.assert().success().stdout(predicate::str::contains(".hidden").not());

    let mut cmd_with_all = Command::cargo_bin("lstr")?;
    cmd_with_all.arg("-a").arg(temp_dir.path());
    cmd_with_all.assert().success().stdout(predicate::str::contains(".hidden"));
    Ok(())
}

#[test]
fn test_depth_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::create_dir(temp_dir.path().join("dir1"))?;
    fs::File::create(temp_dir.path().join("dir1/b.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-L").arg("1").arg(temp_dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("dir1"))
        .stdout(predicate::str::contains("b.txt").not());
    Ok(())
}

#[test]
fn test_gitignore_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    // 1. Initialize a true git repository
    Command::new("git").arg("init").current_dir(temp_path).output()?;
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_path)
        .output()?;

    // 2. Create and commit the .gitignore file
    let gitignore_path = temp_path.join(".gitignore");
    fs::write(&gitignore_path, "ignored.txt\nignored_dir/\n")?;
    Command::new("git").arg("add").arg(&gitignore_path).current_dir(temp_path).output()?;
    Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("add gitignore")
        .current_dir(temp_path)
        .output()?;

    // 3. Create other files to be checked
    fs::File::create(temp_path.join("ignored.txt"))?;
    fs::File::create(temp_path.join("good.txt"))?;
    fs::create_dir(temp_path.join("ignored_dir"))?;
    fs::File::create(temp_path.join("ignored_dir/a.txt"))?;

    // 4. Run lstr, passing the temp path as an argument. This is more robust
    // than relying on `current_dir` for this specific test.
    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-g").arg(temp_path);

    // 5. Assert that the correct files are included and excluded.
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("good.txt"))
        .stdout(predicate::str::contains("ignored.txt").not())
        .stdout(predicate::str::contains("ignored_dir").not());

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_permissions_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let file_path = temp_dir.path().join("test_file.txt");
    fs::File::create(&file_path)?;

    let perms = fs::Permissions::from_mode(0o550);
    fs::set_permissions(&file_path, perms)?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-p").arg(temp_dir.path());
    cmd.assert().success().stdout(predicate::str::contains("-r-xr-x---"));

    Ok(())
}

#[test]
fn test_git_status_flag() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    let temp_path = temp_dir.path();

    Command::new("git").arg("init").current_dir(temp_path).output()?;
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_path)
        .output()?;
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_path)
        .output()?;

    fs::write(temp_path.join("committed.txt"), "initial content")?;
    Command::new("git").args(["add", "committed.txt"]).current_dir(temp_path).output()?;
    Command::new("git").args(["commit", "-m", "initial commit"]).current_dir(temp_path).output()?;

    fs::write(temp_path.join("committed.txt"), "modified content")?;
    fs::write(temp_path.join("staged.txt"), "staged")?;
    Command::new("git").args(["add", "staged.txt"]).current_dir(temp_path).output()?;
    fs::write(temp_path.join("untracked.txt"), "untracked")?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-G").arg("-a").arg(temp_path);

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r"M\s+.*committed\.txt").unwrap())
        .stdout(predicate::str::is_match(r"A\s+.*staged\.txt").unwrap())
        .stdout(predicate::str::is_match(r"\?\s+.*untracked\.txt").unwrap());

    Ok(())
}

#[test]
fn test_sort_by_name() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("zebra.txt"))?;
    fs::File::create(temp_dir.path().join("apple.txt"))?;
    fs::File::create(temp_dir.path().join("banana.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--sort").arg("name").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Files should appear in alphabetical order
    let apple_pos = stdout.find("apple.txt").unwrap();
    let banana_pos = stdout.find("banana.txt").unwrap();
    let zebra_pos = stdout.find("zebra.txt").unwrap();

    assert!(apple_pos < banana_pos);
    assert!(banana_pos < zebra_pos);

    Ok(())
}

#[test]
fn test_dirs_first_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("aaa_file.txt"))?;
    fs::create_dir(temp_dir.path().join("zzz_dir"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--dirs-first").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Directory should appear before file, despite alphabetical order
    let dir_pos = stdout.find("zzz_dir").unwrap();
    let file_pos = stdout.find("aaa_file.txt").unwrap();

    assert!(dir_pos < file_pos);

    Ok(())
}

#[test]
fn test_natural_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("file1.txt"))?;
    fs::File::create(temp_dir.path().join("file10.txt"))?;
    fs::File::create(temp_dir.path().join("file2.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--natural-sort").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // With natural sorting: file1 < file2 < file10
    let file1_pos = stdout.find("file1.txt").unwrap();
    let file2_pos = stdout.find("file2.txt").unwrap();
    let file10_pos = stdout.find("file10.txt").unwrap();

    assert!(file1_pos < file2_pos);
    assert!(file2_pos < file10_pos);

    Ok(())
}

#[test]
fn test_reverse_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("apple.txt"))?;
    fs::File::create(temp_dir.path().join("zebra.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--reverse").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // With reverse sorting: zebra should come before apple
    let apple_pos = stdout.find("apple.txt").unwrap();
    let zebra_pos = stdout.find("zebra.txt").unwrap();

    assert!(zebra_pos < apple_pos);

    Ok(())
}

#[test]
fn test_case_sensitive_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("Apple.txt"))?;
    fs::File::create(temp_dir.path().join("banana.txt"))?;

    // Test case-sensitive (Apple should come before banana in ASCII)
    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--case-sensitive").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    let apple_pos = stdout.find("Apple.txt").unwrap();
    let banana_pos = stdout.find("banana.txt").unwrap();

    // In case-sensitive ASCII order: "Apple" < "banana" (uppercase < lowercase)
    assert!(apple_pos < banana_pos);

    Ok(())
}

#[test]
fn test_sort_by_extension() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("file.zzz"))?;
    fs::File::create(temp_dir.path().join("file.aaa"))?;
    fs::File::create(temp_dir.path().join("file.bbb"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--sort").arg("extension").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Files should be sorted by extension: .aaa < .bbb < .zzz
    let aaa_pos = stdout.find("file.aaa").unwrap();
    let bbb_pos = stdout.find("file.bbb").unwrap();
    let zzz_pos = stdout.find("file.zzz").unwrap();

    assert!(aaa_pos < bbb_pos);
    assert!(bbb_pos < zzz_pos);

    Ok(())
}

#[test]
fn test_default_sort_order() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Create files with explicit writes and different names to avoid conflicts
    let file1_path = temp_dir.path().join("0num.txt");
    let file_a_path = temp_dir.path().join("Upper.txt");
    let file_a_lower_path = temp_dir.path().join("lower.txt");

    fs::write(&file1_path, "1")?;
    fs::write(&file_a_path, "A")?;
    fs::write(&file_a_lower_path, "a")?;

    // Verify files exist
    assert!(file1_path.exists(), "0num.txt was not created");
    assert!(file_a_path.exists(), "Upper.txt was not created");
    assert!(file_a_lower_path.exists(), "lower.txt was not created");

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--case-sensitive").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Check if files are at least present
    assert!(stdout.contains("0num.txt"), "0num.txt missing from output");
    assert!(stdout.contains("Upper.txt"), "Upper.txt missing from output");
    assert!(stdout.contains("lower.txt"), "lower.txt missing from output");

    // With default order: numbers < uppercase < lowercase
    let file1_pos = stdout.find("0num.txt").expect("0num.txt not found in output");
    let file_a_pos = stdout.find("Upper.txt").expect("Upper.txt not found in output");
    let file_a_lower_pos = stdout.find("lower.txt").expect("lower.txt not found in output");

    assert!(file1_pos < file_a_pos);
    assert!(file_a_pos < file_a_lower_pos);

    Ok(())
}

#[test]
fn test_dotfiles_first_sorting() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Create files and folders with explicit writes/creates
    fs::write(temp_dir.path().join("regular.txt"), "regular")?;
    fs::write(temp_dir.path().join(".hidden.txt"), "hidden")?;
    fs::create_dir(temp_dir.path().join("folder"))?;
    fs::create_dir(temp_dir.path().join(".dotfolder"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--dotfiles-first").arg("-a").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Order should be: .dotfolder -> folder -> .hidden.txt -> regular.txt
    // With proper tree connectors: ├── for first 3 items, └── for last item
    let dotfolder_line_pos = stdout.find("├── .dotfolder").expect(".dotfolder line not found");
    let folder_line_pos = stdout.find("├── folder").expect("folder line not found");
    let hidden_line_pos = stdout.find("├── .hidden.txt").expect(".hidden.txt line not found");
    let regular_line_pos = stdout.find("└── regular.txt").expect("regular.txt line not found");

    assert!(dotfolder_line_pos < folder_line_pos);
    assert!(folder_line_pos < hidden_line_pos);
    assert!(hidden_line_pos < regular_line_pos);

    Ok(())
}

#[test]
fn test_tree_structure_display() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Create the exact structure from issue #36:
    // .
    // └── t1
    //     ├── t2
    //     │   ├── hello.md
    //     │   └── t3
    //     └── tmp.txt

    // Create t1 directory
    fs::create_dir(temp_dir.path().join("t1"))?;

    // Create t2 subdirectory
    fs::create_dir(temp_dir.path().join("t1/t2"))?;

    // Create files and subdirectories
    fs::write(temp_dir.path().join("t1/t2/hello.md"), "# Hello")?;
    fs::create_dir(temp_dir.path().join("t1/t2/t3"))?;
    fs::write(temp_dir.path().join("t1/tmp.txt"), "temporary content")?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Expected tree structure with proper connectors:
    // t1 should use └── (last/only item in root)
    // t2 should use ├── (not last in t1)
    // tmp.txt should use └── (last in t1)
    // hello.md should use ├── (not last in t2)
    // t3 should use └── (last in t2)

    // Check that we have proper tree connectors, not all └──
    assert!(stdout.contains("└── t1"), "t1 should use └── connector");
    assert!(stdout.contains("├── t2"), "t2 should use ├── connector (not last in parent)");
    assert!(stdout.contains("└── tmp.txt"), "tmp.txt should use └── connector (last in parent)");
    assert!(
        stdout.contains("├── hello.md"),
        "hello.md should use ├── connector (not last in parent)"
    );
    assert!(stdout.contains("└── t3"), "t3 should use └── connector (last in parent)");

    // Verify we have vertical connectors for proper tree visualization
    assert!(stdout.contains("│"), "Should contain vertical tree connectors");

    Ok(())
}

#[test]
fn test_tree_structure_with_dirs_first() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Create a structure to test --dirs-first with tree connectors
    fs::create_dir(temp_dir.path().join("dir1"))?;
    fs::create_dir(temp_dir.path().join("dir2"))?;
    fs::write(temp_dir.path().join("file1.txt"), "content1")?;
    fs::write(temp_dir.path().join("file2.txt"), "content2")?;

    // Add some nested content
    fs::write(temp_dir.path().join("dir1/nested.txt"), "nested content")?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("--dirs-first").arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // With --dirs-first, directories should come first
    // dir1 should use ├── (not last directory)
    // dir2 should use ├── (not last directory, files come after)
    // file1.txt should use ├── (not last file)
    // file2.txt should use └── (last file)

    assert!(stdout.contains("├── dir1"), "dir1 should use ├── connector");
    assert!(stdout.contains("├── dir2"), "dir2 should use ├── connector");
    assert!(stdout.contains("├── file1.txt"), "file1.txt should use ├── connector");
    assert!(stdout.contains("└── file2.txt"), "file2.txt should use └── connector (last)");

    Ok(())
}

#[test]
fn test_single_file_tree() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Single file should use └──
    fs::write(temp_dir.path().join("single.txt"), "content")?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(stdout.contains("└── single.txt"), "single file should use └── connector");

    Ok(())
}

#[test]
fn test_deep_nested_tree() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;

    // Create a deeper nesting structure: a/b/c/d/file.txt
    fs::create_dir(temp_dir.path().join("a"))?;
    fs::create_dir(temp_dir.path().join("a/b"))?;
    fs::create_dir(temp_dir.path().join("a/b/c"))?;
    fs::create_dir(temp_dir.path().join("a/b/c/d"))?;
    fs::write(temp_dir.path().join("a/b/c/d/deep.txt"), "deep content")?;

    // Add sibling to 'a' to test vertical connectors
    fs::create_dir(temp_dir.path().join("sibling"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg(temp_dir.path());

    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;

    // Should have proper vertical connectors for deep nesting
    assert!(stdout.contains("├── a"), "a should use ├── (has sibling)");
    assert!(stdout.contains("└── sibling"), "sibling should use └── (last)");
    assert!(stdout.contains("│"), "Should contain vertical connectors for deep nesting");

    Ok(())
}

#[test]
fn test_ignore_files_only_respected_with_gitignore_flag() -> Result<(), Box<dyn std::error::Error>>
{
    let temp_dir = tempdir()?;
    fs::write(temp_dir.path().join(".ignore"), "secret.txt\n")?;
    fs::File::create(temp_dir.path().join("secret.txt"))?;
    fs::File::create(temp_dir.path().join("visible.txt"))?;

    // Without -g, ignore files must not filter the output.
    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg(temp_dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("secret.txt"))
        .stdout(predicate::str::contains("visible.txt"));

    // With -g, .ignore files are respected like .gitignore.
    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-g").arg(temp_dir.path());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("visible.txt"))
        .stdout(predicate::str::contains("secret.txt").not());
    Ok(())
}

#[test]
fn test_dirs_only_connectors() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::create_dir(temp_dir.path().join("adir"))?;
    fs::File::create(temp_dir.path().join("adir/inner.txt"))?;
    fs::create_dir(temp_dir.path().join("bdir"))?;
    fs::File::create(temp_dir.path().join("zfile.txt"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-d").arg(temp_dir.path());
    // With files filtered out, the last *printed* entry must get the
    // last-sibling connector, and no file may leak into the output.
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("├── adir"))
        .stdout(predicate::str::contains("└── bdir"))
        .stdout(predicate::str::contains("zfile").not())
        .stdout(predicate::str::contains("inner").not());
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_permissions_show_symlink_type() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempdir()?;
    fs::File::create(temp_dir.path().join("target.txt"))?;
    std::os::unix::fs::symlink("target.txt", temp_dir.path().join("link_to_target"))?;

    let mut cmd = Command::cargo_bin("lstr")?;
    cmd.arg("-p").arg(temp_dir.path());
    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let link_line =
        stdout.lines().find(|l| l.contains("link_to_target")).expect("symlink should be listed");
    assert!(link_line.starts_with('l'), "symlink line should start with 'l': {link_line}");
    let file_line =
        stdout.lines().find(|l| l.contains("target.txt")).expect("file should be listed");
    assert!(file_line.starts_with('-'), "file line should start with '-': {file_line}");
    Ok(())
}
