use super::*;
use std::{cell::RefCell, collections::BTreeMap, io::Write};

struct Mock {
    responses: BTreeMap<String, Result<Vec<u8>, String>>,
    requests: RefCell<Vec<String>>,
}
impl Transport for Mock {
    fn fetch(&self, url: &str, limit: u64, output: &mut dyn Write) -> Result<(), String> {
        self.requests.borrow_mut().push(url.into());
        let bytes = self
            .responses
            .get(url)
            .ok_or("Unexpected network request")?
            .as_ref()
            .map_err(Clone::clone)?;
        if bytes.len() as u64 > limit {
            return Err("fixture exceeds download limit".into());
        }
        output.write_all(bytes).map_err(|e| e.to_string())
    }
}
fn target() -> Result<Target, String> {
    Target::for_triple("x86_64-unknown-linux-gnu")
}
fn metadata(version: &str) -> Result<Value, String> {
    let name = target()?.artifact();
    Ok(serde_json::json!({
        "tag_name": format!("v{version}"), "draft": false, "prerelease": false,
        "assets": [{ "name": name, "state": "uploaded", "size": 5,
            "browser_download_url": format!("https://github.com/csd113/Vitrallis-Shell/releases/download/v{version}/{name}"),
            "digest": format!("sha256:{}", hex_digest(&Sha256::digest(b"shell"))) }]
    }))
}
fn mock(releases: &[Value]) -> Result<Mock, serde_json::Error> {
    Ok(Mock {
        responses: BTreeMap::from([(
            format!("{}?per_page=100&page=1", release::API),
            Ok(serde_json::to_vec(releases)?),
        )]),
        requests: RefCell::default(),
    })
}
pub fn release() -> Result<Release, String> {
    release::select(
        &metadata("1.0.0")?,
        Version::new(1, 0, 0),
        target()?.artifact(),
    )
}

#[test]
fn versions_use_semantic_precedence_and_ignore_drafts_prereleases()
-> Result<(), Box<dyn std::error::Error>> {
    for (current, newest, available) in [
        ("1.0.0", "1.0.0", false),
        ("1.9.0", "1.10.0", true),
        ("2.0.0", "1.99.0", false),
        ("1.0.0-beta.10", "1.0.0", true),
        ("1.0.0+local", "1.0.0+release", false),
        ("0.9.0", "1.0.0", true),
    ] {
        let transport = mock(&[metadata(newest)?])?;
        let state = check(&transport, current, target)?;
        assert_eq!(
            matches!(state, State::Available(_)),
            available,
            "{current} / {newest}"
        );
        assert!(matches!(state, State::Available(_) | State::Current));
    }
    let mut draft = metadata("99.0.0")?;
    draft["draft"] = true.into();
    let mut pre = metadata("98.0.0")?;
    pre["prerelease"] = true.into();
    let transport = mock(&[
        metadata("1.9.0")?,
        draft,
        metadata("1.10.0")?,
        pre,
        metadata("3.0.0-beta.1")?,
    ])?;
    let State::Available(release) = check(&transport, "1.0.0", target)? else {
        return Err("missing update".into());
    };
    assert_eq!(release.version, Version::new(1, 10, 0));
    Ok(())
}

#[test]
fn beta_builds_receive_published_previews_without_downgrades()
-> Result<(), Box<dyn std::error::Error>> {
    for (current, newest, available) in [
        ("0.1.0-beta.1", "0.1.0-beta.2", true),
        ("0.1.0-beta.2", "0.1.0-beta2.1", true),
        ("0.1.0-beta2.1", "0.1.0-beta.2", false),
        ("0.1.0-beta.2", "0.1.0-beta.10", true),
        ("0.1.0-beta.10", "0.1.0-beta.2", false),
        ("0.1.0-beta.2", "0.1.0-beta.2", false),
        ("0.1.0-beta.2+old", "0.1.0-beta.2+new", false),
        ("0.1.0-beta.2", "0.1.0-rc.1", true),
        ("0.1.0-beta.2", "0.1.0", true),
    ] {
        for flagged in [false, true] {
            let mut value = metadata(newest)?;
            value["prerelease"] = flagged.into();
            let state = check(&mock(&[value])?, current, target)?;
            assert_eq!(
                matches!(state, State::Available(_)),
                available,
                "{current} / {newest}"
            );
            assert!(matches!(state, State::Available(_) | State::Current));
        }
    }
    let mut draft = metadata("99.0.0-beta.1")?;
    draft["draft"] = true.into();
    let mut beta = metadata("0.1.0-beta.10")?;
    beta["prerelease"] = true.into();
    let transport = mock(&[beta, draft.clone(), metadata("0.1.0-beta.2")?])?;
    let State::Available(release) = check(&transport, "0.1.0-beta.1", target)? else {
        return Err("missing beta update".into());
    };
    assert_eq!(release.version, Version::parse("0.1.0-beta.10")?);
    assert!(check(&mock(&[draft])?, "0.1.0-beta.1", target).is_err());
    assert!(check(&transport, "0.1.0", target).is_err());
    let mut invalid = metadata("0.1.0-beta.2")?;
    invalid["prerelease"] = "true".into();
    assert!(check(&mock(&[invalid])?, "0.1.0-beta.1", target).is_err());
    Ok(())
}

