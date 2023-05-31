//! Only supports ASCII right now. Didn't think that far ahead. Whoops.
//!
//! Did I mention I hate UI?

use bevy::{prelude::*, ui::FocusPolicy, utils::HashSet};
use bevy_asset_loader::prelude::{AssetCollection, LoadingStateAppExt};
use itertools::Itertools;

use crate::{asset::AssetLoadState, character::AvatarLoadState, util::keys::{InputExt, KeyCodeExt}};

use self::generation::parse_dialogue;

pub mod generation;

#[derive(Resource, AssetCollection)]
pub struct DialogueAssets {
    #[asset(key = "font.fira-sans")]
    pub fira_sans: Handle<Font>,
    #[asset(key = "sfx.dialogue.eightball")]
    pub eightball_blip: Handle<AudioSource>,
}

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StopChars>()
            .init_resource_after_loading_state::<_, DefaultTextStyle>(AssetLoadState::Loading)
            .add_event::<DialogueEvent>()
            .add_collection_to_loading_state::<_, DialogueAssets>(AssetLoadState::Loading)
            .add_startup_system(init_dialogue_box)
            .add_systems(
                (prepare_dialogue_block, speak_dialogue, continue_dialogue)
                    .chain()
                    .in_set(OnUpdate(AssetLoadState::Success)),
            )
            .add_system(test_dialogue.in_schedule(OnEnter(AvatarLoadState::Loaded)));
    }
}

#[derive(Resource)]
pub struct DefaultTextStyle(pub TextStyle);

impl FromWorld for DefaultTextStyle {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<DialogueAssets>();
        Self(TextStyle {
            font: assets.fira_sans.clone(),
            font_size: 24.0,
            color: Color::WHITE,
        })
    }
}

/// Procedurally iterates the characters in a block of dialogue.
///
/// Characters in the `StopChars` resource will momentarily pause dialogue,
/// but *only* if followed by whitespace.
#[derive(Component)]
pub struct TextMotor {
    /// Characters per second.
    pub cps: f32,
    /// Delay after punctuation, in seconds.
    pub stop_delay: f32,
    /// Iterates `TextSection`s for the active block of dialogue.
    pub sections: Box<dyn Iterator<Item = TextSection> + Send + Sync + 'static>,
    /// Iterates characters of the current `TextSection`.
    pub chars: Box<dyn Iterator<Item = char> + Send + Sync + 'static>,
    /// Sound blip when iterating a character.
    pub blip: Handle<AudioSource>,
    /// Accumulated time without writing a character.
    pub acc: f32,
    /// Most recently pushed character.
    pub latest_char: char,
    /// Whether this will skip on the next frame.
    pub skip: bool,
}

/// Which characters followed by whitespace indicate a "pause" in dialogue.
#[derive(Resource)]
pub struct StopChars(pub HashSet<char>);

impl Default for StopChars {
    fn default() -> Self {
        Self(HashSet::from_iter(['.', ',', ';', ':', '!', '?']))
    }
}

#[derive(Component)]
pub struct DialogueWindow;

#[derive(Component)]
pub struct DialoguePortrait;

#[derive(Component)]
pub struct DialogueText;

#[derive(Component)]
pub struct DialogueSelector;

