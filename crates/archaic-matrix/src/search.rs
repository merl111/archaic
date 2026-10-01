use crate::{SearchMode, SearchPage, message_actions::failure, paging::Cursor, timeline};
use matrix_sdk::{
    Room,
    room::MessagesOptions,
    ruma::{api::client::search::search_events::v3, uint},
};
use serde_json::{Value, json};
#[derive(Clone, Default)]
pub(crate) struct Search {
    key: Option<(String, String, SearchMode)>,
    cursor: Cursor,
    page: SearchPage,
}
impl Search {
    pub async fn load(
        &mut self,
        room: &Room,
        query: &str,
        mode: SearchMode,
        more: bool,
    ) -> Result<SearchPage, &'static str> {
        let query = query.trim();
        if query.is_empty() || query.len() > 1024 {
            return Err("search-invalid");
        }
        let key = (room.room_id().to_string(), query.to_owned(), mode);
        if more && self.key.as_ref() != Some(&key) {
            return Err("search-restart");
        }
        let mut staged = if more {
            self.clone()
        } else {
            Self {
                key: Some(key),
                ..Self::default()
            }
        };
        if more && !staged.cursor.more() {
            return Ok(staged.page.clone());
        }
        let (values, next, scanned) = match mode {
            SearchMode::Indexed => return Err("search-invalid"),
            SearchMode::Server => server_page(room, query, &staged.cursor).await?,
            SearchMode::Local => local_page(room, &staged.cursor).await?,
        };
        staged.page.unavailable += values
            .iter()
            .filter(|v| v["type"] == "m.room.encrypted")
            .count();
        let rules = room.clone_info().room_version_rules_or_default().redaction;
        let entries = values
            .into_iter()
            .map(|v| timeline::decode(v, rules.keep_room_redaction_redacts))
            .collect::<Vec<_>>();
        let needle = query.to_lowercase();
        let hits = timeline::aggregate(&entries).into_iter().filter(|m| {
            !m.deleted
                && (mode == SearchMode::Server
                    || m.body
                        .as_ref()
                        .is_some_and(|body| body.to_lowercase().contains(&needle)))
        });
        for hit in hits {
            if !staged.page.messages.iter().any(|m| m.id == hit.id) {
                staged.page.messages.push(hit);
            }
        }
        if staged.page.messages.len() > 1000 {
            return Err("tool-limit");
        }
        staged.cursor.advance(next)?;
        staged.page.more = staged.cursor.more();
        staged.page.scanned += scanned;
        *self = staged;
        Ok(self.page.clone())
    }
}
async fn server_page(
    room: &Room,
    query: &str,
    cursor: &Cursor,
) -> Result<(Vec<Value>, Option<String>, usize), &'static str> {
    if room
        .latest_encryption_state()
        .await
        .map_err(failure)?
        .is_encrypted()
    {
        return Err("search-encrypted");
    }
    let categories = serde_json::from_value(json!({"room_events":{"search_term":query,"keys":["content.body"],"order_by":"recent","filter":{"rooms":[room.room_id()],"limit":50}}})).map_err(|_| "search-failed")?;
    let mut request = v3::Request::new(categories);
    request.next_batch = cursor.next.clone();
    let response = room
        .client()
        .send(request)
        .await
        .map_err(|e| failure(e.into()))?
        .search_categories
        .room_events;
    if response.results.len() > 50 {
        return Err("history-invalid-page");
    }
    let mut values = Vec::new();
    for result in response.results {
        let Some(raw) = result.result else {
            continue;
        };
        raw.deserialize().map_err(|_| "history-invalid-page")?;
        let value: Value =
            serde_json::from_str(raw.json().get()).map_err(|_| "history-invalid-page")?;
        if value["room_id"].as_str() != Some(room.room_id().as_str()) {
            return Err("history-invalid-page");
        }
        values.push(value);
    }
    let count = values.len();
    Ok((values, response.next_batch, count))
}
async fn local_page(
    room: &Room,
    cursor: &Cursor,
) -> Result<(Vec<Value>, Option<String>, usize), &'static str> {
    let mut options = MessagesOptions::backward();
    options.limit = uint!(50);
    options.from = cursor.next.clone();
    let response = room.messages(options).await.map_err(failure)?;
    if response.chunk.len() > 50 {
        return Err("history-invalid-page");
    }
    let values = response
        .chunk
        .iter()
        .map(|event| crate::details::value(room, event))
        .collect::<Result<Vec<_>, _>>()?;
    let count = values.len();
    Ok((values, response.end, count))
}