#[test]
fn pocketchip_beta_selects_the_standard_arm_artifact_and_verifies_download()
-> Result<(), Box<dyn std::error::Error>> {
    let arm = || Target::for_triple("armv7-unknown-linux-gnueabihf");
    let name = arm()?.artifact();
    assert_eq!(name, "vitrallis-armv7-unknown-linux-gnueabihf-glibc2.36");
    let mut value = metadata("0.1.0-beta.2")?;
    value["prerelease"] = true.into();
    assert!(check(&mock(&[value.clone()])?, "0.1.0-beta.1", arm).is_err());
    value["assets"][0]["name"] = name.clone().into();
    let url =
        format!("https://github.com/csd113/Vitrallis-Shell/releases/download/v0.1.0-beta.2/{name}");
    value["assets"][0]["browser_download_url"] = url.clone().into();
    let mut transport = mock(&[value])?;
    transport.responses.insert(url, Ok(b"shell".to_vec()));
    let State::Available(release) = check(&transport, "0.1.0-beta.1", arm)? else {
        return Err("missing PocketCHIP beta update".into());
    };
    assert_eq!(release.name, name);
    let scratch = crate::test_support::Scratch::new()?;
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(scratch.0.join("download"))?;
    download(&transport, &release, &mut file)?;
    assert_eq!(std::fs::read(scratch.0.join("download"))?, b"shell");
    Ok(())
}

#[test]
fn invalid_metadata_network_and_unsupported_builds_fail_closed()
-> Result<(), Box<dyn std::error::Error>> {
    for value in [
        serde_json::json!({}),
        metadata("not-semver")?,
        metadata("01.0.0")?,
    ] {
        assert!(check(&mock(&[value])?, "0.1.0", target).is_err());
    }
    assert!(check(&mock(&[])?, "0.1.0", target).is_err());
    let transport = mock(&[metadata("1.0.0")?])?;
    assert!(check(&transport, "invalid", target).is_err());
    assert!(check(&transport, "0.1.0", || Target::for_triple("unsupported")).is_err());
    assert!(
        check(&transport, "0.1.0", || Target::for_triple(
            "aarch64-unknown-linux-gnu"
        ))
        .is_err()
    );
    for response in [
        Err("network offline".into()),
        Ok(b"not JSON".to_vec()),
        Ok(b"{}".to_vec()),
    ] {
        let mut transport = mock(&[])?;
        transport
            .responses
            .insert(format!("{}?per_page=100&page=1", release::API), response);
        assert!(check(&transport, "0.1.0", target).is_err());
    }
    Ok(())
}

#[test]
fn artifacts_reject_foreign_urls_duplicates_missing_hashes_and_bad_sizes() -> Result<(), String> {
    for (field, value) in [
        (
            "browser_download_url",
            serde_json::json!("https://example.com/payload"),
        ),
        ("size", serde_json::json!(0)),
        ("size", serde_json::json!(release::MAX_BINARY + 1)),
        ("digest", Value::Null),
        ("digest", serde_json::json!("sha256:bad")),
        ("state", serde_json::json!("new")),
    ] {
        let mut value_metadata = metadata("1.0.0")?;
        value_metadata["assets"][0][field] = value;
        assert!(
            release::select(&value_metadata, Version::new(1, 0, 0), target()?.artifact()).is_err()
        );
    }
    let mut value = metadata("1.0.0")?;
    value["assets"] = serde_json::json!([value["assets"][0], value["assets"][0]]);
    assert!(release::select(&value, Version::new(1, 0, 0), target()?.artifact()).is_err());
    Ok(())
}

