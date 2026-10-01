//! Explicitly offline data for inspecting density, typography and chat states.
use crate::model::{Model, Phase};
use archaic_matrix::{
    Attachment, Membership, Message, MessageDetails, Reaction, RoomInfo, RoomSummary,
};
pub fn model() -> Model {
    let mut rooms = [
        "Design critique",
        "Engineering",
        "Product notes",
        "General",
        "Release planning",
        "Community",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, name)| RoomSummary {
        id: format!("!room{i}:example.org"),
        name: name.into(),
        membership: Membership::Joined,
        inviter: None,
        info: RoomInfo {
            members: 14,
            members_known: true,
            encrypted: Some(true),
            topic: "Thoughtful conversations. Small details. Better software.".into(),
            space: false,
            ..Default::default()
        },
    })
    .collect::<Vec<_>>();
    rooms.extend(
        ["Archaic", "Matrix community"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| RoomSummary {
                id: format!("!space{i}:example.org"),
                name: name.into(),
                membership: Membership::Joined,
                inviter: None,
                info: RoomInfo {
                    space: true,
                    ..Default::default()
                },
            }),
    );
    let mut messages = [
        ("maya", "The latest desktop layouts are ready to review. I focused on making the conversation feel quieter."),
        ("noor", "The extra space around the message groups makes a difference. Much easier to scan now."),
        ("ana", "Three things for today’s critique:\n1. Composer reply and edit states\n2. Thread navigation\n3. Verification flow"),
        ("ben", "I added the annotated screens and the spacing notes."),
        ("alex", "Let’s keep the same rhythm in the dialogs, too. Every control should feel like it belongs here."),
        ("maya", "This is an offline UI preview. Normal launches connect to your Matrix homeserver.")
    ].into_iter().enumerate().map(|(i,(sender,body))| Message {
        id: format!("$preview{i}"), sender: format!("@{sender}:example.org"), body: Some(body.into()),
        timestamp: 1790842200000 + i as u64 * 60000, editable: true, ..Default::default()
    }).collect::<Vec<_>>();
    messages[1].reactions = vec![
        Reaction {
            id: "$reaction1".into(),
            sender: "@alex:example.org".into(),
            key: "🙌".into(),
        },
        Reaction {
            id: "$reaction2".into(),
            sender: "@ana:example.org".into(),
            key: "🙌".into(),
        },
    ];
    messages[3].attachment = Some(Attachment {
        name: "desktop-review.pdf".into(),
        kind: "m.file".into(),
        size: Some(2400000),
    });
    messages[4].reply_to = Some(messages[2].id.clone());
    let root = messages[2].clone();
    let thread = vec![
        Message {
            id: "$thread1".into(),
            sender: "@noor:example.org".into(),
            body: Some("The reply context should stay close to the composer.".into()),
            timestamp: 1790842600000,
            thread_root: Some(root.id.clone()),
            editable: true,
            ..Default::default()
        },
        Message {
            id: "$thread2".into(),
            sender: "@alex:example.org".into(),
            body: Some("Agreed. Escape should also bring back the ordinary draft.".into()),
            timestamp: 1790842660000,
            thread_root: Some(root.id.clone()),
            editable: true,
            ..Default::default()
        },
    ];
    let mut m = Model {
        phase: Phase::Connected,
        user: "@alex:example.org".into(),
        selected: Some(rooms[0].id.clone()),
        rooms,
        messages,
        ..Default::default()
    };
    m.tools.details = Some(MessageDetails {
        message: root,
        thread,
        unavailable: 0,
        reply: None,
        reply_unavailable: false,
        revisions: Vec::new(),
        read_by: Vec::new(),
        thread_read_by: Vec::new(),
        more: false,
        loaded: 2,
    });
    m.activity.unread.insert("!room1:example.org".into(), 8);
    m.activity.unread.insert("!room4:example.org".into(), 3);
    m
}
