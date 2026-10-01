//! Paginated, decrypted thread roots with scoped read-receipt indicators.
use crate::{
    message_actions::failure,
    organization::{Page, Row},
};
use matrix_sdk::{
    Client, Room,
    ruma::{EventId, uint},
};
pub(crate) async fn list(
    client: &Client,
    joined: &Room,
    participated: bool,
    next: Option<String>,
) -> Result<Page, &'static str> {
    use matrix_sdk::{
        room::ListThreadsOptions, ruma::api::client::threads::get_threads::v1::IncludeThreads,
    };
    let response = joined
        .list_threads(ListThreadsOptions {
            include_threads: if !participated {
                IncludeThreads::All
            } else {
                IncludeThreads::Participated
            },
            from: next.clone(),
            limit: Some(uint!(50)),
        })
        .await
        .map_err(failure)?;
    if response.chunk.len() > 50 || (next.is_some() && next == response.prev_batch_token) {
        return Err("history-invalid-page");
    }
    let ignored = client
        .account()
        .account_data::<matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent>()
        .await
        .map_err(|_| "organization-failed")?
        .map(|raw| raw.deserialize().map(|v| v.ignored_users))
        .transpose()
        .map_err(|_| "organization-failed")?
        .unwrap_or_default();
    let mut rows = Vec::new();
    for event in &response.chunk {
        let value = crate::details::value(joined, event)?;
        if value["sender"]
            .as_str()
            .is_some_and(|sender| ignored.keys().any(|u| u.as_str() == sender))
        {
            continue;
        }
        let id = value["event_id"].as_str().ok_or("history-invalid-page")?;
        let root = EventId::parse(id).map_err(|_| "history-invalid-page")?;
        let title = value["content"]["body"]
            .as_str()
            .unwrap_or(id)
            .chars()
            .take(180)
            .collect();
        use matrix_sdk::ruma::events::receipt::{ReceiptThread, ReceiptType};
        let scope = ReceiptThread::Thread(root);
        let public = joined
            .load_user_receipt(ReceiptType::Read, &scope, joined.own_user_id())
            .await
            .map_err(|_| "organization-failed")?;
        let private = joined
            .load_user_receipt(ReceiptType::ReadPrivate, &scope, joined.own_user_id())
            .await
            .map_err(|_| "organization-failed")?;
        let last =
            value["unsigned"]["m.relations"]["m.thread"]["latest_event"]["event_id"].as_str();
        let at_latest = last.is_some_and(|id| {
            public
                .as_ref()
                .is_some_and(|(event, _)| event.as_str() == id)
                || private
                    .as_ref()
                    .is_some_and(|(event, _)| event.as_str() == id)
        });
        rows.push(Row {
            id: id.into(),
            title,
            detail: format!(
                "{} · {}",
                value["unsigned"]["m.relations"]["m.thread"]["count"]
                    .as_u64()
                    .unwrap_or(0),
                if at_latest { "✓" } else { "●" }
            ),
        });
    }
    Ok(Page {
        rows,
        next: response.prev_batch_token,
        ..Default::default()
    })
}
