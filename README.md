# berdicles

Expressive CPU particle system for the bevy engine.

## Feature Set

* Instancing based CPU particles.
* Expressive trait based particles.
* Particles as emitters.
* Particles trails.
* Particle events.

Non-features

* GPU simulation.
* SIMD and similar optimizations.

Basically the crates trades some potential optimizations for expressiveness.

## Getting Started

Add a `ParticleSystemBundle`, which is a `MaterialMeshBundle` with a `ParticleInstance`.

* Huh?

First we need to add `ParticleMaterialPlugin`, not `MaterialPlugin`, which sets up a different render pipeline.
This uses the mesh as the particle shape and the shader for instancing. The `StandardParticle` is already setup
in this crate, but you can define your own `Material` by referencing this shader's source code.

To create a `ParticleInstance` we need a `ParticleSystem` trait implementor and a `Particle` that it spawns.

## Sub-particle Systems

`SubParticleSystem` uses a parent particle system's particles as spawners.

* Add `ParticleParent(Entity)` to point to a parent
* Add the downcast function `as_sub_particle_system` to your `ParticleSystem` implementation.

Yes you can chain these infinitely.

## Event Particle Systems

`EventParticleSystem` can listen to events like particle despawning or colliding and spawn particles on events.

* Add `ParticleParent(Entity)` to point to a parent
* Add `ParticleEventBuffer` to the parent to record these events,
* Add the downcast function `as_event_particle_system` to your `ParticleSystem` implementation.

## Trail Rendering

We can render trails behind particles as mesh.

* Implement `TrailedParticle` on your particle.
* Add `on_update`, `detach_slice` and `as_trail_particle_system` to your `ParticleSystem` implementation.
* Add `TrailMeshOf(Entity)` to a `MaterialMeshBundle` to render them.

## Todo

[ ] Billboard: Already doable with `apply_meta`, but an abstraction would be nice.
