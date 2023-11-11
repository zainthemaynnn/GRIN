use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use bevy::prelude::*;
use bevy_hanabi::prelude::*;

fn calc_func_id<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Modifier to initialize particles at a set velocity.
// for some reason `bevy_hanabi` does not have this...
// perhaps I will PR it in at some point.
#[derive(Reflect, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub struct SetVelocityModifier {
    pub direction: ExprHandle,
    pub speed: ExprHandle,
}

#[typetag::serde]
impl Modifier for SetVelocityModifier {
    fn context(&self) -> ModifierContext {
        ModifierContext::Init
    }

    fn as_init(&self) -> Option<&dyn InitModifier> {
        Some(self)
    }

    fn as_init_mut(&mut self) -> Option<&mut dyn InitModifier> {
        Some(self)
    }

    fn attributes(&self) -> &[Attribute] {
        &[Attribute::VELOCITY]
    }

    fn boxed_clone(&self) -> BoxedModifier {
        Box::new(self.clone())
    }
}

#[typetag::serde]
impl InitModifier for SetVelocityModifier {
    fn apply_init(&self, module: &mut Module, context: &mut InitContext) -> Result<(), ExprError> {
        let func_id = calc_func_id(self);
        let func_name = format!("set_velocity_{0:016X}", func_id);
        let direction = context.eval(module, self.direction)?;
        let speed = context.eval(module, self.speed)?;

        context.init_extra += &format!(
            r##"fn {0}(transform: mat4x4<f32>, particle: ptr<function, Particle>) {{
    let velocity_vec4 = transform * vec4<f32>({1}, 0.0);
    (*particle).{2} = velocity_vec4.xyz * {3};
}}
"##,
            func_name,
            direction,
            Attribute::VELOCITY.name(),
            speed,
        );

        context.init_code += &format!("{}(transform, &particle);\n", func_name);

        Ok(())
    }
}
