use tokio::net::UdpSocket;
use redis::{Client, AsyncCommands};
use std::process::Command;
use std::time::Duration;
use shared::{Heartbeat, GameMessage};

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

            let status = if hb.player_count >= hb.max_players {
                "full"
            } else {
                "available"
            };
            
            // Mise à jour atomique dans Redis
            let _: () = con.hset(&key, "status", status).await?;
            let _: () = con.hset(&key, "ip", hb.ip).await?;
            let _: () = con.hset(&key, "port", hb.port).await?;
            let _: () = con.hset(&key, "zone", hb.zone).await?;
            let _: () = con.hset(&key, "player_count", hb.player_count).await?;
            let _: () = con.hset(&key, "max_players", hb.max_players).await?;
            let _: () = con.expire(&key, 15).await?; 
            
            println!("Heartbeat reçu du serveur: {}", hb.id);
        }
    }
}

async fn scaler_loop(redis_client: Client, min_available_servers: usize) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut current_port = 7000; // Gestion simple des ports

    loop {
        interval.tick().await;
        
        if let Ok(mut con) = redis_client.get_async_connection().await {
            // 1. On récupère toutes les clés des serveurs actifs
            let keys: Vec<String> = con.keys("server:*").await.unwrap_or_default();
            
            let mut available_servers_count = 0;

            // 2. On compte combien de ces serveurs sont réellement disponibles
            for key in &keys {
                let status: String = con.hget(key, "status").await.unwrap_or_else(|_| "full".to_string());
                
                if status == "available" {
                    available_servers_count += 1;
                }
            }

            println!("Statut de la flotte : {} serveur(s) disponible(s) sur {} au total (Seuil min : {})", 
                     available_servers_count, keys.len(), min_available_servers);

            // 3. Prise de décision : si on manque de serveurs "disponibles", on en crée un nouveau
            if available_servers_count < min_available_servers {
                println!("Scaling : Nombre de serveurs disponibles ({}) inférieur au minimum requis ({}). Lancement...", 
                         available_servers_count, min_available_servers);
                
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