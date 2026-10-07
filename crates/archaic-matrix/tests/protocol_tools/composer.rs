use super::*;
#[tokio::test]
async fn sticker_voice_and_poll_are_real_matrix_events() {
    let server = server().await;
    Mock::given(method("GET"))
        .and(path_regex(".*/media/config$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"m.upload.size":30000000})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path_regex(r"/_matrix/media/(v3|r0)/upload$"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"content_uri":"mxc://local/composer"})),
        )
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path_regex(r"/_matrix/client/(v3|r0)/rooms/.*/send/.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"event_id":"$composed"})))
        .mount(&server)
        .await;
    let (sender, mut receiver, worker) = login(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let sticker = dir.path().join("sticker.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 100, 20, 255]))
        .save(&sticker)
        .unwrap();
    let result = perform(
        &sender,
        &mut receiver,
        &request(ToolAction::Sticker { path: sticker }),
    )
    .await;
    assert!(
        matches!(result, Ok(ToolData::Uploaded)),
        "{result:?}; {:?}",
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| r.url.path().to_owned())
            .collect::<Vec<_>>()
    );
    let voice = dir.path().join("voice.wav");
    let mut wav = Vec::from(&b"RIFF"[..]);
    wav.extend(38u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(8000u32.to_le_bytes());
    wav.extend(16000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(2u32.to_le_bytes());
    wav.extend([0, 0]);
    std::fs::write(&voice, wav).unwrap();
    assert!(matches!(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::Voice {
                path: voice,
                duration_ms: 1
            })
        )
        .await,
        Ok(ToolData::Uploaded)
    ));
    assert!(matches!(
        perform(
            &sender,
            &mut receiver,
            &request(ToolAction::Poll {
                question: "Lunch".into(),
                answers: vec!["Pizza".into(), "Sushi".into()]
            })
        )
        .await,
        Ok(ToolData::Uploaded)
    ));
    let requests = server.received_requests().await.unwrap();
    let sends: Vec<_> = requests.iter().filter(|r| r.method == "PUT").collect();
    assert!(sends.iter().any(|r| r.url.path().contains("/m.sticker/")
        && r.body_json::<Value>().unwrap()["url"] == "mxc://local/composer"));
    assert!(sends.iter().any(|r| {
        let b = r.body_json::<Value>().unwrap();
        b["msgtype"] == "m.audio" && b.get("org.matrix.msc3245.voice").is_some()
    }));
    assert!(
        sends
            .iter()
            .any(|r| r.url.path().contains("/org.matrix.msc3381.poll.start/"))
    );
    worker.abort();
}
