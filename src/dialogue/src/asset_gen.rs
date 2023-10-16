use bevy::{
    asset::LoadState,
    ecs::query::QuerySingleError,
    prelude::*,
    reflect::{TypePath, TypeUuid},
    render::view::RenderLayers,
    utils::HashMap,
};
use bevy_asset_loader::prelude::*;
use html_parser::{Dom, Node};
use itertools::Itertools;
use serde::Deserialize;

use grin_character::kit::eightball::EightBall;
use grin_render::{
    gopro::{add_gopro_world, GoProSettings},
    sketched::SketchUiImage,
    RenderLayer,
};
use grin_rig::humanoid::Humanoid;

#[derive(Resource)]
pub struct DefaultTextStyle(pub TextStyle);

impl FromWorld for DefaultTextStyle {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(TextStyle {
            font: asset_server.load("fonts/FiraSans-Regular.ttf"),
            font_size: 24.0,
            color: Color::WHITE,
        })
    }
}

#[derive(Resource, AssetCollection)]
pub struct DialogueAssets {
    #[asset(key = "sfx.dialogue.eightball")]
    pub smirk_blip: Handle<AudioSource>,
    #[asset(key = "image.smirk-icon")]
    pub smirk_icon: Handle<SketchUiImage>,
}

#[derive(Resource)]
pub struct PortraitHandles {
    pub smirk: Handle<Image>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum DialogueAssetLoadState {
    #[default]
    Loading,
    Success,
    Failure,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Icon {
    Smirk,
}

impl Icon {
    pub fn from_asset_collection<'a>(
        &self,
        assets: &'a DialogueAssets,
    ) -> &'a Handle<SketchUiImage> {
        match self {
            Icon::Smirk => &assets.smirk_icon,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum Blip {
    Smirk,
}

impl Blip {
    pub fn from_asset_collection<'a>(&self, assets: &'a DialogueAssets) -> &'a Handle<AudioSource> {
        match self {
            Blip::Smirk => &assets.smirk_blip,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub enum Portrait {
    #[default]
    Smirk,
}

impl Portrait {
    // I can't think of a way to query based on component type with regular systems.
    // instead I just threw in `&mut World` access + a macro. not performance critical.
    pub fn render_target(&self, world: &mut World) -> Result<Handle<Image>, QuerySingleError> {
        macro_rules! typed_portrait {
            ( $ty:ident ) => {{
                let humanoid = world
                    .query_filtered::<&Humanoid, With<$ty>>()
                    .get_single(world)?;

                Ok(add_gopro_world(
                    world,
                    GoProSettings {
                        entity: humanoid.head,
                        transform: Transform::from_translation(Vec3::new(0.0, 0.0, -2.0))
                            .looking_to(Vec3::Z, Vec3::Y),
                        size: UVec2::splat(240),
                        render_layers: RenderLayers::layer(RenderLayer::AVATAR as u8),
                    },
                ))
            }};
        }

        match self {
            Portrait::Smirk => typed_portrait!(EightBall),
        }
    }
}

// spent a good few days making this work with `bevy_asset_loader`
// but I guess it was much simpler to just go manual all along!
pub fn add_dialogue_assets(
    mut commands: Commands,
    default_style: Res<DefaultTextStyle>,
    dialogue_assets: Res<DialogueAssets>,
    mut dialogue_maps: ResMut<Assets<DialogueMap>>,
    mut assets: ResMut<Assets<super::Dialogue>>,
    mut next_state: ResMut<NextState<DialogueAssetLoadState>>,
    asset_server: Res<AssetServer>,
) {
    let handles = vec![asset_server.load("dialogue/intro.dialogue.ron")];

    match asset_server.get_group_load_state(handles.iter().map(|h| h.id())) {
        LoadState::Loaded => next_state.set(DialogueAssetLoadState::Success),
        LoadState::Failed => {
            next_state.set(DialogueAssetLoadState::Failure);
            return;
        }
        _ => return,
    };

    for h_dialogue_map in handles.iter() {
        let unparsed_dialogue_map = dialogue_maps.get(h_dialogue_map).unwrap();
        let mut dialogue_map = super::DialogueMap::default();
        for (
            key,
            Dialogue {
                text,
                portrait,
                blip,
                cps,
                stop_delay,
                next,
            },
        ) in unparsed_dialogue_map.0.iter()
        {
            // AVERAGE RUST PROGRAM
            let dialogue = super::Dialogue {
                text: parse_dialogue(text, &default_style.0),
                portrait: portrait.clone(),
                blip: blip.from_asset_collection(&dialogue_assets).clone(),
                cps: cps.unwrap_or(15.0),
                stop_delay: stop_delay.unwrap_or(0.5),
                next: match next {
                    DialogueNext::Continue(dialogue) => {
                        super::DialogueNext::Continue(dialogue.clone())
                    }
                    DialogueNext::Respond(DialogueOptions(options)) => {
                        super::DialogueNext::Respond(super::DialogueOptions::new(
                            options
                                .iter()
                                .map(
                                    |DialogueOption {
                                         icon,
                                         text,
                                         dialogue,
                                     }| {
                                        super::DialogueOption {
                                            dialogue: dialogue.clone(),
                                            icon: icon.as_ref().map(|icon| {
                                                icon.from_asset_collection(&dialogue_assets).clone()
                                            }),
                                            text: parse_dialogue(text, &default_style.0),
                                        }
                                    },
                                )
                                .collect_vec(),
                        ))
                    }
                    DialogueNext::Finish => super::DialogueNext::Finish,
                },
            };
            let h_dialogue = assets.add(dialogue);
            dialogue_map.0.insert(key.clone(), h_dialogue);
        }
        commands.insert_resource(dialogue_map);
    }

    dialogue_maps.clear();
}

// these are raw replacements for some of the structs in `super`
#[derive(Debug, Deserialize, Clone, TypeUuid, TypePath)]
#[uuid = "892b1e02-e963-4f09-a38f-ff02793aa915"]
pub struct DialogueMap(pub HashMap<String, Dialogue>);

#[derive(Debug, Deserialize, Clone)]
pub struct Dialogue {
    pub text: String,
    pub portrait: Portrait,
    pub blip: Blip,
    pub cps: Option<f32>,
    pub stop_delay: Option<f32>,
    pub next: DialogueNext,
}

#[derive(Debug, Deserialize, Clone)]
pub enum DialogueNext {
    Continue(String),
    Respond(DialogueOptions),
    Finish,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DialogueOptions(pub Vec<DialogueOption>);

#[derive(Debug, Deserialize, Clone)]
pub struct DialogueOption {
    pub icon: Option<Icon>,
    pub text: String,
    pub dialogue: String,
}

pub fn parse_dialogue(text: &str, default_style: &TextStyle) -> Text {
    let mut sections = Vec::new();
    let dom = Dom::parse(text).expect("malformed html dialogue");
    for n in dom.children.iter() {
        sections.append(&mut parse_section(n, default_style.clone()));
    }
    Text::from_sections(sections)
}

pub fn parse_color(color: Option<&str>) -> Result<Color, String> {
    // AVERAGE RUST PROGRAM
    let (r, g, b, a) = color
        .ok_or("no color specified")?
        .trim_matches(|c| c == '(' || c == ')')
        .split(", ")
        .into_iter()
        .map(|s| s.parse::<f32>().map_err(|e| e.to_string()))
        .collect_tuple()
        .ok_or("incorrect number of color components")?;
    Ok(Color::rgba(r?, g?, b?, a?))
}

pub fn parse_section(node: &Node, mut style: TextStyle) -> Vec<TextSection> {
    match node {
        Node::Text(t) => vec![TextSection::new(t, style)],
        Node::Element(e) => {
            for (attr, v) in e.attributes.iter() {
                match attr.as_str() {
                    "color" => style.color = parse_color(v.as_deref()).unwrap(),
                    "size" => {
                        style.font_size = v.as_deref().expect("no size specified").parse().unwrap()
                    }
                    _ => warn!("Unrecognized tag in dialogue: {}", attr),
                }
            }

            let mut sections = Vec::new();
            for n in e.children.iter() {
                sections.append(&mut parse_section(n, style.clone()));
            }
            sections
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_test() {
        let text = dbg!(parse_dialogue(
            r#"
                just some white text, chilling
                <span color="(1.0, 0.0, 1.0, 1.0)">MY BROTHER IN TEXT, WE ONLY DO <span size=24>MAGENTA</span> OUT HERE</span>
                :(
            "#,
            &TextStyle::default(),
        ));
        assert_eq!(text.sections.len(), 5);
        assert_eq!(text.sections[0].style.color, Color::WHITE);
        assert_eq!(text.sections[1].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[2].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[3].style.color, Color::FUCHSIA);
        assert_eq!(text.sections[4].style.color, Color::WHITE);
        assert_eq!(text.sections[1].style.font_size, 12.0);
        assert_eq!(text.sections[2].style.font_size, 24.0);
        assert_eq!(text.sections[3].style.font_size, 12.0);
    }
}
