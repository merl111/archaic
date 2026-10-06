use super::*;
use std::time::Duration;
#[tokio::test]
async fn queued_image_has_a_local_preview_before_upload_completes() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/media/config$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"m.upload.size":30000000})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(".*/upload$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(5))
                .set_body_json(json!({"content_uri":"mxc://local/pixel"})),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pixel.png");
    image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]))
        .save(&path)
        .unwrap();
    let (sender, mut receiver, worker) = login(&server).await;
    let upload = request(ToolAction::QueueUpload { path });
    sender
        .send(Command::RoomTool(upload.clone()))
        .await
        .unwrap();
    let event = next(&mut receiver, |e| {
        matches!(e, EventKind::ImagePreview { .. })
    })
    .await;
    let EventKind::ImagePreview {
        room_id,
        event_id,
        result,
    } = event.kind
    else {
        unreachable!()
    };
    assert_eq!(room_id, upload.room_id);
    assert_eq!(event_id, format!("~{}", upload.id));
    let image = result.unwrap();
    assert_eq!((image.width, image.height), (2, 3));
    assert_eq!(image.pixels, [1, 2, 3, 255].repeat(6));
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().contains("/send/m.room.message/"))
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn attachment_retry_keeps_bytes_name_and_message_transaction_and_discard_releases_snapshot() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/media/config$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"m.upload.size":30000000})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(".*/upload$"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"content_uri":"mxc://local/file"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/send/m.room.message/.*"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("report.txt");
    std::fs::write(&path, b"original bytes").unwrap();
    let (sender, mut receiver, worker) = login(&server).await;
    let upload = request(ToolAction::Upload { path: path.clone() });
    assert_error(
        perform(&sender, &mut receiver, &upload).await,
        "message-action-failed",
    );
    std::fs::write(&path, b"changed bytes").unwrap();
    let result = perform(&sender, &mut receiver, &upload).await;
    if let Err(key) = result {
        panic!(
            "upload failed: {key}; requests: {:?}",
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .map(|r| (&r.method, r.url.as_str()))
                .collect::<Vec<_>>()
        );
    }
    let requests = server.received_requests().await.unwrap();
    let uploads: Vec<_> = requests
        .iter()
        .filter(|r| {
            r.method == "POST"
                && r.url.path().ends_with("/upload")
                && !r.url.path().contains("/keys/")
        })
        .collect();
    assert_eq!(uploads.len(), 2);
    assert!(uploads.iter().all(|r| r.body == b"original bytes"));
    assert!(
        uploads
            .iter()
            .all(|r| !r.url.as_str().contains(dir.path().to_str().unwrap()))
    );
    let sends: Vec<_> = requests.iter().filter(|r| r.method == "PUT").collect();
    assert_eq!(sends.len(), 2);
    assert_eq!(sends[0].url.path(), sends[1].url.path());
    let body: Value = sends[1].body_json().unwrap();
    assert_eq!(body["msgtype"], "m.file");
    assert_eq!(body["body"], "report.txt");
    // A deliberate discard must permit a fresh read even when the same request is sent again.
    Mock::given(method("PUT"))
        .and(path_regex(".*/send/m.room.message/.*"))
        .respond_with(
            ResponseTemplate::new(503)
                .set_body_json(json!({"errcode":"M_UNKNOWN","error":"retry"})),
        )
        .with_priority(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let another = request(ToolAction::Upload { path: path.clone() });
    assert_error(
        perform(&sender, &mut receiver, &another).await,
        "message-action-failed",
    );
    sender.send(Command::DiscardUpload).await.unwrap();
    std::fs::write(&path, b"fresh after discard").unwrap();
    assert!(matches!(
        perform(&sender, &mut receiver, &another).await,
        Ok(ToolData::Uploaded)
    ));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .rfind(|r| r.method == "POST"
                && r.url.path().ends_with("/upload")
                && !r.url.path().contains("/keys/"))
            .unwrap()
            .body,
        b"fresh after discard"
    );
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn attachment_download_saves_exact_bytes_and_never_overwrites_existing_files() {
    let server = server().await;
    target(&server,event("$file","m.room.message",json!({"msgtype":"m.file","body":"../../untrusted.txt","url":"mxc://local/file","info":{"size":12}}),10)).await;
    Mock::given(method("GET"))
        .and(path_regex(".*/download/local/file$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test payload"))
        .expect(1)
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("chosen.txt");
    let download = request(ToolAction::Download {
        event_id: "$file".into(),
        path: path.clone(),
    });
    assert!(matches!(
        perform(&sender, &mut receiver, &download).await,
        Ok(ToolData::Downloaded)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"test payload");
    assert_error(
        perform(&sender, &mut receiver, &download).await,
        "file-exists",
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    target(&server,event("$large","m.room.message",json!({"msgtype":"m.file","body":"too large","url":"mxc://local/large","info":{"size":30000000}}),11)).await;
    assert_error(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::Download {
                event_id: "$large".into(),
                path: dir.path().join("large"),
            }),
        )
        .await,
        "file-size-limit",
    );
    assert!(!dir.path().join("large").exists());
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
#[tokio::test]
async fn receipts_are_explicit_private_or_public_and_use_selected_event() {
    let server = server().await;
    target(
        &server,
        event(
            "$read",
            "m.room.message",
            json!({"msgtype":"m.text","body":"Read me"}),
            10,
        ),
    )
    .await;
    Mock::given(method("POST"))
        .and(path_regex(".*/receipt/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(".*/account_data/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path().contains("/receipt/"))
    );
    for private in [true, false] {
        assert!(matches!(
            perform(
                &sender,
                &mut receiver,
                &request(ToolAction::MarkRead {
                    event_id: "$read".into(),
                    private
                })
            )
            .await,
            Ok(ToolData::Read)
        ));
    }
    let requests = server.received_requests().await.unwrap();
    let receipts: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().contains("/receipt/"))
        .collect();
    assert_eq!(receipts.len(), 2);
    assert!(receipts[0].url.path().contains("/m.read.private/"));
    assert!(receipts[1].url.path().contains("/m.read/"));
    assert!(receipts.iter().all(|r| r.url.path().ends_with("read")));
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn encrypted_attachment_download_verifies_hash_before_saving_plaintext() {
    let server = server().await;
    // AES-256-CTR known fixture: zero key/IV, SHA-256 over ciphertext. Test data only.
    let ciphertext = vec![
        153, 251, 163, 10, 219, 48, 253, 236, 201, 104, 195, 96, 230, 229, 67, 239, 62, 106, 228,
        143, 231, 49, 83, 202, 221,
    ];
    let file = json!({"url":"mxc://local/encrypted","v":"v2","key":{"kty":"oct","key_ops":["encrypt","decrypt"],"alg":"A256CTR","k":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ext":true},"iv":"AAAAAAAAAAAAAAAAAAAAAA","hashes":{"sha256":"9H1TYGelWHSaVXuC+Yr9O3db44RVmuvMf6C1mANpdik"}});
    target(
        &server,
        event(
            "$encrypted",
            "m.room.message",
            json!({"msgtype":"m.file","body":"encrypted.bin","file":file}),
            1,
        ),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/download/local/encrypted$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(ciphertext))
        .mount(&server)
        .await;
    let mut corrupt = file.clone();
    corrupt["url"] = json!("mxc://local/corrupt");
    target(
        &server,
        event(
            "$corrupt",
            "m.room.message",
            json!({"msgtype":"m.file","body":"corrupt.bin","file":corrupt}),
            2,
        ),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/download/local/corrupt$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tampered bytes"))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("good");
    let bad = dir.path().join("bad");
    assert!(matches!(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::Download {
                event_id: "$encrypted".into(),
                path: good.clone()
            })
        )
        .await,
        Ok(ToolData::Downloaded)
    ));
    assert_eq!(std::fs::read(good).unwrap(), b"Encrypted attachment test");
    assert_error(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::Download {
                event_id: "$corrupt".into(),
                path: bad.clone(),
            }),
        )
        .await,
        "message-action-failed",
    );
    assert!(!bad.exists());
    let requests = server.received_requests().await.unwrap();
    for r in requests
        .iter()
        .filter(|r| r.url.path().contains("/download/"))
    {
        assert_eq!(
            r.headers.get("authorization").unwrap(),
            "Bearer test-access-token"
        );
    }
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn preview_decodes_verified_image_and_cancel_does_not_publish_download() {
    let server = server().await;
    target(
        &server,
        event(
            "$image",
            "m.room.message",
            json!({"msgtype":"m.image","body":"pixel.png","url":"mxc://local/pixel"}),
            10,
        ),
    )
    .await;
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    Mock::given(method("GET"))
        .and(path_regex(".*/download/local/pixel$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(png.into_inner()))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let preview = request(ToolAction::Preview {
        event_id: "$image".into(),
    });
    let Ok(ToolData::Preview {
        pixels,
        width,
        height,
    }) = perform(&sender, &mut receiver, &preview).await
    else {
        panic!("decoded image expected")
    };
    assert_eq!((width, height), (2, 3));
    assert_eq!(&pixels[..4], &[1, 2, 3, 255]);
    target(
        &server,
        event(
            "$slow",
            "m.room.message",
            json!({"msgtype":"m.file","body":"slow.bin","url":"mxc://local/slow"}),
            11,
        ),
    )
    .await;
    Mock::given(method("GET"))
        .and(path_regex(".*/download/local/slow$"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_secs(30))
                .set_body_bytes(b"late content"),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("download");
    let download = request(ToolAction::Download {
        event_id: "$slow".into(),
        path: destination.clone(),
    });
    sender
        .send(Command::RoomTool(download.clone()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|r| r.url.path().ends_with("/download/local/slow"))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    sender
        .send(Command::CancelTransfer {
            id: download.id.clone(),
        })
        .await
        .unwrap();
    let result = next(
        &mut receiver,
        |e| matches!(e,EventKind::RoomToolResult{request,..} if request.id==download.id),
    )
    .await;
    assert!(matches!(
        result.kind,
        EventKind::RoomToolResult {
            result: Err("transfer-cancelled"),
            ..
        }
    ));
    assert!(!destination.exists());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}

#[tokio::test]
async fn thread_receipt_uses_the_actual_root_and_preserves_privacy() {
    let server = server().await;
    target(&server,event("$reply","m.room.message",json!({"msgtype":"m.text","body":"reply","m.relates_to":{"rel_type":"m.thread","event_id":"$root","is_falling_back":true,"m.in_reply_to":{"event_id":"$root"}}}),10)).await;
    Mock::given(method("POST"))
        .and(path_regex(".*/receipt/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    assert!(matches!(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::MarkThreadRead {
                event_id: "$reply".into(),
                private: true
            })
        )
        .await,
        Ok(ToolData::Read)
    ));
    let requests = server.received_requests().await.unwrap();
    let receipt = requests
        .iter()
        .find(|r| r.url.path().contains("/receipt/"))
        .unwrap();
    assert!(receipt.url.path().contains("/m.read.private/"));
    assert_eq!(receipt.body_json::<Value>().unwrap()["thread_id"], "$root");
    drop(sender);
    drop(receiver);
    worker.await.unwrap();
}
