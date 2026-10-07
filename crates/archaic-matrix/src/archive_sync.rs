//! One low-priority history request at a time, independent of the UI command worker.
use crate::{archive::Archive, connection::SyncUpdate};
use matrix_sdk::{Client, Room, room::MessagesOptions, ruma::uint};
use std::{collections::HashMap, time::Duration};
use tokio::{sync::mpsc, task::JoinHandle};

pub(crate) async fn fetch(archive: &Archive, room: &Room, job: &str) -> Result<bool, &'static str> {
    let id = room.room_id().as_str();
    let from = archive.position(id, job)?;
    if from.started && from.cursor.is_none() {
        return Ok(false);
    }
    let mut options = MessagesOptions::backward();
    options.limit = uint!(50);
    options.from = from.cursor.clone();
    let page = room
        .messages(options)
        .await
        .map_err(crate::message_actions::failure)?;
    if page.chunk.len() > 50 {
        return Err("history-invalid-page");
    }
    let values = page
        .chunk
        .iter()
        .map(|e| crate::details::value(room, e))
        .collect::<Result<Vec<_>, _>>()?;
    let redacts = room
        .clone_info()
        .room_version_rules_or_default()
        .redaction
        .keep_room_redaction_redacts;
    let archive = archive.clone();
    let id = id.to_owned();
    let job = job.to_owned();
    tokio::task::spawn_blocking(move || {
        archive.commit_page(&id, &job, &from, page.end, values, redacts)
    })
    .await
    .map_err(|_| "storage-failed")?
}

pub(crate) fn start(
    client: Client,
    archive: Archive,
    epoch: u64,
    updates: mpsc::Sender<(u64, SyncUpdate)>,
    initial_sync: tokio::sync::oneshot::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if initial_sync.await.is_err() {
            return;
        }
        let mut last_report = None;
        let mut retry_at = HashMap::new();
        let mut decrypt_after = HashMap::<String, Vec<u8>>::new();
        loop {
            if !report(&client, &archive, epoch, &updates, &retry_at).await {
                return;
            }
            for room in client.joined_rooms().into_iter().filter(|r| !r.is_space()) {
                if last_report
                    .is_none_or(|at: tokio::time::Instant| at.elapsed() >= Duration::from_secs(5))
                {
                    if !report(&client, &archive, epoch, &updates, &retry_at).await {
                        return;
                    }
                    last_report = Some(tokio::time::Instant::now());
                }
                let id = room.room_id().to_string();
                if retry_at
                    .get(&id)
                    .is_some_and(|at| tokio::time::Instant::now() < *at)
                {
                    continue;
                }
                let result = step(
                    &archive,
                    &room,
                    decrypt_after.entry(id.clone()).or_default(),
                )
                .await;
                match result {
                    Ok(true) => {
                        retry_at.remove(&id);
                        let _ = updates.send((epoch, SyncUpdate::Archived(id))).await;
                    }
                    Ok(false) => {
                        retry_at.remove(&id);
                    }
                    Err(_) => {
                        retry_at.insert(id, tokio::time::Instant::now() + Duration::from_secs(30));
                    }
                }
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
}
async fn report(
    client: &Client,
    archive: &Archive,
    epoch: u64,
    updates: &mpsc::Sender<(u64, SyncUpdate)>,
    retry_at: &HashMap<String, tokio::time::Instant>,
) -> bool {
    let rooms: Vec<_> = client
        .joined_rooms()
        .into_iter()
        .filter(|r| !r.is_space())
        .collect();
    let complete = rooms
        .iter()
        .filter(|r| {
            archive.jobs(r.room_id().as_str()).is_ok_and(|jobs| {
                jobs.iter().all(|j| {
                    archive
                        .position(r.room_id().as_str(), j)
                        .is_ok_and(|p| p.started && p.cursor.is_none())
                })
            })
        })
        .count();
    if let Ok((events, encrypted)) = archive.counts() {
        let status = crate::HistorySyncStatus {
            rooms: rooms.len(),
            complete,
            events,
            encrypted,
            retrying: rooms
                .iter()
                .filter(|r| retry_at.contains_key(r.room_id().as_str()))
                .count(),
        };
        if updates
            .send((epoch, SyncUpdate::HistorySync(status)))
            .await
            .is_err()
        {
            return false;
        }
    }
    true
}
async fn step(archive: &Archive, room: &Room, after: &mut Vec<u8>) -> Result<bool, &'static str> {
    let mut changed = false;
    // Fair round-robin: one page per room per pass. Completed rooms only retry a small
    // encrypted batch, so room-key recovery also reaches history outside the viewport.
    for job in archive.jobs(room.room_id().as_str())? {
        if fetch(archive, room, &job).await? {
            changed = true;
            break;
        }
    }
    let pending = archive.pending(room.room_id().as_str(), after)?;
    if pending.is_empty() {
        after.clear();
        return Ok(changed);
    }
    let mut decrypted = Vec::new();
    for (id, value) in pending {
        *after = id;
        let raw = matrix_sdk::ruma::serde::Raw::new(&value).map_err(|_| "history-invalid-page")?;
        if let Ok(event) = room.decrypt_event(&raw.cast_unchecked(), None).await
            && let Ok(value) = crate::details::value(room, &event)
            && value["type"] != "m.room.encrypted"
        {
            decrypted.push(value);
        }
    }
    if !decrypted.is_empty() {
        archive.merge(
            room.room_id().as_str(),
            decrypted,
            room.clone_info()
                .room_version_rules_or_default()
                .redaction
                .keep_room_redaction_redacts,
        )?;
        changed = true;
    }
    Ok(changed)
}
