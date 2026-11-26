//! Google's CDM loaded in the helper, as far as a licence request: set `NIGHTGLASS_WIDEVINE_CDM` to a
//! `libwidevinecdm` file to run it. Nothing is ever sent to a licence server.

use nightglass_drm::client::{Backend, Helper};
use nightglass_drm::protocol::Key;

fn widevine_pssh(kid: [u8; 16]) -> Vec<u8> {
    let mut data = vec![0x12, 16];
    data.extend_from_slice(&kid);
    data.extend_from_slice(&[0x22, 10]);
    data.extend_from_slice(b"nightglass");
    let mut pssh = Vec::new();
    pssh.extend_from_slice(&(32 + data.len() as u32).to_be_bytes());
    pssh.extend_from_slice(b"pssh");
    pssh.extend_from_slice(&[0; 4]);
    pssh.extend_from_slice(&[0xed, 0xef, 0x8b, 0xa9, 0x79, 0xd6, 0x4a, 0xce, 0xa3, 0xc8, 0x27, 0xdc, 0xd5, 0x1d, 0x21, 0xed]);
    pssh.extend_from_slice(&(data.len() as u32).to_be_bytes());
    pssh.extend_from_slice(&data);
    pssh
}

#[tokio::test]
async fn google_cdm_issues_a_licence_request() {
    let Some(cdm) = std::env::var_os("NIGHTGLASS_WIDEVINE_CDM") else {
        eprintln!("NIGHTGLASS_WIDEVINE_CDM unset: skipped");
        return;
    };
    let helper = Helper::start(&Backend::native(env!("CARGO_BIN_EXE_nightglass-cdm"), cdm)).await.unwrap();
    assert!(helper.cdm_version().starts_with('4'), "{}", helper.cdm_version());
    helper.initialize().await.unwrap();
    let kid = *b"nightglass-test!";
    let session = helper.create_session(widevine_pssh(kid)).await.unwrap();
    assert!(!session.is_empty());
    let (_, challenge) = helper.next_message(&session).await.unwrap();
    assert!(challenge.len() > 64, "a signed licence request, {} bytes", challenge.len());
    assert_eq!(challenge[0], 0x08, "SignedMessage.type comes first");
    assert!(helper.keys().iter().all(|k| !k.usable()));
    let refused = helper.probe(&kid, false).await.unwrap_err();
    assert!(matches!(refused, nightglass_drm::DrmError::NoKey { .. }), "{refused}");
    let garbage = helper.update_session(&session, vec![0x08, 0x02, 0x12, 0x00]).await;
    assert!(matches!(garbage, Err(nightglass_drm::DrmError::Rejected(_))), "{garbage:?}");
    helper.close_session(&session).await.unwrap();
    let _: Vec<Key> = helper.keys();
}
