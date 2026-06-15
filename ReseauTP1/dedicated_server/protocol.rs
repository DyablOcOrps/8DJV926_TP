use bevy::prelude::Vec2;

// Définition des constantes de tags pour le protocole inter-shards
pub const TAG_HANDOFF_REQUEST: u8 = 0x20;
pub const TAG_HANDOFF_ACCEPT: u8 = 0x21;
pub const TAG_HANDOFF_REJECT: u8 = 0x22;
pub const TAG_GHOST_UPDATE: u8 = 0x23;
pub const TAG_HANDOFF_COMPLETE: u8 = 0x24;

#[derive(Debug)]
pub enum ShardMessage {
    HandoffRequest {
        entity_id: u32,
        pos: Vec2,
        vel: Vec2,
        state: [u8; 64],
    },
    HandoffAccept { entity_id: u32 },
    HandoffReject { entity_id: u32 },
    GhostUpdate {
        entity_id: u32,
        pos: Vec2,
        vel: Vec2,
    },
    HandoffComplete { entity_id: u32 },
}

impl ShardMessage {
    // Sérialise le message en vecteur d'octets binaire (Little Endian)
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            ShardMessage::HandoffRequest { entity_id, pos, vel, state } => {
                bytes.push(TAG_HANDOFF_REQUEST);
                bytes.extend_from_slice(&entity_id.to_le_bytes());
                bytes.extend_from_slice(&pos.x.to_le_bytes());
                bytes.extend_from_slice(&pos.y.to_le_bytes());
                bytes.extend_from_slice(&vel.x.to_le_bytes());
                bytes.extend_from_slice(&vel.y.to_le_bytes());
                bytes.extend_from_slice(state);
            }
            ShardMessage::HandoffAccept { entity_id } => {
                bytes.push(TAG_HANDOFF_ACCEPT);
                bytes.extend_from_slice(&entity_id.to_le_bytes());
            }
            ShardMessage::HandoffReject { entity_id } => {
                bytes.push(TAG_HANDOFF_REJECT);
                bytes.extend_from_slice(&entity_id.to_le_bytes());
            }
            ShardMessage::GhostUpdate { entity_id, pos, vel } => {
                bytes.push(TAG_GHOST_UPDATE);
                bytes.extend_from_slice(&entity_id.to_le_bytes());
                bytes.extend_from_slice(&pos.x.to_le_bytes());
                bytes.extend_from_slice(&pos.y.to_le_bytes());
                bytes.extend_from_slice(&vel.x.to_le_bytes());
                bytes.extend_from_slice(&vel.y.to_le_bytes());
            }
            ShardMessage::HandoffComplete { entity_id } => {
                bytes.push(TAG_HANDOFF_COMPLETE);
                bytes.extend_from_slice(&entity_id.to_le_bytes());
            }
        }
        bytes
    }

    // Désérialise un tableau d'octets reçu du réseau en ShardMessage structuré
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() { return None; }
        let tag = bytes[0];
        
        match tag {
            TAG_HANDOFF_REQUEST if bytes.len() >= 85 => {
                let entity_id = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                let px = f32::from_le_bytes(bytes[5..9].try_into().ok()?);
                let py = f32::from_le_bytes(bytes[9..13].try_into().ok()?);
                let vx = f32::from_le_bytes(bytes[13..17].try_into().ok()?);
                let vy = f32::from_le_bytes(bytes[17..21].try_into().ok()?);
                let mut state = [0u8; 64];
                state.copy_from_slice(&bytes[21..85]);
                
                Some(ShardMessage::HandoffRequest {
                    entity_id,
                    pos: Vec2::new(px, py),
                    vel: Vec2::new(vx, vy),
                    state,
                })
            }
            TAG_HANDOFF_ACCEPT if bytes.len() >= 5 => {
                let entity_id = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                Some(ShardMessage::HandoffAccept { entity_id })
            }
            TAG_HANDOFF_REJECT if bytes.len() >= 5 => {
                let entity_id = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                Some(ShardMessage::HandoffReject { entity_id })
            }
            TAG_GHOST_UPDATE if bytes.len() >= 21 => {
                let entity_id = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                let px = f32::from_le_bytes(bytes[5..9].try_into().ok()?);
                let py = f32::from_le_bytes(bytes[9..13].try_into().ok()?);
                let vx = f32::from_le_bytes(bytes[13..17].try_into().ok()?);
                let vy = f32::from_le_bytes(bytes[17..21].try_into().ok()?);
                
                Some(ShardMessage::GhostUpdate {
                    entity_id,
                    pos: Vec2::new(px, py),
                    vel: Vec2::new(vx, vy),
                })
            }
            TAG_HANDOFF_COMPLETE if bytes.len() >= 5 => {
                let entity_id = u32::from_le_bytes(bytes[1..5].try_into().ok()?);
                Some(ShardMessage::HandoffComplete { entity_id })
            }
            _ => None
        }
    }
}