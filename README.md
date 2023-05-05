# GRIN
A third person shooter.

![yes the gif is grainy I didn't want to recreate it okay?](assets/promo.gif)

## Engine features I'm waiting on
- Morph targets: Needed for character animations.
- Extensible `StandardMaterial`: Reduces boilerplate.
- Ability to disable transform propagation: Propagation is nice but it makes physics tricky.

## Platform support
At the moment every single material texture is represented by a texture array.
This most notably excludes [OpenGL ES 2.0 and WebGL 1.0](https://docs.unity3d.com/Manual/class-Texture2DArray.html).
Might fix it later.
