use std::fs;

use harness::application::{CoreApplication, CoreDistributionPort, PortError};
use harness::domain::{ContentHash, CoreDistribution, DistributionFile, RelativePath};
use harness::infrastructure::{FileSystemInstallationState, GitThreeWayMerge};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct DistributionFixture(CoreDistribution);

impl CoreDistributionPort for DistributionFixture {
    fn current(&self) -> Result<CoreDistribution, PortError> {
        Ok(self.0.clone())
    }
}

#[test]
fn update_merges_non_overlapping_changes_and_stops_on_policy_overlap() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("docs/WORKFLOW.md");
    let version_one = application("1.0.0", b"one\ntwo\nthree\n");
    version_one.install(root.path(), false).unwrap();

    fs::write(&path, b"ONE\ntwo\nthree\n").unwrap();
    let version_two = application("2.0.0", b"one\ntwo\nTHREE\n");
    let preview = version_two.update(root.path(), true).unwrap();
    assert!(!preview.applied);
    assert!(preview.conflicts.is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"ONE\ntwo\nthree\n");

    let applied = version_two.update(root.path(), false).unwrap();
    assert!(applied.applied);
    assert!(applied.conflicts.is_empty());
    assert_eq!(fs::read(&path).unwrap(), b"ONE\ntwo\nTHREE\n");
    assert!(root
        .path()
        .join(applied.backup_path.unwrap())
        .join("files/docs/WORKFLOW.md")
        .is_file());

    let manifest_before = fs::read(root.path().join(".harness-core/manifest.json")).unwrap();
    let base_before = fs::read(root.path().join(".harness-core/base/docs/WORKFLOW.md")).unwrap();
    let local_before = fs::read(&path).unwrap();
    let version_three = application("3.0.0", b"UPSTREAM\ntwo\nTHREE\n");
    let conflict = version_three.update(root.path(), false).unwrap();
    assert!(!conflict.applied);
    assert_eq!(conflict.conflicts.len(), 1);
    assert_eq!(fs::read(&path).unwrap(), local_before);
    assert_eq!(
        fs::read(root.path().join(".harness-core/manifest.json")).unwrap(),
        manifest_before
    );
    assert_eq!(
        fs::read(root.path().join(".harness-core/base/docs/WORKFLOW.md")).unwrap(),
        base_before
    );
}

#[test]
fn update_handles_one_sided_add_remove_and_missing_file_rules_atomically() {
    let root = tempfile::tempdir().unwrap();
    let version_one = application_with_files(
        "1.0.0",
        &[
            ("docs/local.md", b"base local\n"),
            ("docs/upstream.md", b"base upstream\n"),
            ("docs/removed.md", b"remove me\n"),
        ],
    );
    version_one.install(root.path(), false).unwrap();
    fs::write(root.path().join("docs/local.md"), b"consumer local\n").unwrap();

    let version_two = application_with_files(
        "2.0.0",
        &[
            ("docs/local.md", b"base local\n"),
            ("docs/upstream.md", b"upstream changed\n"),
            ("docs/added.md", b"new upstream\n"),
        ],
    );
    let report = version_two.update(root.path(), false).unwrap();
    assert!(report.applied);
    assert_eq!(
        fs::read(root.path().join("docs/local.md")).unwrap(),
        b"consumer local\n"
    );
    assert_eq!(
        fs::read(root.path().join("docs/upstream.md")).unwrap(),
        b"upstream changed\n"
    );
    assert_eq!(
        fs::read(root.path().join("docs/added.md")).unwrap(),
        b"new upstream\n"
    );
    assert!(!root.path().join("docs/removed.md").exists());

    fs::remove_file(root.path().join("docs/upstream.md")).unwrap();
    let before = fs::read(root.path().join("docs/local.md")).unwrap();
    let version_three = application_with_files(
        "3.0.0",
        &[
            ("docs/local.md", b"would change\n"),
            ("docs/upstream.md", b"upstream changed again\n"),
            ("docs/added.md", b"new upstream\n"),
        ],
    );
    let conflict = version_three.update(root.path(), false).unwrap();
    assert!(!conflict.applied);
    assert!(conflict
        .conflicts
        .iter()
        .any(|value| value.path.as_str() == "docs/upstream.md"));
    assert_eq!(fs::read(root.path().join("docs/local.md")).unwrap(), before);
}

fn application(
    version: &str,
    content: &[u8],
) -> CoreApplication<DistributionFixture, FileSystemInstallationState, GitThreeWayMerge> {
    application_with_files(version, &[("docs/WORKFLOW.md", content)])
}

fn application_with_files(
    version: &str,
    files: &[(&str, &[u8])],
) -> CoreApplication<DistributionFixture, FileSystemInstallationState, GitThreeWayMerge> {
    CoreApplication::new(
        DistributionFixture(CoreDistribution {
            version: version.to_owned(),
            files: files
                .iter()
                .map(|(path, content)| DistributionFile {
                    path: RelativePath::parse(*path).unwrap(),
                    content: content.to_vec(),
                    hash: ContentHash::parse(format!("{:x}", Sha256::digest(content))).unwrap(),
                })
                .collect(),
        }),
        FileSystemInstallationState,
        GitThreeWayMerge,
    )
}
