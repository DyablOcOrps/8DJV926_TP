use bevy::prelude::*;
use bevy::time::Stopwatch;
use std::net::{UdpSocket, SocketAddr};
use std::collections::HashMap;
use uuid::Uuid;
// On importe les structures communes définies dans la crate 'shared'.
// Cela garantit que le DS, l'Orchestrateur et le Gatekeeper utilisent exactement les mêmes structures de données.
use shared::{Heartbeat, GameMessage}; 

mod resources;
use resources::{ServerConfig, ServerSocket, PlayerRegistry, HeartbeatTimer};

// ==========================================
// 1. MAIN (Fonction principale)
// ==========================================

fn main() {
    // Récupération dynamique du port via les variables d'environnement. Sinon port 7001 par défaut.
    let ds_port: u16 = std::env::var("DS_PORT")
        .unwrap_or_else(|_| "7001".to_string())
        .parse()
        .expect("Le port DS_PORT fourni est invalide");

    // Récupération du port de l'orchestrateur pour savoir à qui envoyer les heartbeats. Sinon port 4000 par défaut.
    let orch_port = std::env::var("ORCH_PORT").unwrap_or_else(|_| "4000".to_string());
    let orchestrator_addr: SocketAddr = format!("127.0.0.1:{}", orch_port)
        .parse()
        .expect("Adresse de l'orchestrateur invalide");

    App::new()
        .add_plugins(MinimalPlugins)
        // Initialisation des ressources
        .insert_resource(ServerConfig {
            id: Uuid::new_v4().to_string(),
            port: ds_port,
            zone: "zone_A".to_string(),
            max_players: 10,
            orchestrator_addr: orchestrator_addr, // Port de l'orchestrateur
        })
        .insert_resource(PlayerRegistry::default())
        .insert_resource(HeartbeatTimer(Timer::from_seconds(5.0, TimerMode::Repeating)))
        .add_systems(Startup, bind_socket)
        .add_systems(Update, (receive_packets, send_heartbeat).chain())
        .run();
}

// ==========================================
// 2. SYSTÈMES BEVY 
// ==========================================

// Système de démarrage : Initialise le canal réseau UDP.
fn bind_socket(mut commands: Commands, config: Res<ServerConfig>) {
    let addr = format!("0.0.0.0:{}", config.port);
    let socket = UdpSocket::bind(&addr).expect("Impossible de binder le port");
    socket.set_nonblocking(true).expect("Erreur mode non-bloquant");
    
    println!("DS {} démarré sur le port {}", config.id, config.port);

    // On passe le socket à Bevy pour que les autres systèmes puissent l'utiliser
    commands.insert_resource(ServerSocket(socket));
}

// Système de mise à jour : Traite tous les paquets réseaux des joueurs entrants.
fn receive_packets(
    socket: Res<ServerSocket>,
    mut registry: ResMut<PlayerRegistry>,
    config: Res<ServerConfig>
) {
    let mut buf = [0u8; 1024];

    // Boucle de lecture : tant qu'il y a des paquets dans le buffer réseau de la machine
    while let Ok((size, addr)) = socket.0.recv_from(&mut buf) {
        let payload = String::from_utf8_lossy(&buf[..size]);
        
        // Logique JOIN
        if payload.starts_with("JOIN") {
            if registry.players.len() >= config.max_players {
                println!("Serveur plein ({}/{}). Rejet de la connexion de {}", registry.players.len(), config.max_players, addr);
                continue;
            }

            // Génération d'un ID de joueur unique pour la session
            let player_id = Uuid::new_v4().to_string();
            registry.players.insert(addr, player_id.clone());
            
            // Construction de la réponse : "WELCOME {player_id}"
            let response = format!("WELCOME {}", player_id);
            socket.0.send_to(response.as_bytes(), addr).unwrap();
            println!("Nouveau joueur : {} (ID: {})", addr, player_id);
        }
    }
}

// Système de mise à jour : Envoie périodiquement l'état du serveur à l'orchestrateur.
fn send_heartbeat(
    time: Res<Time>,
    mut timer: ResMut<HeartbeatTimer>,
    config: Res<ServerConfig>,
    registry: Res<PlayerRegistry>,
    socket: Res<ServerSocket>
) {
    if timer.0.tick(time.delta()).just_finished() {
        let hb = Heartbeat {
            id: config.id.clone(),
            ip: "127.0.0.1".to_string(),
            port: config.port,
            zone: config.zone.clone(),
            player_count: registry.players.len(),
            max_players: config.max_players,
        };

        // Sérialisation de l'objet en chaine JSON
        let msg = serde_json::to_string(&hb).unwrap();

        // On envoie le heartbeat à l'orchestrateur en UDP
        if let Err(e) = socket.0.send_to(msg.as_bytes(), config.orchestrator_addr) {
            eprintln!("Échec heartbeat : {}", e);
        } else {
            println!("Heartbeat envoyé (Joueurs: {})", hb.player_count);
        }
    }
}