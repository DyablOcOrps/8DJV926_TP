mod quadtree;
use quadtree::QuadTree;
use bevy::prelude::Vec2;
use tokio::net::UdpSocket;
use std::collections::HashMap;

fn main() {
    println!("Hello, world!");
}

async fn listen_position_updates(quadtree: QuadTree) -> anyhow::Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:5000").await?; // Port au choix pour le service spatial
    let mut buf = [0u8; 1024];

    // Une map pour se souvenir du shard actuel de chaque client
    let mut client_shards: HashMap<u32, u32> = HashMap::new();

    loop {
        let (len, _addr) = socket.recv_from(&mut buf).await?;
        
        // Sécurité : Un message valide fait au moins 1 + 4 + 4 + 4 = 13 octets
        if len >= 13 && buf[0] == 0x10 {
            // Lecture des données binaires (Little Endian)
            let client_id = u32::from_le_bytes(buf[1..5].try_into()?);
            let x = f32::from_le_bytes(buf[5..9].try_into()?);
            let y = f32::from_le_bytes(buf[9..13].try_into()?);

            let pos = Vec2 { x, y };

            // 1. Trouver le shard actuel grâce au QuadTree
            if let Some(new_shard_id) = quadtree.shard_for(pos) {
                
                // On regarde quel était son ancien shard
                let old_shard_id = client_shards.get(&client_id);

                if old_shard_id != Some(&new_shard_id) {
                    // 2. LE SHARD A CHANGÉ ! 
                    println!("Le client {} passe sur le shard {}", client_id, new_shard_id);

                    // Action Réseau A: Envoyer "Unsubscribe" pour l'ancien shard au Broker
                    if let Some(&old_id) = old_shard_id {
                        //send_unsubscribe_to_broker(client_id, old_id).await?;
                    }

                    // Action Réseau B: Envoyer "Subscribe" pour le nouveau shard au Broker
                    send_subscribe_to_broker(client_id, new_shard_id).await?;

                    // Mettre à jour notre mémoire locale
                    client_shards.insert(client_id, new_shard_id);
                }
                if false{
                    
                }
            }
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