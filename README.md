# berdicles

Instancing and projectile system for bevy.

## Use cases

Despite the name, `berdicles` can do pretty much anything related to instancing,
for example render the same material with different colors.
The crate can create VFX such as particle systems, spawn hair or grass, manage projectile events, etc.

## Feature Set

* Instancing based particles.
* Fully support bevy's mesh and material system.
* Custom shaders and instance buffers.
* Emit projectiles from parent projectiles.
* Mesh based projectile trails.
* Projectile events that spawn other particles, i.e. explosion.
* Multiple renders from the same simulation result via `ProjectileRef`.
* Billboard rendering.

Non-features

* GPU simulation.
* SIMD.

## Getting Started

Add a `ProjectileCluster`, `Mesh3d` and `InstancedMaterial3d` to an entity.

To create a `ProjectileCluster` we need a `ProjectileSystem` trait implementor and
a `Projectile` type that it can spawns.

See the examples folder for more information.

## Trait Based Particles

Physics based particles is commonly seen in most particle system implementations,
but they might be frustrating to work with in some situations.
We provide alternative ways to implement particles, instead of defining things
as velocities, forces or curves.

```rust
impl Particle for SpiralParticle {
    fn update(&mut self, dt: f32) { 
        self.lifetime += dt;
    }

    fn get_transform(&self) -> Transform {
        Transform::from_translation(
            Vec3::new(self.lifetime, 0., 0.)
        ).with_rotation(
            Quat::from_rotation_y(self.lifetime)
        )
    }

    fn expiration_state(&self) -> ExpirationState{
        ExpirationState::explode_if(self.lifetime > 8.)
    }
}
```

## Comparison with `bevy_hanabi`

`berdicle` is more of a projectile system since we have more control
over the simulation and the render pipeline.
Events can be easily extracted from `berdicles` due to this fact. However,
for most VFX with no gameplay function, 
`bevy_hanabi` should be the superior choice.

## Versions

| bevy | berdicles    |
|------|--------------|
| 0.14 | 0.1-0.2      |
| 0.15 | 0.3-latest   |

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