#[test]
fn downloads_require_complete_verified_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = crate::test_support::Scratch::new()?;
    let release = release()?;
    for (index, bytes) in [
        Ok(b"shell".to_vec()),
        Ok(b"short".to_vec()),
        Ok(b"sh".to_vec()),
        Err("connection interrupted".into()),
    ]
    .into_iter()
    .enumerate()
    {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(scratch.0.join(index.to_string()))?;
        let transport = Mock {
            responses: BTreeMap::from([(release.binary.url.clone(), bytes)]),
            requests: RefCell::default(),
        };
        assert_eq!(
            download(&transport, &release, &mut file).is_ok(),
            index == 0
        );
        assert_eq!(
            transport.requests.borrow().as_slice(),
            std::slice::from_ref(&release.binary.url)
        );
    }
    assert!(target()?.verify_header(b"shell").is_err());
    Ok(())
}

#[test]
fn checksum_sidecars_must_match_name_size_and_any_api_digest()
-> Result<(), Box<dyn std::error::Error>> {
    let mut release = release()?;
    let checksum = format!(
        "{}  {}\n",
        hex_digest(&Sha256::digest(b"shell")),
        release.name
    )
    .into_bytes();
    let asset = release::Asset {
        url: format!("{}.sha256", release.binary.url),
        size: checksum.len() as u64,
        digest: None,
    };
    release.checksum = Some(asset.clone());
    let scratch = crate::test_support::Scratch::new()?;
    for (index, bytes) in [
        checksum.clone(),
        b"bad".to_vec(),
        format!("{}  {}\n", "0".repeat(64), release.name).into_bytes(),
    ]
    .into_iter()
    .enumerate()
    {
        let transport = Mock {
            responses: BTreeMap::from([
                (asset.url.clone(), Ok(bytes)),
                (release.binary.url.clone(), Ok(b"shell".to_vec())),
            ]),
            requests: RefCell::default(),
        };
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(scratch.0.join(index.to_string()))?;
        assert_eq!(
            download(&transport, &release, &mut file).is_ok(),
            index == 0
        );
    }
    assert!(release::checksum(&checksum, "another-artifact").is_err());
    let mut bad_asset = asset;
    bad_asset.digest = Some("0".repeat(64));
    assert!(verify_bytes(&checksum, &bad_asset).is_err());
    Ok(())
}

#[test]
fn install_requires_separate_confirmation_and_never_enumerates_apps()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{
        input::Action,
        settings::{Page, Settings},
    };
    let transport = mock(&[metadata("1.0.0")?])?;
    let mut settings = Settings::default();
    settings.show();
    settings.page(Page::Updates);
    settings.updater.state = check(&transport, "0.1.0", target)?;
    assert_eq!(settings.input(Action::SelectAndActivate(1)), None);
    assert!(settings.update_confirmation.is_some());
    assert_eq!(settings.selected, 0);
    assert_eq!(settings.input(Action::Activate), None);
    assert!(settings.update_confirmation.is_none());
    settings.input(Action::SelectAndActivate(1));
    assert_eq!(
        settings.input(Action::SelectAndActivate(1)),
        Some(crate::settings::Request::InstallUpdate)
    );
    assert_eq!(
        transport.requests.borrow().as_slice(),
        [format!("{}?per_page=100&page=1", release::API)]
    );
    Ok(())
}

#[test]
fn disconnected_worker_and_network_failure_are_visible_and_retryable() {
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(sender);
    let mut updater = Updater {
        state: State::Checking,
        result: Some(receiver),
    };
    assert!(updater.poll());
    assert!(matches!(updater.state, State::Failed(_)));
    assert!(!updater.state.busy());
    assert!(!updater.poll());
}

#[test]
fn pagination_finds_newer_versions_and_never_accepts_partial_results()
-> Result<(), Box<dyn std::error::Error>> {
    let first_page = vec![metadata("1.0.0")?; 100];
    let mut transport = mock(&first_page)?;
    let second = format!("{}?per_page=100&page=2", release::API);
    transport.responses.insert(
        second.clone(),
        Ok(serde_json::to_vec(&[metadata("2.0.0")?])?),
    );
    let State::Available(release) = check(&transport, "1.0.0", target)? else {
        return Err("second-page update missing".into());
    };
    assert_eq!(release.version, Version::new(2, 0, 0));
    transport
        .responses
        .insert(second, Err("network interrupted on page two".into()));
    assert!(check(&transport, "1.0.0", target).is_err_and(|error| error.contains("page two")));
    Ok(())
}
