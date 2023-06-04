//! !!! SPAGHETTI WARNING !!!
//!
//! This module manages what you think it does.
//! Also it only supports ASCII right now. Didn't think that far ahead. Whoops.
//!
//! Did I mention I hate UI?

pub mod asset_gen;

use bevy::{
    prelude::*,
    reflect::TypeUuid,
    ui::FocusPolicy,
    utils::{HashMap, HashSet},
};
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use itertools::Itertools;

use crate::{
    asset::AssetLoadState,
    render::sketched::SketchUiImage,
    util::keys::{InputExt, KeyCodeExt},
};

use self::asset_gen::{DefaultTextStyle, DialogueAssetLoadState};

/// Maps `Dialogue` string ID's (defined in assets file) to `Dialogue` handles.
// the strings are like, handles... for handles.
// this is because dialogue blocks do not have file paths so you can't really refer to them with a handle.
// a bit of extra internal work but doesn't affect the interface for this module so meh.
#[derive(Resource, Default)]
pub struct DialogueMap(pub HashMap<String, Handle<Dialogue>>);

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DefaultTextStyle>()
            .init_resource::<StopChars>()
            .add_state::<DialogueAssetLoadState>()
            .add_asset::<Dialogue>()
            .add_collection_to_loading_state::<_, asset_gen::DialogueAssets>(
                AssetLoadState::Loading,
            )
            .add_plugin(RonAssetPlugin::<asset_gen::DialogueMap>::new(&[
                "dialogue.ron",
            ]))
            .add_event::<DialogueEvent>()
            .add_event::<SelectedDialogueOptionEvent>()
            .add_startup_system(init_dialogue_box)
            .add_systems(
                (
                    prepare_dialogue_block,
                    speak_dialogue,
                    continue_dialogue,
                    apply_system_buffers,
                    select_dialogue_options,
                    display_dialogue_options,
                    apply_system_buffers,
                    highlight_selected_dialogue,
                )
                    .chain()
                    .in_set(OnUpdate(DialogueAssetLoadState::Success)),
            )
            .add_system(asset_gen::add_dialogue_assets.in_set(OnUpdate(AssetLoadState::Success)));
    }
}

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum DialogueSet {
    Dialogue,
    DialogueSelect,
    DialogueShow,
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

#[derive(Component)]
pub struct IconContainer;

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
                background_color: BackgroundColor(Color::BLACK.with_a(0.5)),
                focus_policy: FocusPolicy::Block,
                z_index: ZIndex::Global(1000),
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
                    ..Default::default()
                },
            ));
            parent
                .spawn(NodeBundle {
                    style: Style {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::BLACK.with_a(0.5)),
                    ..Default::default()
                })
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
                    background_color: BackgroundColor(Color::BLACK.with_a(0.5)),
                    ..Default::default()
                },
            ));
        });
}

#[derive(Default, Clone, TypeUuid)]
#[uuid = "25fec548-b7dd-4664-be6c-ced090b2787f"]
pub struct Dialogue {
    pub text: Text,
    pub blip: Handle<AudioSource>,
    pub cps: f32,
    pub stop_delay: f32,
    pub next: DialogueNext,
}

#[derive(Component, Default, Clone)]
pub enum DialogueNext {
    /// Another block of text.
    Continue(String),
    /// Let the player respond.
    Respond(DialogueOptions),
    #[default]
    Finish,
}

pub enum DialogueEvent {
    Say(Handle<Dialogue>),
    Finish,
}

#[derive(Component, Clone)]
pub struct DialogueOptions {
    pub selected: usize,
    pub options: Vec<DialogueOption>,
}

impl DialogueOptions {
    pub fn new(options: impl IntoIterator<Item = DialogueOption>) -> Self {
        Self {
            options: options.into_iter().collect_vec(),
            selected: 0,
        }
    }
}

#[derive(Clone)]
pub struct DialogueOption {
    /// The icon when selecting this option.
    pub icon: Option<Handle<SketchUiImage>>,
    /// Text for this option.
    pub text: Text,
    /// Dialogue after selecting this option.
    pub dialogue: String,
}

#[derive(Component)]
pub struct DialogueOptionIcon;

