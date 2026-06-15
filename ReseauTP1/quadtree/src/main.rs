mod quadtree;
use quadtree::QuadTree;
use bevy::prelude::Vec2;
use bevy::prelude::Rect;
use tokio::net::UdpSocket;
use std::collections::{HashMap, HashSet};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Initialisation du Service Spatial avec Bevy Math...");

    // Définition des limites avec le Rect de Bevy (min_x, min_y, max_x, max_y)
    let world_bounds = Rect::new(0.0, 0.0, 1000.0, 1000.0);

    // Construction du QuadTree avec la logique Bevy Rect
    let spatial_tree = QuadTree {
        bounds: world_bounds,
        depth: 0,
        max_depth: 1,
        shard_id: None,
        children: Some(Box::new([
            // Shard 0 : Moitié gauche (de x=0 à x=500)
            QuadTree {
                bounds: Rect::new(0.0, 0.0, 500.0, 500.0),
                depth: 1,
                max_depth: 1,
                children: None,
                shard_id: Some(0),
            },
            // Shard 1 : Moitié droite (de x=500 à x=1000)
            QuadTree {
                bounds: Rect::new(500.0, 0.0, 1000.0, 500.0),
                depth: 1,
                max_depth: 1,
                children: None,
                shard_id: Some(1),
            },
            QuadTree { bounds: Rect::new(0.0, 500.0, 500.0, 1000.0), depth: 1, max_depth: 1, children: None, shard_id: Some(2) },
            QuadTree { bounds: Rect::new(500.0, 500.0, 1000.0, 1000.0), depth: 1, max_depth: 1, children: None, shard_id: Some(3) },
        ])),
    };

    println!("Service Spatial en écoute sur le port 5000...");
    listen_position_updates(spatial_tree).await?;

    Ok(())
}

async fn listen_position_updates(quadtree: QuadTree) -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:5000").await?;
    let mut buf = [0u8; 1024];

    // Structure pour suivre l'état de chaque joueur en mémoire
    struct PlayerState {
        current_authority: u32,           // Le shard_id qui a l'autorité actuelle
        active_subscriptions: HashSet<u32>, // Les shards auxquels le client est actuellement abonné
    }

    // Notre registre local : client_id -> PlayerState
    let mut players: HashMap<u32, PlayerState> = HashMap::new();
    
    // Définition de la taille de ta marge (par exemple 10.0 unités de jeu)
    let margin = 10.0; 

    loop {
        let (len, _addr) = socket.recv_from(&mut buf).await?;
        
        if len >= 13 && buf[0] == 0x10 { // Tag 0x10 = PositionUpdate
            let client_id = u32::from_le_bytes(buf[1..5].try_into()?);
            let x = f32::from_le_bytes(buf[5..9].try_into()?);
            let y = f32::from_le_bytes(buf[9..13].try_into()?);

            let pos = Vec2 { x, y };

            // 1. Récupérer ou créer l'état du joueur s'il se connecte pour la première fois
            let player_state = if let Some(state) = players.get_mut(&client_id) {
                state
            } else {
                // S'il est nouveau, on trouve son shard de départ pour initialiser l'autorité
                let initial_shard = quadtree.shard_for(pos).unwrap_or(0);
                players.insert(client_id, PlayerState {
                    current_authority: initial_shard,
                    active_subscriptions: HashSet::new(),
                });
                players.get_mut(&client_id).unwrap()
            };

            // 2. GESTION DE L'AUTORITÉ (Dès qu'on change de feuille dans le Quad Tree)
            if let Some(new_authority_shard) = quadtree.shard_for(pos) {
                if player_state.current_authority != new_authority_shard {
                    println!(
                        " Tranchez la frontière ! L'autorité du client {} passe du shard {} au shard {}", 
                        client_id, player_state.current_authority, new_authority_shard
                    );
                    
                    // messages réseau de transfert d'autorité
                    player_state.current_authority = new_authority_shard;
                }
            }

            // 3. GESTION DES ABONNEMENTS (MARGE)
            // On récupère tous les shards présents dans le rayon "margin" autour du joueur
            let shards_in_range = quadtree.shards_near(pos, margin); // Retourne un Vec<u32>
            let new_subscriptions_set: HashSet<u32> = shards_in_range.into_iter().collect();

            // A. Détecter les ENTRÉES dans une marge -> On s'abonne (Subscribe)
            for &shard_id in &new_subscriptions_set {
                if !player_state.active_subscriptions.contains(&shard_id) {
                    println!(" Entrée dans la marge : Abonnement du client {} au shard:{}", client_id, shard_id);
                    send_subscribe_to_broker(client_id, shard_id).await?;
                }
            }

            // B. Détecter les SORTIES d'une marge -> On se désabonne (Unsubscribe)
            for &shard_id in &player_state.active_subscriptions {
                if !new_subscriptions_set.contains(&shard_id) {
                    println!(" Sortie de la marge : Désabonnement du client {} au shard:{}", client_id, shard_id);
                    send_unsubscribe_to_broker(client_id, shard_id).await?;
                }
            }

            // C. Mettre à jour la liste des abonnements actifs pour le prochain tick
            player_state.active_subscriptions = new_subscriptions_set;
        }
    }
}

async fn send_subscribe_to_broker(client_id: u32, shard_id: u32) -> anyhow::Result<()> {
    let broker_socket = UdpSocket::bind("0.0.0.0:0").await?; // Port aléatoire pour émettre
    //TODO let broker_addr = "adresse du broker";

    let mut packet = Vec::new();
    
    // 1. Tag : 0x01 pour Subscribe
    packet.push(0x01); 

    // 2. client_id (u32 transformé en 4 octets little-endian)
    packet.extend_from_slice(&client_id.to_le_bytes());

    // 3. topic [u8; 32] : on écrit "shard:X"
    let topic_str = format!("shard:{}", shard_id);
    let mut topic_bytes = [0u8; 32];
    
    // On copie le texte dans notre tableau de 32 octets
    let bytes = topic_str.as_bytes();
    let len = bytes.len().min(32);
    topic_bytes[..len].copy_from_slice(&bytes[..len]);
    
    packet.extend_from_slice(&topic_bytes);

    // 4. On envoie les octets au Broker !
    //broker_socket.send_to(&packet, broker_addr).await?;
    Ok(())
}

async fn send_unsubscribe_to_broker(client_id: u32, shard_id: u32) -> anyhow::Result<()> {
    // 1. On ouvre un socket UDP temporaire sur un port libre (0) pour émettre le paquet
    let broker_socket = UdpSocket::bind("0.0.0.0:0").await?;
    //TODO let broker_addr = "adresse du broker";
    
    let mut packet = Vec::new();
    
    // 2. Tag : 0x02 pour Unsubscribe (selon le protocole de l'énoncé)
    packet.push(0x02); 

    // 3. client_id : converti en 4 octets (u32) au format Little-Endian
    packet.extend_from_slice(&client_id.to_le_bytes());

    // 4. topic [u8; 32] : on prépare le nom du shard (ex: "shard:0")
    let topic_str = format!("shard:{}", shard_id);
    let mut topic_bytes = [0u8; 32]; // Tableau fixe de 32 octets rempli de zéros
    
    // On copie les caractères du texte dans notre tableau fixe
    let bytes = topic_str.as_bytes();
    let len = bytes.len().min(32);
    topic_bytes[..len].copy_from_slice(&bytes[..len]);
    
    packet.extend_from_slice(&topic_bytes);

    // 5. Envoi du paquet d'octets au Broker via le réseau
    //broker_socket.send_to(&packet, broker_addr).await?;
    
    Ok(())
}