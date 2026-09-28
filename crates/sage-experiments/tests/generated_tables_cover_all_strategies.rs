//! End-to-end CSV-to-LaTeX behavior with synthetic rows, not paper evidence.
use std::fs;
use std::path::Path;
use std::process::Command;

fn write_csv(raw: &Path, name: &str, content: &str) {
    fs::write(raw.join(name), content).expect("write synthetic CSV");
}

fn table(out: &Path, name: &str) -> String {
    fs::read_to_string(out.join(name)).expect("read generated table")
}

#[test]
fn generated_tables_preserve_observed_arms_and_distinct_outcomes() {
    let temp = std::env::temp_dir().join(format!(
        "sage-tables-{}-{:?}-{:?}",
        std::process::id(),
        std::thread::current().id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("valid clock")
            .as_nanos()
    ));
    let raw = temp.join("raw");
    let out = temp.join("tables");
    fs::create_dir_all(&raw).expect("create fixture directory");

    write_csv(
        &raw,
        "rq1_migration_cost.csv",
        "strategy,seed,run_status,finalized_blocks,safety_violation,downtime_micros,finality_gap\n\
         Sage,0,completed,20,false,1000,0\n\
         New_Arm,0,completed,10,true,2000,2\n\
         New_Arm,1,failed,0,false,0,0\n",
    );
    write_csv(
        &raw,
        "rq2_partition_safety.csv",
        "strategy,partition_split,partition_duration_blocks,safety_violation,migration_success,disjoint_quorum_windows,run_status\n\
         Sage,3,6,false,true,0,ok\n\
         SageBlind,3,6,false,true,2,ok\n\
         SageBlind,3,6,true,false,0,failed\n",
    );
    write_csv(
        &raw,
        "rq3_kappa_ablation.csv",
        "kappa,safety_violation,migration_success,cutover_latency_micros,run_status\n\
         1,false,true,100,completed\n\
         8,true,false,200,completed\n\
         8,false,false,0,failed\n",
    );
    write_csv(
        &raw,
        "rq4_rollback.csv",
        "abort_height,rollback_occurred,rollback_latency_micros,absolute_reversion_rejected,safety_violation,run_status\n\
         12,true,80,false,false,ok\n\
         30,false,0,true,false,ok\n",
    );
    write_csv(
        &raw,
        "manifest_tests.csv",
        "test_name,passed,message\ncorrect_manifest_passes,true,accepted\nforged_manifest_rejected,true,rejected\n",
    );
    write_csv(
        &raw,
        "safety_partition.csv",
        "strategy,safety_violation\nSage,false\n",
    );

    let result = Command::new(env!("CARGO_BIN_EXE_generate_tables"))
        .args([
            "--raw-dir",
            raw.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("launch generator");
    assert!(
        result.status.success(),
        "generator failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let rq1 = table(&out, "rq1_table.tex");
    assert!(
        rq1.contains(r"New\_Arm & 10.0 & 2.000 & 2.0 & 1/1"),
        "{rq1}"
    );
    assert!(
        !rq1.contains("0/2"),
        "failed RQ1 trial entered denominator: {rq1}"
    );
    let rq2 = table(&out, "rq2_table.tex");
    assert!(
        rq2.contains("SageBlind (1 inconclusive) & 1 & 0/1 & 1/1"),
        "{rq2}"
    );
    assert!(
        rq2.contains("Sage & 1 & 0/1 & 0/1 & 0.000 [0.000, 0.793]"),
        "{rq2}"
    );
    let rq3 = table(&out, "rq3_table.tex");
    assert!(rq3.contains("8 & 1 & 1/1"), "{rq3}");
    let rq4 = table(&out, "rq4_table.tex");
    assert!(rq4.contains("12 & 1 & 1/1 & 0/1"), "{rq4}");
    assert!(rq4.contains("30 & 1 & 0/1 & 1/1"), "{rq4}");
    let manifest = table(&out, "manifest_table.tex");
    assert!(
        manifest.contains(r"correct\_manifest\_passes & accept & pass"),
        "{manifest}"
    );
    assert!(
        manifest.contains(r"forged\_manifest\_rejected & reject & pass"),
        "{manifest}"
    );
    fs::remove_dir_all(temp).expect("remove synthetic CSV fixtures");
}
