# GRIN
A third person shooter.

![yes the gif is grainy I didn't want to recreate it okay?](assets/promo.gif)

## Engine features I'm waiting on
- Morph targets: Needed for character animations.
    - Alternative: It would be nice to have some access to shear. `GlobalTransform` uses an affine matrix but `Transform` doesn't,
    and it's impossible to mutate it so that it actually does affine transformations.
- Extensible `StandardMaterial`: Reduces boilerplate.
- Ability to disable transform propagation: Propagation is nice but it makes physics tricky.
    - Related... Entity-entity relations (custom hierarchy): Currently using a bootleg ones for rewinds. Not amazing.
- Text wrapping with percent layout: Self-explanatory.
- Window icons: Doable already but only through `winit`. This is low priority enough that I can just wait for first-class.

## Platform support
At the moment every single material texture is represented by a texture array.
This most notably excludes [OpenGL ES 2.0 and WebGL 1.0](https://docs.unity3d.com/Manual/class-Texture2DArray.html).
Might fix it later.