pub fn prepare_dialogue_block(
    mut commands: Commands,
    dialogue_assets: Res<Assets<Dialogue>>,
    mut text_query: Query<(Entity, &mut Text), With<DialogueText>>,
    mut window_query: Query<&mut Style, With<DialogueWindow>>,
    mut events: EventReader<DialogueEvent>,
) {
    let (e_text, mut text) = text_query.single_mut();
    for event in events.iter() {
        text.sections.clear();
        match event {
            DialogueEvent::Say(h_dialogue) => {
                let Dialogue {
                    text,
                    blip,
                    cps,
                    stop_delay,
                    next,
                } = dialogue_assets.get(h_dialogue).unwrap().clone();

                commands.entity(e_text).insert((
                    TextMotor {
                        cps,
                        stop_delay,
                        sections: Box::new(text.sections.into_iter()),
                        // will be filled on the first iteration
                        chars: Box::new(std::iter::empty()),
                        blip: blip.clone(),
                        acc: 0.0,
                        latest_char: '\n', // placeholder
                        skip: false,
                    },
                    next,
                ));
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
    options_query: Query<(Entity, &DialogueOptions), Without<Children>>,
) {
    let Ok((e_options, options)) = options_query.get_single() else {
        return;
    };

    commands.entity(e_options).with_children(|parent| {
        for DialogueOption { text, .. } in options.options.iter() {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        min_size: Size::height(Val::Px(40.0)),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .with_children(|parent| {
                    parent.spawn((
                        IconContainer,
                        NodeBundle {
                            style: Style {
                                size: Size::height(Val::Px(64.0)),
                                aspect_ratio: Some(1.5),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    ));

                    parent.spawn(TextBundle {
                        text: text.clone(),
                        ..Default::default()
                    });
                });
        }
    });
}

pub struct SelectedDialogueOptionEvent {
    pub option: DialogueOption,
    pub selected: usize,
    pub deselected: Option<usize>,
}

pub fn select_dialogue_options(
    input: Res<Input<KeyCode>>,
    audio: Res<Audio>,
    dialogue_map: Res<DialogueMap>,
    dialogue_assets: Res<Assets<Dialogue>>,
    mut options_query: Query<&mut DialogueOptions, With<DialogueSelector>>,
    mut events: EventWriter<SelectedDialogueOptionEvent>,
) {
    let Ok(mut options) = options_query.get_single_mut() else {
        return;
    };

    let mut changed = false;
    let pre_selected = options.selected;

    if input.any_pressed(KeyCode::ANY_UP) && options.selected > 0 {
        options.selected -= 1;
        changed = true;
    } else if input.any_pressed(KeyCode::ANY_DOWN) && options.selected < options.options.len() - 1 {
        options.selected += 1;
        changed = true;
    }

    if let Some(index) = input.just_released_number().map(|i| i - 1) {
        if index < options.options.len() {
            options.selected = index;
            changed = true;
        }
    }

    let option = &options.options[options.selected];
    if changed {
        let handle = &dialogue_map.0[&option.dialogue].clone();
        let dialogue = dialogue_assets.get(handle).unwrap();
        audio.play(dialogue.blip.clone());
    }

    if options.selected != pre_selected {
        events.send(SelectedDialogueOptionEvent {
            option: option.clone(),
            selected: options.selected,
            deselected: Some(pre_selected),
        });
    }
}

pub fn highlight_selected_dialogue(
    mut commands: Commands,
    selector_query: Query<&Children, With<DialogueSelector>>,
    children_query: Query<&Children, Without<DialogueSelector>>,
    icon_query: Query<(), With<IconContainer>>,
    mut events: EventReader<SelectedDialogueOptionEvent>,
) {
    for SelectedDialogueOptionEvent {
        option,
        selected,
        deselected,
    } in events.iter()
    {
        let selector_children = selector_query.single();
        let selected = selector_children[*selected];
        let deselected = deselected.map(|i| selector_children[i]);

        if let Some(deselected) = deselected {
            for child in children_query.get(deselected).unwrap().iter() {
                if icon_query.get(*child).is_ok() {
                    commands
                        .entity(*child)
                        .remove::<(UiImage, Handle<SketchUiImage>, BackgroundColor)>();
                }
            }
        }

        for child in children_query.get(selected).unwrap().iter() {
            if let Some(icon) = &option.icon {
                if icon_query.get(*child).is_ok() {
                    commands.entity(*child).insert(icon.clone());
                }
            }
        }
    }
}

// yucky. this is a yucky system.
pub fn continue_dialogue(
    mut commands: Commands,
    input: Res<Input<KeyCode>>,
    dialogue_map: Res<DialogueMap>,
    text_query: Query<(Entity, &DialogueNext), With<DialogueText>>,
    mut motor_query: Query<&mut TextMotor, With<DialogueText>>,
    selector_query: Query<Entity, With<DialogueSelector>>,
    opts_query: Query<&DialogueOptions, With<DialogueSelector>>,
    mut events: EventWriter<DialogueEvent>,
    mut opt_events: EventWriter<SelectedDialogueOptionEvent>,
) {
    if input.just_released(KeyCode::Return) {
        let (e_text, next) = text_query.single();
        let e_select = selector_query.single();
        if let Ok(mut motor) = motor_query.get_mut(e_text) {
            motor.skip = true;
        } else if let Ok(opts) = opts_query.get(e_select) {
            let handle = dialogue_map.0[&opts.options[opts.selected].dialogue].clone();
            events.send(DialogueEvent::Say(handle));
            commands
                .entity(e_select)
                .remove::<DialogueOptions>()
                .despawn_descendants();
        } else {
            match next {
                DialogueNext::Continue(dialogue) => {
                    let handle = dialogue_map.0[dialogue].clone();
                    events.send(DialogueEvent::Say(handle));
                }
                DialogueNext::Respond(opts) => {
                    commands.entity(e_select).insert(opts.clone());
                    opt_events.send(SelectedDialogueOptionEvent {
                        option: opts.options[0].clone(),
                        selected: 0,
                        deselected: None,
                    });
                }
                DialogueNext::Finish => events.send(DialogueEvent::Finish),
            }
        }
    }
}

// TODO: unit test this thing. maybe. someday.
