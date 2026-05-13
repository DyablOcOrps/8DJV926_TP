use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Heartbeat {
    pub id: String,
    pub ip: String,
    pub port: u16,
    pub zone: String,
    pub player_count: usize,
    pub max_players: usize,
}

// Les messages réseau entre le client et toi
#[derive(Debug, Serialize, Deserialize)]
pub enum GameMessage {
    Join { username: String },
    Welcome { player_id: String },
}