pub fn init_dialogue_box(mut commands: Commands) {
    // mfw I spend multiple days figuring out why html/css is giving me different layouts ;)))
    // https://github.com/bevyengine/bevy/issues/1490
    commands
        .spawn((
            DialogueWindow,
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    direction: Direction::LeftToRight,
                    flex_direction: FlexDirection::Row,
                    position: UiRect {
                        bottom: Val::Percent(0.0),
                        right: Val::Percent(0.0),
                        left: Val::Auto,
                        top: Val::Auto,
                    },
                    padding: UiRect::all(Val::Px(8.0)),
                    size: Size {
                        width: Val::Percent(100.0),
                        height: Val::Px(200.0),
                    },
                    max_size: Size::width(Val::Percent(100.0)),
                    gap: Size::width(Val::Px(8.0)),
                    ..Default::default()
                },
                background_color: BackgroundColor(Color::rgba(0.0, 0.0, 0.0, 0.5)),
                focus_policy: FocusPolicy::Block,
                z_index: ZIndex::Global(1),
                ..Default::default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                DialoguePortrait,
                NodeBundle {
                    style: Style {
                        aspect_ratio: Some(1.0),
                        size: Size::height(Val::Percent(100.0)),
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::RED),
                    ..Default::default()
                },
            ));
            parent
                .spawn((NodeBundle {
                    style: Style {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::PURPLE),
                    ..Default::default()
                },))
                .with_children(|parent| {
                    parent.spawn((
                        DialogueText,
                        TextBundle {
                            style: Style {
                                size: Size::height(Val::Percent(100.0)),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ));
                });

            parent.spawn((
                DialogueSelector,
                NodeBundle {
                    style: Style {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::GREEN),
                    ..Default::default()
                },
            ));
        });
}

#[derive(Default)]
pub struct Dialogue {
    pub text: String,
    pub blip: Handle<AudioSource>,
    pub cps: Option<f32>,
    pub stop_delay: Option<f32>,
    pub next: DialogueNext,
}

#[derive(Default)]
pub enum DialogueNext {
    /// Another block of text.
    Continue(String),
    /// Let the player respond.
    Respond(DialogueOptions),
    #[default]
    Finish,
}

pub enum DialogueEvent {
    Say(Dialogue),
    Finish,
}

#[derive(Component)]
pub struct DialogueOptions {
    pub selected: usize,
    pub options: Vec<DialogueOption>,
}

pub struct DialogueOption {
    /// The icon when selecting this option.
    ///
    /// TODO?: I might need to make a standalone sketch effect for UI images
    /// as I did for `SketchMaterial`. for now this'll do.
    pub icon: Option<Vec<Handle<Image>>>,
    /// Text for this option.
    pub text: String,
    /// Dialogue after selecting this option.
    pub dialogue: Dialogue,
}

#[derive(Component)]
pub struct DialogueOptionIcon;

pub fn prepare_dialogue_block(
    mut commands: Commands,
    text_style: Res<DefaultTextStyle>,
    text_query: Query<Entity, With<DialogueText>>,
    mut window_query: Query<&mut Style, With<DialogueWindow>>,
    mut events: EventReader<DialogueEvent>,
    assets: Res<DialogueAssets>,
) {
    for event in events.iter() {
        match event {
            DialogueEvent::Say(Dialogue {
                text,
                blip,
                cps,
                stop_delay,
                next,
            }) => {
                let e_text = text_query.single();
                commands.entity(e_text).insert(TextMotor {
                    cps: cps.unwrap_or(15.0),
                    stop_delay: stop_delay.unwrap_or(0.4),
                    sections: Box::new(
                        parse_dialogue(text.as_str(), &text_style.0.clone())
                            .sections
                            .into_iter(),
                    ),
                    // will be filled on the first iteration
                    chars: Box::new(std::iter::empty()),
                    blip: blip.clone(),
                    acc: 0.0,
                    latest_char: '\n', // placeholder
                    skip: false,
                });
            }
            DialogueEvent::Finish => {
                let mut style = window_query.single_mut();
                style.display = Display::None;
            }
        }
    }
}

