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
        
        // Logique simplifiée pour compter les serveurs
        if let Ok(mut con) = redis_client.get_async_connection().await {
            let keys: Vec<String> = con.keys("server:*").await.unwrap_or_default();
            let count = keys.len();

            if count < min_servers {
                println!("Scaling: {}/{} serveurs. Lancement...", count, min_servers);
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