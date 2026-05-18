use bevy::prelude::*;
use std::net::{UdpSocket, SocketAddr};
use std::collections::HashMap;

// ==========================================
// 1. RESSOURCES BEVY (États globaux)
// ==========================================

// Configuration immuable du serveur définie au démarrage.
#[derive(Resource)]
pub struct ServerConfig {
    pub id: String,
    pub port: u16,
    pub zone: String,
    pub max_players: usize,
    pub orchestrator_addr: SocketAddr,
}

// Permet d'accéder au réseau depuis n'importe quel système Bevy.
#[derive(Resource)]
pub struct ServerSocket(pub UdpSocket);

// Registre des joueurs présents sur le serveur : associe l'adresse IP avec UUID en jeu
#[derive(Resource, Default)]
pub struct PlayerRegistry {
    pub players: HashMap<SocketAddr, String>, // Adresse -> PlayerID
}

// Gestionnaire de la fréquence des heartbeats
#[derive(Resource)]
pub struct HeartbeatTimer(pub Timer);