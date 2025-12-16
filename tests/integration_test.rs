use std::fs;
use std::process::Command;

#[test]
fn test_basic_script_execution() {
    // Create a simple test script
    let script = r#"
wait 100ms
type "echo test"
wait 100ms
"#;

    let script_path = "/tmp/test_basic.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Check that it ran successfully
    assert!(
        output.status.success(),
        "scriptty failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that output contains our command
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("echo test"),
        "Output should contain typed command"
    );

    // Clean up
    let _ = fs::remove_file(script_path);
}

#[test]
fn test_expect_command() {
    // Create a script with expect
    let script = r#"
expect "$"
type "echo 'Hello World'"
expect "Hello World"
wait 200ms
type "exit"
"#;

    let script_path = "/tmp/test_expect.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Check that it ran successfully
    assert!(
        output.status.success(),
        "scriptty failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Hello World"),
        "Output should contain expected text"
    );

    // Clean up
    let _ = fs::remove_file(script_path);
}

#[test]
fn test_send_command() {
    // Test the send command (instant, no typing simulation)
    let script = r#"
expect "$"
send "echo instant"
wait 200ms
type "exit"
"#;

    let script_path = "/tmp/test_send.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Check that it ran successfully
    assert!(
        output.status.success(),
        "scriptty failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clean up
    let _ = fs::remove_file(script_path);
}

#[test]
fn test_invalid_script() {
    // Test with invalid command
    let script = r#"
invalid_command "test"
"#;

    let script_path = "/tmp/test_invalid.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty - should fail
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Should not succeed
    assert!(
        !output.status.success(),
        "scriptty should fail with invalid command"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command"),
        "Error should mention unknown command"
    );

    // Clean up
    let _ = fs::remove_file(script_path);
}

#[test]
fn test_expect_timeout() {
    // Test expect timeout behavior
    let script = r#"
expect "$"
type "echo test"
expect "this_will_never_appear" 500ms
"#;

    let script_path = "/tmp/test_timeout.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty - should fail with timeout
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Should not succeed
    assert!(
        !output.status.success(),
        "scriptty should fail with timeout"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Timeout"), "Error should mention timeout");

    // Clean up
    let _ = fs::remove_file(script_path);
}

#[test]
fn test_comment_handling() {
    // Test that comments are properly ignored
    let script = r#"
# This is a comment
expect "$"  # Wait for prompt

# Another comment
type "echo test"

wait 200ms
type "exit"
"#;

    let script_path = "/tmp/test_comments.script";
    fs::write(script_path, script).expect("Failed to write test script");

    // Run scriptty
    let output = Command::new("./target/release/scriptty")
        .arg("--script")
        .arg(script_path)
        .arg("--command")
        .arg("sh")
        .output()
        .expect("Failed to execute scriptty");

    // Check that it ran successfully
    assert!(
        output.status.success(),
        "scriptty failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Clean up
    let _ = fs::remove_file(script_path);
}
