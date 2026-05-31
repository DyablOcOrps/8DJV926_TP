use bevy::prelude::*;
use std::net::SocketAddr;
use std::collections::HashMap;
use crate::protocol::ShardMessage;
use crate::resources::ServerSocket; // On réutilise ton socket existant

// Component d'identification unique de l'entité réseau requis pour matcher les paquets
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkEntityId(pub u32);

#[derive(Component, Default)]
pub struct Velocity(pub Vec2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum AuthorityState {
    Owned,
    PendingHandoff { target_shard: u32 },
    Ghost { source_shard: u32 },
}

// Ressource Bevy pour connaître la correspondance ShardID -> Adresse Réseau (IP:Port)
#[derive(Resource, Default)]
pub struct ShardNetworkMap {
    pub shards: HashMap<u32, SocketAddr>,
}

// Événement déclenché lorsque on detecte une frontière
#[derive(Event)]
pub struct CrossingAlert {
    pub entity: Entity,
    pub target_shard_id: u32,
}

pub struct FlexibleAuthorityPlugin;

impl Plugin for FlexibleAuthorityPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CrossingAlert>()
           .insert_resource(ShardNetworkMap::default())
           .add_systems(Update, (
               handle_crossing_alerts,
               send_ghost_updates,
               receive_and_process_network,
           ));
    }
}

// 1. REÇOIT LE CROSSING ALERT DU SPATIAL SERVICE
fn handle_crossing_alerts(
    mut commands: Commands,
    mut events: EventReader<CrossingAlert>,
    mut query: Query<(&NetworkEntityId, &mut AuthorityState, &Transform, &Velocity)>,
    network_map: Res<ShardNetworkMap>,
    socket: Res<ServerSocket>,
) {
    for event in events.read() {
        if let Ok((net_id, mut state, transform, vel)) = query.get_mut(event.entity) {
            if *state == AuthorityState::Owned {
                // Étape 1 : On passe l'entité en transit
                *state = AuthorityState::PendingHandoff { target_shard: event.target_shard_id };
                
                // Étape 2 : On prépare le HandoffRequest
                let request = ShardMessage::HandoffRequest {
                    entity_id: net_id.0,
                    pos: transform.translation.truncate(),
                    vel: vel.0,
                    state: [0u8; 64], 
                };
                
                // Étape 3 : Envoi au shard cible
                if let Some(addr) = network_map.shards.get(&event.target_shard_id) {
                    let _ = socket.0.send_to(&request.serialize(), *addr);
                    println!("Shard: Demande de transfert émise pour l'entité {} vers Shard {}", net_id.0, event.target_shard_id);
                }
            }
        }
    }
}

// 2. ENVOIE EN CONTINU LES POSITIONS TANT QU'ON EST EN TRAIN DE PASSER LA FRONTIÈRE
fn send_ghost_updates(
    query: Query<(&NetworkEntityId, &Transform, &Velocity, &AuthorityState)>,
    network_map: Res<ShardNetworkMap>,
    socket: Res<ServerSocket>,
) {
    for (net_id, transform, vel, state) in query.iter() {
        if let AuthorityState::PendingHandoff { target_shard } = state {
            if let Some(addr) = network_map.shards.get(target_shard) {
                let update = ShardMessage::GhostUpdate {
                    entity_id: net_id.0,
                    pos: transform.translation.truncate(),
                    vel: vel.0,
                };
                let _ = socket.0.send_to(&update.serialize(), *addr);
            }
        }
    }
}

// 3. ÉCOUTE ET TRAITE LES PACKETS RESEAU DESTINÉS À L'AUTORITÉ FLEXIBLE
fn receive_and_process_network(
    mut commands: Commands,
    socket: Res<ServerSocket>,
    network_map: Res<ShardNetworkMap>,
    mut query: Query<(Entity, &NetworkEntityId, &mut AuthorityState, &mut Transform, &mut Velocity)>,
) {
    let mut buf = [0u8; 512];
    
    while let Ok((size, src_addr)) = socket.0.recv_from(&mut buf) {
        if let Some(msg) = ShardMessage::deserialize(&buf[..size]) {
            match msg {
                ShardMessage::HandoffRequest { entity_id, pos, vel, state: _ } => {
                    
                    // On accepte la demande
                    let response = ShardMessage::HandoffAccept { entity_id };
                    let _ = socket.0.send_to(&response.serialize(), src_addr);
                    
                    // On spawn l'entité en mode GHOST chez nous (Lecture seule)
                    commands.spawn((
                        NetworkEntityId(entity_id),
                        Transform::from_translation(pos.extend(0.0)),
                        Velocity(vel),
                        AuthorityState::Ghost { source_shard: 0 }, 
                    ));
                    println!("Shard: Reçu HandoffRequest. Spawn de l'entité {} en mode GHOST", entity_id);
                }
                
                ShardMessage::HandoffAccept { entity_id } => {
                    println!("Shard: Le voisin a accepté le transfert de l'entité {}. Envoi des positions entamé.", entity_id);
                }
                
                ShardMessage::HandoffReject { entity_id } => {
                    // Sécurité : le voisin refuse. On reprend le contrôle complet et on applique un rebond physique
                    if let Some((_, _, mut state, _, mut vel)) = query.iter_mut().find(|(_, id, _, _, _)| id.0 == entity_id) {
                        *state = AuthorityState::Owned;
                        vel.0 = -vel.0 * 1.5; 
                        println!("Shard: Transfert refusé pour {}. Application d'un rebond.", entity_id);
                    }
                }
                
                ShardMessage::GhostUpdate { entity_id, pos, vel } => {
                    // On met à jour l'entité Ghost locale passivement avec les données du maitre
                    if let Some((_, _, state, mut transform, mut local_vel)) = query.iter_mut().find(|(_, id, _, _, _)| id.0 == entity_id) {
                        if let AuthorityState::Ghost { .. } = *state {
                            transform.translation = pos.extend(0.0);
                            local_vel.0 = vel;
                        }
                    }
                }
                
                ShardMessage::HandoffComplete { entity_id } => {
                    // Étape finale : On devient propriétaire officiel !
                    if let Some((_, _, mut state, _, _)) = query.iter_mut().find(|(_, id, _, _, _)| id.0 == entity_id) {
                        *state = AuthorityState::Owned;
                        println!("Shard: Transfert complété. L'entité {} est maintenant sous notre AUTORITÉ complète.", entity_id);
                    }
                }
            }
        }
    }
}