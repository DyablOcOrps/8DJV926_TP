use bevy::prelude::Vec2;
use bevy::prelude::Rect;

pub struct QuadTree {
    bounds: Rect,
    depth: u8,
    max_depth: u8,
    children: Option<Box<[QuadTree; 4]>>,
    shard_id: Option<u32>,  // défini uniquement sur les feuilles
}

impl QuadTree {
    /// Retourne le shard_id de la feuille contenant `pos`.
    pub fn shard_for(&self, pos: Vec2) -> Option<u32> 
    {
        if !self.bounds.contains(pos) {
            return None;
        }

        if let Some(id) = self.shard_id {
            return Some(id);
        }

        if let Some(ref kids) = self.children {
            for child in kids.iter() {
                if let Some(id) = child.shard_for(pos) {
                    return Some(id);
                }
            }
        }

        None
    }

    /// Retourne les shard_ids distincts dans un rayon `margin` autour de `pos`.
    /// Utilisé pour détecter l'approche d'une frontière inter-shard.
    pub fn shards_near(&self, pos: Vec2, margin: f32) -> Vec<u32>
    {
        let mut shard_vector = Vec::new();

        if let Some(current_shard) = self.shard_for(pos) {
            shard_vector.push(current_shard);

            if let Some(right_shard) = self.shard_for(pos + Vec2::new(margin, 0f32)) {
                if right_shard != current_shard{
                    shard_vector.push(right_shard);
                }
            }

            if let Some(left_shard) = self.shard_for(pos + Vec2::new(-1f32 * margin, 0f32)) {
                if left_shard != current_shard{
                    shard_vector.push(left_shard);
                }
            }

            if let Some(up_shard) = self.shard_for(pos + Vec2::new(0f32, margin,)) {
                if up_shard != current_shard{
                    shard_vector.push(up_shard);
                }
            }

            if let Some(down_shard) = self.shard_for(pos + Vec2::new(0f32, -1f32 * margin,)) {
                if down_shard != current_shard{
                    shard_vector.push(down_shard);
                }
            }
        }

        return shard_vector;
    }
}

fn main() {
    println!("Hello, world!");
}
