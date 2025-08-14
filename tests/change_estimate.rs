use std::fs;
use std::process::Command;

#[test]
fn reports_median_change_from_change_estimates(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let criterion = dir.path().join("criterion");
    let bench_dir = criterion.join("group").join("bench");
    let before_dir = bench_dir.join("before");
    fs::create_dir_all(&before_dir)?;
    fs::write(
        before_dir.join("benchmark.json"),
        r#"{
    "group_id":"group",
    "function_id":null,
    "value_str":null,
    "throughput":null,
    "full_id":"bench",
    "directory_name":"bench"
}"#,
    )?;
    fs::write(
        before_dir.join("estimates.json"),
        r#"{
    "mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":1.0},"point_estimate":1.0,"standard_error":0.0},
    "median":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":1.0},"point_estimate":1.0,"standard_error":0.0},
    "median_abs_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":0.0},"point_estimate":0.0,"standard_error":0.0},
    "slope":null,
    "std_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":0.0},"point_estimate":0.0,"standard_error":0.0}
}"#,
    )?;
    let change_dir = bench_dir.join("change");
    fs::create_dir_all(&change_dir)?;
    fs::write(
        change_dir.join("estimates.json"),
        r#"{
    "mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":1.0},"point_estimate":1.0,"standard_error":0.0},
    "median":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":1.0},"point_estimate":1.23,"standard_error":0.0},
    "median_abs_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":0.0},"point_estimate":0.0,"standard_error":0.0},
    "slope":null,
    "std_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":0.0,"upper_bound":0.0},"point_estimate":0.0,"standard_error":0.0}
}"#,
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_critcmp"))
        .arg("--use-critcmp-change-estimate")
        .arg("--target-dir")
        .arg(dir.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("bench"));
    assert!(stdout.contains("1.23"));
    Ok(())
}