pub fn speak_dialogue(
    mut commands: Commands,
    time: Res<Time>,
    stop_chars: Res<StopChars>,
    audio: Res<Audio>,
    mut sinks: ResMut<Assets<AudioSink>>,
    mut active_blip: Local<Handle<AudioSink>>,
    mut text_query: Query<(Entity, &mut Text, &mut TextMotor), With<DialogueText>>,
) {
    let Ok((e_text, mut text, mut motor)) = text_query.get_single_mut() else {
        return;
    };

    motor.acc += time.delta_seconds();
    let mut spoke = false;

    while motor.skip || motor.acc >= 1.0 / motor.cps {
        match motor.chars.next() {
            Some(c) => {
                text.sections.last_mut().unwrap().value.push(c);

                if !spoke && !c.is_whitespace() {
                    spoke = true;
                }

                // apply extra delay for punctuation
                if (c.is_whitespace() || c == '"') && stop_chars.0.contains(&motor.latest_char) {
                    motor.acc -= motor.stop_delay;
                } else {
                    motor.acc -= 1.0 / motor.cps;
                }

                motor.latest_char = c;
            }
            None => match motor.sections.next() {
                Some(s) => {
                    // copy the style, but put the text in the motor
                    motor.chars = Box::new(s.value.chars().collect_vec().into_iter());
                    text.sections.push(TextSection::from_style(s.style));
                }
                None => {
                    // finished
                    commands.entity(e_text).remove::<TextMotor>();
                    break;
                }
            },
        }
    }

    if spoke {
        // terminate the current blip since it's not looking for overlaps
        if let Some(blip) = sinks.get_mut(&active_blip) {
            blip.stop();
        }
        *active_blip = sinks.get_handle(audio.play(motor.blip.clone()));
    }
}

pub fn display_dialogue_options(
    mut commands: Commands,
    text_style: Res<DefaultTextStyle>,
    options_query: Query<(Entity, &DialogueOptions), Without<Children>>,
) {
    let Ok((e_options, options)) = options_query.get_single() else {
        return;
    };

    commands.entity(e_options).with_children(|parent| {
        for DialogueOption {
            icon,
            text,
            dialogue,
        } in options.options.iter()
        {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        min_size: Size::height(Val::Px(40.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::FUCHSIA),
                    ..Default::default()
                })
                .with_children(|parent| {
                    let mut icon_container = parent.spawn(NodeBundle {
                        style: Style {
                            size: Size::all(Val::Px(40.0)),
                            ..Default::default()
                        },
                        ..Default::default()
                    });

                    if let Some(icon) = icon {
                        icon_container.insert((DialogueOptionIcon, UiImage::new(icon[0].clone())));
                    }

                    parent.spawn(TextBundle {
                        text: parse_dialogue(text, &text_style.0.clone()),
                        background_color: BackgroundColor(Color::AQUAMARINE),
                        ..Default::default()
                    });
                });
        }
    });
}

pub fn select_dialogue_options(
    input: Res<Input<KeyCode>>,
    audio: Res<Audio>,
    mut options_query: Query<(Entity, &mut DialogueOptions)>,
) {
    let Ok((entity, mut options)) = options_query.get_single_mut() else {
        return;
    };

    let mut changed = false;

    if input.any_pressed(KeyCode::ANY_UP) {
        options.selected = (options.selected - 1).max(0);
        changed = true;
    } else if input.any_pressed(KeyCode::ANY_DOWN) {
        options.selected = (options.selected + 1).min(options.options.len());
        changed = true;
    }

    if let Some(index) = input.pressed_number() {
        if index < options.options.len() {
            options.selected = index;
            changed = true;
        }
    }

    if changed {
        audio.play(options.options[options.selected].dialogue.blip.clone());
    }
}

pub fn continue_dialogue(
    input: Res<Input<KeyCode>>,
    text_query: Query<Entity, With<DialogueText>>,
    mut motor_query: Query<&mut TextMotor, With<DialogueText>>,
    mut events: EventWriter<DialogueEvent>,
) {
    if input.just_released(KeyCode::Return) {
        let e_text = text_query.single();
        if let Ok(mut motor) = motor_query.get_mut(e_text) {
            motor.skip = true;
        } else {
            events.send(DialogueEvent::Finish);
        }
    }
}

pub fn test_dialogue(mut events: EventWriter<DialogueEvent>, assets: Res<DialogueAssets>) {
    events.send(DialogueEvent::Say(Dialogue {
        text: "Hey there buddy.".to_owned(),
        blip: assets.eightball_blip.clone(),
        ..Default::default()
    }));
}
