//! Parse Matrix links with the protocol library. Opening a link never joins a room implicitly.
use matrix_sdk::ruma::{MatrixToUri, MatrixUri, matrix_uri::MatrixId};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Room {
        address: String,
        event: Option<String>,
        via: Vec<String>,
    },
    User(String),
}
pub fn parse(value: &str) -> Result<Target, &'static str> {
    if value.len() > 4096 {
        return Err("link-invalid");
    }
    let (id, via) = if value.starts_with("matrix:") {
        let uri = MatrixUri::parse(value).map_err(|_| "link-invalid")?;
        (uri.id().clone(), uri.via().to_vec())
    } else {
        let url = url::Url::parse(value).map_err(|_| "link-invalid")?;
        if url.scheme() != "https"
            || url.host_str() != Some("matrix.to")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("link-invalid");
        }
        let uri = MatrixToUri::parse(value).map_err(|_| "link-invalid")?;
        (uri.id().clone(), uri.via().to_vec())
    };
    let via = via.into_iter().map(|s| s.to_string()).collect();
    Ok(match id {
        MatrixId::Room(id) => Target::Room {
            address: id.to_string(),
            event: None,
            via,
        },
        MatrixId::RoomAlias(id) => Target::Room {
            address: id.to_string(),
            event: None,
            via,
        },
        MatrixId::Event(room, event) => Target::Room {
            address: room.to_string(),
            event: Some(event.to_string()),
            via,
        },
        MatrixId::User(id) => Target::User(id.to_string()),
        _ => return Err("link-invalid"),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_links_preserve_room_event_and_reject_other_origins() {
        assert!(matches!(
            parse("https://matrix.to/#/!room:example.org/$event?via=example.org"),
            Ok(Target::Room { event: Some(_), .. })
        ));
        assert_eq!(
            parse("matrix:u/alice:example.org").unwrap(),
            Target::User("@alice:example.org".into())
        );
        for uri in [
            "javascript:alert(1)",
            "https://matrix.to.evil/#/@alice:example.org",
            "https://evil/#/!room:example.org",
            "https://alice@matrix.to/#/@alice:example.org",
        ] {
            assert!(parse(uri).is_err());
        }
    }
}
