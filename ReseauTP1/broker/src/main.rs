use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use bytes::Bytes;
use game_sockets::{GamePeer, GameConnection, GameStream, GameNetworkEvent};
use game_sockets::protocols::QuicBackend;

//Hashmap for pub/Sub
pub struct BrokerServer {
    //Key topic, value ID connection
    pub subscriptions: HashMap<[u8; 32], HashSet<GameConnection>>,
    
    // Link user ID to GameConnection
    pub client_routing: HashMap<u32, GameConnection>,
    pub shard_routing: HashMap<u32, GameConnection>,
}


impl BrokerServer {
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
            client_routing: HashMap::new(),
            shard_routing: HashMap::new(),
        }
    }

pub fn handle_packet(&mut self, peer: &GamePeer, current_conn: GameConnection, packet: &[u8]) {
        if packet.is_empty() { return; }
        
        let tag = packet[0]; 
        let default_stream = GameStream::from(0); //Default stream
        
        match tag {
            0x01 => { // SUB
                if packet.len() < 37 { return; }
                let client_id = u32::from_le_bytes(packet[1..5].try_into().unwrap());
                
                let mut topic = [0u8; 32];
                topic.copy_from_slice(&packet[5..37]);
                
                // Associate ID to  Connection
                self.client_routing.insert(client_id, current_conn);
                
                //Add connection in subscribe topic
                self.subscriptions.entry(topic).or_default().insert(current_conn);
                println!("Joueur {} (Conn: {}) abonné au topic {:?}", client_id, current_conn.connection_id, topic);
            },
            0x03 => { // PUBLISH / BROADCAST
                if packet.len() < 35 { return; }
                let mut topic = [0u8; 32];
                topic.copy_from_slice(&packet[1..33]);

                let payload_len = u16::from_le_bytes(packet[33..35].try_into().unwrap()) as usize;
                if packet.len() < 35 + payload_len { return; }
                let payload = &packet[35..35 + payload_len];
        
                let mut broadcast_packet = Vec::new();
                broadcast_packet.push(0x04); // Tag de diffusion
                broadcast_packet.extend_from_slice(&(payload_len as u16).to_le_bytes()); 
                broadcast_packet.extend_from_slice(payload);
                let bytes_msg = Bytes::from(broadcast_packet);

                // Send game connection to all subscribe
                if let Some(conns) = self.subscriptions.get(&topic) {
                    for conn in conns {
                        let _ = peer.send(conn, &default_stream, bytes_msg.clone());
                        println!("Envoi du broadcast à la connexion {}", conn.connection_id);
                    }
                }
            },
            0x02 => { // UNSUB
                if packet.len() < 37 { return; }
                let client_id = u32::from_le_bytes(packet[1..5].try_into().unwrap());
                let mut topic = [0u8; 32];
                topic.copy_from_slice(&packet[5..37]);
        
                if let Some(conn) = self.client_routing.get(&client_id) {
                    if let Some(conns) = self.subscriptions.get_mut(&topic) {
                        conns.remove(conn);
                        if conns.is_empty() {
                            self.subscriptions.remove(&topic);
                        }
                    }
                }
            },
            0x05 => { // INPUT ROUTING TO SHARD
                if packet.len() < 21 { return; }
                let client_id = u32::from_le_bytes(packet[1..5].try_into().unwrap());
        
                if let Some(shard_conn) = self.get_shard_conn_for_client(client_id) {
                    let bytes_msg = Bytes::copy_from_slice(packet);
                    let _ = peer.send(&shard_conn, &default_stream, bytes_msg);
                } else {
                    println!("Erreur : Impossible de trouver le shard pour le client {}", client_id);
                }
            },
            _ => println!("Tag inconnu reçu : {}", tag),
        }
    }

    fn get_shard_conn_for_client(&self, client_id: u32) -> Option<GameConnection> {
        //Get game connection of the client
        let client_conn = self.client_routing.get(&client_id)?;
        
        // Search in wich shard he is
        for (topic, conns) in &self.subscriptions {
            if conns.contains(client_conn) {
                let shard_id = self.extract_shard_id_from_topic(topic);
                //Return game connection ID link to this shard
                return self.shard_routing.get(&shard_id).cloned();
            }
        }
        None
    }

    fn extract_shard_id_from_topic(&self, topic: &[u8; 32]) -> u32 {
        let topic_str = String::from_utf8_lossy(topic);
        let clean_str = topic_str.trim_matches('\0'); 
        if let Some((_, id_str)) = clean_str.split_once(':') {
            id_str.parse::<u32>().unwrap_or(0)
        } else {
            0 
        }
    }
}

//Main loop
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Démarrage du Broker MMORPG...");

    // Initialise backend
    let backend = QuicBackend::new(); 

    //Create gamepeer
    let mut peer = GamePeer::new(backend);

    // Listen for the broker
    peer.listen("0.0.0.0", 8060)?;
    println!("Backend réseau initialisé. Écoute sur le port 8060...");

    // Initialisation du Broker partagé
    let broker = Arc::new(Mutex::new(BrokerServer::new()));
    let default_stream = GameStream::from(0);

    // main loop
    loop {
        while let Ok(Some(event)) = peer.poll() {
            let mut b = broker.lock().await;
            
            match event {
                GameNetworkEvent::Connected(conn) => {
                    println!("Nouvelle connexion physique détectée (UUID: {})", conn.connection_id);
                }
                
                GameNetworkEvent::Disconnected(conn) => {
                    println!("Déconnexion physique (UUID: {})", conn.connection_id);
                    
                    // clean up routing table
                    b.client_routing.retain(|_, v| v.connection_id != conn.connection_id);
                    b.shard_routing.retain(|_, v| v.connection_id != conn.connection_id);
                    for conns in b.subscriptions.values_mut() {
                        conns.remove(&conn);
                    }
                }
                
                GameNetworkEvent::Message { connection, stream, data } => {
                    // On donne le paquet à manger à ta logique de routage
                    b.handle_packet(&peer, connection, &data);
                }
                
                GameNetworkEvent::Error { connection, inner } => {
                    println!("Erreur sur la connexion {} : {:?}", connection.connection_id, inner);
                }
                
                GameNetworkEvent::StreamCreated(conn, stream) => {
                    println!("Stream {} créé pour la connexion {}", stream.stream_id, conn.connection_id);
                }
                
                GameNetworkEvent::StreamClosed(conn, stream) => {
                    println!("Stream {} fermé pour la connexion {}", stream.stream_id, conn.connection_id);
                }
            }
        }

        // Évite que le CPU tourne à 100% sur un thread vide. 
        // 1ms de sleep est idéal pour un broker (faible latence).
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }
}