use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ServerStatus<'a> {
    version: ServerVersion<'a>,
    players: ServerPlayers<'a>,
    description: ServerDescription<'a>,
    favicon: Option<&'a str>,
    #[serde(rename = "enforcesSecureChat")]
    enforces_secure_chat: bool,
    #[serde(rename = "previewsChat")]
    previews_chat: bool,
}

#[derive(Serialize)]
struct ServerVersion<'a> {
    name: &'a str,
    protocol: usize,
}

#[derive(Serialize)]
struct ServerPlayers<'a> {
    max: usize,
    online: usize,
    sample: Vec<ServerPlayersSample<'a>>,
}

#[derive(Serialize)]
struct ServerPlayersSample<'a> {
    name: &'a str,
    #[serde(with = "uuid::serde::compact")]
    id: Uuid,
}

#[derive(Serialize)]
struct ServerDescription<'a> {
    text: &'a str,
}

impl ServerStatus<'_> {
    pub fn get_example() -> Self {
        ServerStatus {
            version: ServerVersion {
                name: "1.20.1",
                protocol: 763,
            },
            players: ServerPlayers {
                max: 100,
                online: 105,
                sample: vec![ServerPlayersSample {
                    name: "thinkofdeath",
                    id: Uuid::parse_str("4566e69f-c907-48ee-8d71-d7ba5aa00d20").unwrap(),
                }],
            },
            description: ServerDescription {
                text: "Hello, world.",
            },
            favicon: None,
            enforces_secure_chat: true,
            previews_chat: true,
        }
    }
}
