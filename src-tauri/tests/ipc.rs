//! Exercises the IPC commands against Tauri's mock runtime, without a window
//! or a microphone: configure → label_strip → frame, checking the binary
//! responses have the layout the frontend expects.

use overtone_desktop_lib::{build_app, AppState};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewWindowBuilder};

fn request(cmd: &str, body: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://localhost:3000/".parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

#[test]
fn commands_return_binary_frames() {
    // The real context brings the capabilities (ACL) that allow the commands.
    let app = build_app(mock_builder())
        .build(tauri::generate_context!())
        .expect("build app");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("webview");
    assert!(app.try_state::<AppState>().is_some());

    let info = get_ipc_response(
        &webview,
        request(
            "configure",
            serde_json::json!({ "config": {
                "fftSize": 16384, "height": 1092, "scale": "piano",
                "coloring": "detailed", "labeling": "piano"
            }}),
        ),
    )
    .expect("configure");
    let info: serde_json::Value = match info {
        InvokeResponseBody::Json(s) => serde_json::from_str(&s).unwrap(),
        other => panic!("expected json, got {other:?}"),
    };
    assert_eq!(info["height"], 1092);
    assert_eq!(info["fftSize"], 16384);
    assert_eq!(info["labelWidth"], 64);
    assert!(info["markers"].as_array().unwrap().len() > 10);
    assert_eq!(info["range"]["minHz"].as_f64().unwrap().round(), 37.0);

    let strip = get_ipc_response(&webview, request("label_strip", serde_json::json!({}))).unwrap();
    match strip {
        InvokeResponseBody::Raw(bytes) => assert_eq!(bytes.len(), 64 * 1092 * 4),
        other => panic!("expected raw bytes, got {other:?}"),
    }

    for expected_seq in 1..=3u32 {
        let frame = get_ipc_response(
            &webview,
            request(
                "frame",
                serde_json::json!({ "view": { "width": 300, "pxPerSecond": 120.0, "viewEnd": null } }),
            ),
        )
        .unwrap();
        let bytes = match frame {
            InvokeResponseBody::Raw(bytes) => bytes,
            other => panic!("expected raw bytes, got {other:?}"),
        };
        let columns = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        // the first frame is a full redraw of 300 columns, later ones carry none
        assert_eq!(columns, if expected_seq == 1 { 300 } else { 0 });
        assert_eq!(bytes.len(), 68 + columns * 1092 * 4 + 1092 * 4);
        assert_eq!(&bytes[0..4], &0x4E54_564Fu32.to_le_bytes());
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            expected_seq
        );
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 1092);
    }

    let status =
        get_ipc_response(&webview, request("capture_status", serde_json::json!({}))).unwrap();
    match status {
        InvokeResponseBody::Json(s) => assert!(s.contains("\"running\":false")),
        other => panic!("expected json, got {other:?}"),
    }

    // A bad configuration is rejected with an error, not a panic.
    let err = get_ipc_response(
        &webview,
        request(
            "configure",
            serde_json::json!({ "config": {
                "fftSize": 1000, "height": 1092, "scale": "piano",
                "coloring": "detailed", "labeling": "piano"
            }}),
        ),
    );
    assert!(err.is_err());
}
