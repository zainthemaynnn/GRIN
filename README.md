# GRIN
A third person shooter.

![yes the gif is grainy I didn't want to recreate it okay?](assets/promo.gif)

## Crate descriptions
- `grin_ai`: All enemy agents, shared enemy behavior systems, behavior tree models.
- `grin_asset`: Custom asset loading.
- `grin_character`: Controls, avatar loading.
- `grin_damage`: Damage, health, status effects, projectiles, map hazard behavior.
- `grin_derive`: Procedural macros.
- `grin_dialogue`: Dialogue window. Will probably change to `grin_ui` at some point.
- `grin_input`: Currently just the player camera.
- `grin_item`: All items.
- `grin_map`: Navmesh.
- `grin_physics`: A couple extra physics utilities based off of `bevy_rapier3d`.
- `grin_render`: Materials, particles, post-processing, other VFX.
- `grin_rig`: Generic player and enemy models.
- `grin_time`: Time-related mechanics. Entity histories, time-stops, slow/speed.
- `grin_util`: Math, algorithms, data structures, SFX, some other random things.

## Engine features I'm waiting on
- Extensible `StandardMaterial` (or 3d mesh pipeline in general).
- Entity-entity relations. I'm using this stuff all over the place. I assume it should help performance a lot.
- Window icons. Doable already but only through `winit`. This is low priority enough that I can just wait for first-class.

## Platform support
At the moment every single material texture is represented by a texture array.
This most notably excludes [OpenGL ES 2.0 and WebGL 1.0](https://docs.unity3d.com/Manual/class-Texture2DArray.html).
Might fix it later.

## Development
Due to school it's more difficult to put a lot of time into this. You and I have seen this story with hundreds of student-created indie games before, so I expect you're rolling your eyes and saying "yeah, he's never gonna finish it." And to that, I say:

> true

But check back in six months, and hopefully the repository commits will still be chugging along.

If I do finish it, it will remain free and open source.
