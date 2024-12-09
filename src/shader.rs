//! Shader module for `berdicles`.

use bevy::{asset::Handle, render::render_resource::Shader};

const fn weak_from_str(s: &str) -> Handle<Shader> {
    if s.len() > 16 {
        panic!()
    }
    let mut bytes = [0u8; 16];
    let s = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        bytes[i] = s[i];
        i += 1;
    }
    Handle::weak_from_u128(u128::from_le_bytes(bytes))
}

pub static PARTICLE_VERTEX: Handle<Shader> = weak_from_str("berdicle/vert");
pub static PARTICLE_FRAGMENT: Handle<Shader> = weak_from_str("berdicle/frag");
pub static PARTICLE_DBG_FRAGMENT: Handle<Shader> = weak_from_str("berdicle/dbg");
pub static TRAIL_VERTEX: Handle<Shader> = weak_from_str("berdicle/trail");
