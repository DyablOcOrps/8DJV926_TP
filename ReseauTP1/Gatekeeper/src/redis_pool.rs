use redis::{AsyncCommands, Client};
use shared::ServerInfo;

pub fn create_client(redis_url: &str) -> Client 
{
    Client::open(redis_url).expect("URL Redis invalide")
}

pub async fn find_available_server(con: &mut redis::aio::Connection) -> Option<ServerInfo> {
    //Get all keys of the server
    let keys: Vec<String> = redis::cmd("KEYS").arg("server:*").query_async(con).await.ok()?;

    for key in keys {
        //HGETALL
        let data: std::collections::HashMap<String, String> = con.hgetall(key).await.ok()?;
        
        //Verify max capacity of the server
        let players: usize = data.get("player_count")?.parse().ok()?;
        let max: usize = data.get("max_players")?.parse().ok()?;

        if players < max {
            return Some(ServerInfo {
                ip: data.get("ip")?.clone(),
                port: data.get("port")?.parse().ok()?,
                zone: data.get("zone")?.clone(),
            });
        }
    }
    None //No server found
}
