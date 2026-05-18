use tokio::net::UdpSocket;
use redis::{Client, AsyncCommands};
use std::process::Command;
use std::time::Duration;
use serde::{Deserialize, Serialize};

// Définition de la structure attendue des serveurs
#[derive(Deserialize, Serialize, Debug)]
pub struct Heartbeat {
    pub id: String,
    pub ip: String,
    pub port: u16,
    pub zone: String,
    pub player_count: usize,
    pub max_players: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let redis_client = Client::open("redis://127.0.0.1/")?;
    let orch_port_str = std::env::var("ORCH_PORT").unwrap_or_else(|_| "4000".to_string());
    let orch_port: u16 = orch_port_str.parse().expect("Port invalide");

    let min_servers = 2; 

    println!("Orchestrateur démarré sur le port {}", orch_port);

    let hb_redis = redis_client.clone();
    let listener_handle = tokio::spawn(async move {
        if let Err(e) = heartbeat_listener(hb_redis, orch_port).await {
            eprintln!("Erreur Listener: {}", e);
        }
    });

    let scaler_redis = redis_client.clone();
    let scaler_handle = tokio::spawn(async move {
        scaler_loop(scaler_redis, min_servers).await;
    });

    let _ = tokio::try_join!(listener_handle, scaler_handle);
    Ok(())
}

async fn heartbeat_listener(redis_client: Client, port: u16) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    let mut buf = [0u8; 1024];

    loop {
        let (len, _addr) = socket.recv_from(&mut buf).await?;
        // Désérialisation du JSON reçu
        if let Ok(hb) = serde_json::from_slice::<Heartbeat>(&buf[..len]) {
            let mut con = redis_client.get_async_connection().await?;
            let key = format!("server:{}", hb.id);
            
            // Mise à jour atomique dans Redis
            let _: () = con.hset(&key, "status", "available").await?;
            let _: () = con.hset(&key, "port", hb.port).await?;
            let _: () = con.hset(&key, "player_count", hb.player_count).await?;
            let _: () = con.hset(&key, "max_players", hb.max_players).await?;
            let _: () = con.expire(&key, 15).await?; 
            
            println!("Heartbeat reçu du serveur: {}", hb.id);
        }
    }
}

async fn scaler_loop(redis_client: Client, min_servers: usize) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut current_port = 7000; // Gestion simple des ports

    loop {
        interval.tick().await;
        
        if let Ok(mut con) = redis_client.get_async_connection().await {
            // 1. On récupère toutes les clés des serveurs
            let keys: Vec<String> = con.keys("server:*").await.unwrap_or_default();
            let total_servers = keys.len();
            
            let mut all_servers_are_full = true;

            // 2. On inspecte chaque serveur pour voir s'il est plein
            for key in &keys {
                // On récupère le nombre de joueurs et le max
                let player_count: usize = con.hget(key, "player_count").await.unwrap_or(0);
                let max_players: usize = con.hget(key, "max_players").await.unwrap_or(1); // Évite la division par 0

                // Si on trouve AU MOINS UN serveur qui n'est pas plein
                if player_count < max_players {
                    all_servers_are_full = false;
                }
            }

            // Si la flotte est vide, alors par définition "tous les serveurs ne sont pas pleins", 
            // mais on doit quand même spawn pour respecter le min_servers.
            let technical_full = total_servers > 0 && all_servers_are_full;

            // 3. Prise de décision pour le Scaling
            if total_servers < min_servers || technical_full {
                if technical_full {
                    println!("Scaling: Tous les serveurs existants sont PLEINS ! Lancement d'un serveur de secours...");
                } else {
                    println!("Scaling: {}/{} serveurs. Lancement...", total_servers, min_servers);
                }
                
                spawn_server(current_port).await;
                current_port += 1;
            }
        }
    }
}

async fn spawn_server(port: u16) {
    // On lance le binaire compilé du DS
    // Bien vérifier que ce chemin existe avant de tester
    let _ = Command::new("./target/debug/dedicated_server")
        .env("DS_PORT", port.to_string())
        .env("ORCH_PORT", "4000")
        .spawn();
}