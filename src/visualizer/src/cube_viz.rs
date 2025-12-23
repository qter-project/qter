use std::{collections::HashMap, sync::Arc};

use bevy::prelude::*;
use internment::ArcIntern;
use puzzle_theory::numbers::{I, Int, chinese_remainder_theorem, lcm_iter};
use qter_core::architectures::{Architecture, decode};

use super::{
    CurrentState, PROGRAMS,
    interpreter_loop::CUBE3,
    interpreter_plugin::{
        BeganProgram, BeginHalt, CubeState, ExecutingInstruction, FinishedProgram, HaltCountUp,
        SolvedGoto,
    },
};

pub struct CubeViz;

impl Plugin for CubeViz {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(CurrentArch(None))
            .add_systems(Startup, setup)
            .add_systems(Update, track_puzzles)
            .add_systems(
                Update,
                (
                    started_program,
                    executed_instruction,
                    state_visualizer,
                    solved_goto_visualizer,
                    start_halt,
                    halt_count,
                    finished_program,
                )
                    .chain(),
            );
    }
}

static NAMES: &[&str] = &["A", "B", "C", "D", "E", "F", "G"];

#[derive(Component)]
struct FaceletIdx(usize);

#[derive(Component)]
struct StateViz;

#[derive(Component)]
struct CycleViz;

#[derive(Component)]
struct Border;

#[derive(Component)]
struct Sticker;

#[derive(Component)]
struct PuzzleMeshes;

#[derive(Component, Clone, Copy, Eq, PartialEq)]
struct WhichPuzzle {
    is_right: bool,
    is_cycle_viz: bool,
}

#[derive(Component)]
struct SolvedGotoStatement;

#[derive(Component)]
struct RegistersViz;

#[derive(Component)]
struct RegistersList;

#[derive(Component)]
struct RegisterValueText(usize);

#[derive(Component)]
struct CycleValueText(usize, usize);

#[derive(Component)]
struct StickerLabel;

#[derive(Resource)]
struct Colors {
    named: HashMap<ArcIntern<str>, Handle<ColorMaterial>>,
    cycles: HashMap<(usize, usize), Handle<ColorMaterial>>,
}

#[derive(Resource)]
struct CurrentArch(Option<(Arc<Architecture>, &'static [Vec<usize>])>);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    window: Single<&Window>,
) {
    commands.spawn(Camera2d);

    commands.insert_resource(CurrentState(CUBE3.identity()));

    let weird_dist = 1. / 3. * 1000.;

    let scale = weird_dist / ((3_f32 / 4.).sqrt() * 2.);

    let rhombus_matrix = Mat2::from_diagonal(Vec2::new(weird_dist, scale))
        * Mat2::from_cols(Vec2::new(-1., 1.), Vec2::new(1., 1.));

    let sticker = meshes.add(Rhombus::new(weird_dist * 2. * 0.9, 2. * scale * 0.9));
    let border = meshes.add(Rhombus::new(weird_dist * 2. * 1.1, 2. * scale * 1.1));

    let spots = [(false, false), (false, true), (true, false), (true, true)];

    let indices = [
        0, 1, 2, 3, 4, 5, 6, 7, // top
        21, 19, 16, 22, 17, 23, 20, 18, // front
        31, 30, 29, 28, 27, 26, 25, 24, // right
        32, 33, 34, 35, 36, 37, 38, 39, // back
        42, 44, 47, 41, 46, 40, 43, 45, // down
        10, 12, 15, 9, 14, 8, 11, 13, // left
    ];

    let mut colors = HashMap::new();

    colors.insert(
        ArcIntern::from("White"),
        materials.add(Color::srgb_u8(255, 255, 255)),
    );
    colors.insert(
        ArcIntern::from("Green"),
        materials.add(Color::srgb_u8(0, 255, 0)),
    );
    colors.insert(
        ArcIntern::from("Red"),
        materials.add(Color::srgb_u8(255, 0, 0)),
    );
    colors.insert(
        ArcIntern::from("Blue"),
        materials.add(Color::srgb_u8(0, 0, 255)),
    );
    colors.insert(
        ArcIntern::from("Orange"),
        materials.add(Color::srgb_u8(255, 128, 0)),
    );
    colors.insert(
        ArcIntern::from("Yellow"),
        materials.add(Color::srgb_u8(255, 255, 0)),
    );
    colors.insert(
        ArcIntern::from("Grey"),
        materials.add(Color::srgb_u8(127, 127, 127)),
    );
    colors.insert(
        ArcIntern::from("Purple"),
        materials.add(Color::srgb_u8(255, 0, 255)),
    );
    colors.insert(
        ArcIntern::from("Transparent"),
        materials.add(Color::srgba_u8(0, 0, 0, 0)),
    );

    let mut cycle_colors = HashMap::new();

    for i in 0..10 {
        for j in 0..10 {
            cycle_colors.insert((i, j), materials.add(cycle_color(i, j)));
        }
    }

    let grey = ArcIntern::from("Grey");
    let transparent = colors.get(&ArcIntern::from("Transparent")).unwrap();

    let center_colors = [
        ArcIntern::<str>::from("White"),
        ArcIntern::from("Green"),
        ArcIntern::from("Red"),
        ArcIntern::from("Blue"),
        ArcIntern::from("Yellow"),
        ArcIntern::from("Orange"),
    ];

    let panel = commands
        .spawn((
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Absolute,
                width: Val::Vw(33.),
                height: Val::Vh(100.),
                top: Val::ZERO,
                right: Val::ZERO,
                ..Default::default()
            },
            // BackgroundColor(Color::srgba_u8(128, 128, 255, 128)),
        ))
        .id();

    commands.spawn((
        Node {
            flex_grow: 1.,
            display: Display::Grid,
            align_content: AlignContent::Start,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            justify_items: JustifyItems::Start,
            column_gap: Val::Px(16.),
            ..Default::default()
        },
        // BackgroundColor(Color::srgba_u8(255, 128, 128, 128)),
        RegistersList,
        ChildOf(panel),
    ));

    let puzzles = commands
        .spawn((
            Node {
                display: Display::Grid,
                column_gap: Val::Px(16.),
                row_gap: Val::Px(16.),
                margin: UiRect::all(Val::ZERO),
                align_items: AlignItems::Stretch,
                align_content: AlignContent::SpaceEvenly,
                justify_items: JustifyItems::Stretch,
                justify_content: JustifyContent::SpaceEvenly,
                grid_template_columns: vec![GridTrack::flex(1.), GridTrack::flex(1.)],
                grid_template_rows: vec![GridTrack::flex(1.), GridTrack::flex(1.)],
                ..Node::default()
            },
            // BackgroundColor(Color::srgba_u8(128, 255, 128, 128)),
            ChildOf(panel),
        ))
        .id();

    commands
        .spawn((
            Node {
                align_self: AlignSelf::Center,
                justify_self: JustifySelf::Center,
                grid_row: GridPlacement::start_span(1, 2),
                grid_column: GridPlacement::start_span(1, 2),
                ..Default::default()
            },
            // BackgroundColor(Color::srgba_u8(128, 255, 128, 128)),
            ChildOf(puzzles),
        ))
        .with_child((
            Text::new(""),
            TextColor(Color::srgb_u8(0, 0, 0)),
            TextFont {
                font_size: window.size().x / 40.,
                ..Default::default()
            },
            SolvedGotoStatement,
        ));

    for (is_cycle_viz, is_right) in spots {
        let which_puzzle = WhichPuzzle {
            is_right,
            is_cycle_viz,
        };

        commands.spawn((
            Node {
                display: Display::Grid,
                aspect_ratio: Some((weird_dist * 3.) / (scale * 6.)),
                margin: UiRect::all(Val::ZERO),
                padding: UiRect::all(Val::ZERO),
                grid_row: GridPlacement::start_span(if is_cycle_viz { 1 } else { 2 }, 1),
                grid_column: GridPlacement::start_span(if is_right { 2 } else { 1 }, 1),
                ..Node::default()
            },
            // BackgroundColor(Color::srgba_u8(128, 255, 255, 128)),
            which_puzzle,
            ChildOf(puzzles),
        ));
        // builder.spawn((
        //     Node {
        //         ..Default::default()
        //     },
        //     BackgroundColor(Color::srgba_u8(128, 0, 255, 127)),
        //     Text2d::new(format!("{is_cycle_viz}-{is_right}")),
        //     TextColor(Color::srgb_u8(128, 255, 255)),
        // ));

        let puzzle_meshes = commands
            .spawn((
                Transform::from_xyz(0., 0., 0.),
                Visibility::default(),
                which_puzzle,
                PuzzleMeshes,
            ))
            .id();

        let rotate = if is_right {
            Mat4::from_scale(Vec3::new(-1., 1., 1.)) * Mat4::from_rotation_z((60_f32).to_radians())
        } else {
            Mat4::IDENTITY
        };

        let idx_to_add = if is_right { 3 } else { 0 };

        let tri_translate = Mat4::from_translation(Vec3::new(0., scale * 3., 0.));

        for (j, tri) in [
            tri_translate,
            Mat4::from_rotation_z((120_f32).to_radians()) * tri_translate,
            Mat4::from_rotation_z((240_f32).to_radians()) * tri_translate,
        ]
        .into_iter()
        .enumerate()
        {
            for (i, (x, y)) in [
                (1., 1.),
                (0., 1.),
                (-1., 1.),
                (1., 0.),
                (-1., 0.),
                (1., -1.),
                (0., -1.),
                (-1., -1.),
                (0., 0.),
            ]
            .into_iter()
            .enumerate()
            {
                let spot = rhombus_matrix * Vec2::new(x, y);
                let transform =
                    rotate * tri * Mat4::from_translation(Vec3::new(spot.x, spot.y, 0.));

                let color = colors
                    .get(if !is_cycle_viz || i == 8 {
                        &center_colors[j + idx_to_add]
                    } else {
                        &grey
                    })
                    .unwrap()
                    .clone();

                if i == 8 {
                    commands.spawn((
                        Mesh2d(sticker.clone()),
                        MeshMaterial2d(color),
                        Transform::from_matrix(transform),
                        ChildOf(puzzle_meshes),
                    ));
                } else {
                    let facelet_idx = indices[(j + idx_to_add) * 8 + i];

                    if is_cycle_viz {
                        commands.spawn((
                            Text2d::new(""),
                            TextColor(Color::BLACK),
                            TextFont {
                                font_size: scale * 2. / 3.,
                                ..Default::default()
                            },
                            Transform::from_matrix(transform)
                                .with_rotation(Quat::IDENTITY)
                                .with_scale(Vec3::new(1., 1., 1.)),
                            FaceletIdx(facelet_idx),
                            StickerLabel,
                            ChildOf(puzzle_meshes),
                        ));

                        commands.spawn((
                            Mesh2d(sticker.clone()),
                            MeshMaterial2d(color),
                            Transform::from_matrix(transform),
                            FaceletIdx(facelet_idx),
                            CycleViz,
                            Sticker,
                            ChildOf(puzzle_meshes),
                        ));
                    } else {
                        commands.spawn((
                            Mesh2d(border.clone()),
                            MeshMaterial2d(transparent.clone()),
                            Transform::from_matrix(
                                Mat4::from_translation(Vec3::new(0., 0., -1.)) * transform,
                            ),
                            FaceletIdx(facelet_idx),
                            StateViz,
                            Border,
                            ChildOf(puzzle_meshes),
                        ));

                        commands.spawn((
                            Mesh2d(sticker.clone()),
                            MeshMaterial2d(color),
                            Transform::from_matrix(transform),
                            FaceletIdx(facelet_idx),
                            StateViz,
                            Sticker,
                            ChildOf(puzzle_meshes),
                        ));

                        // commands.spawn((
                        //     Text2d::new(facelet_idx.to_string()),
                        //     TextColor(Color::srgb_u8(0, 0, 0)),
                        //     TextFont {
                        //         font_size: scale * 2. / 3.,
                        //         ..Default::default()
                        //     },
                        //     Transform::from_matrix(transform)
                        //         .with_rotation(Quat::IDENTITY)
                        //         .with_scale(Vec3::new(1., 1., 1.)),
                        //     ChildOf(puzzle_meshes),
                        // ));
                    }
                }
            }
        }
    }

    commands.insert_resource(Colors {
        named: colors,
        cycles: cycle_colors,
    });
}

fn track_puzzles(
    camera: Single<(&Camera, &GlobalTransform)>,
    puzzle_ui_spots: Query<(&GlobalTransform, &WhichPuzzle, &ComputedNode), Without<PuzzleMeshes>>,
    mut puzzles: Query<(&mut Transform, &WhichPuzzle), With<PuzzleMeshes>>,
    window: Single<&Window>,
) {
    puzzles.par_iter_mut().for_each(|mut puzzle| {
        for spot in puzzle_ui_spots {
            if spot.1 != puzzle.1 {
                continue;
            }

            puzzle.0.translation = camera
                .0
                .viewport_to_world_2d(
                    camera.1,
                    spot.0.translation().xy() / window.resolution.scale_factor(),
                )
                .unwrap()
                .xyx()
                .with_z(0.);

            puzzle.0.scale =
                Vec3::splat(spot.2.size().x / window.resolution.scale_factor() / 2. / 1000.);
        }
    });
}

fn cycle_color(reg_idx: usize, cycle_idx: usize) -> Color {
    #[expect(clippy::cast_precision_loss)]
    Color::oklch(
        0.76 - cycle_idx as f32 * 0.15,
        0.12,
        reg_idx as f32 / 4. * 360. + 240.,
    )
}

#[expect(clippy::too_many_arguments)]
fn started_program(
    colors: Res<Colors>,
    mut current_arch: ResMut<CurrentArch>,
    mut commands: Commands,
    registers_viz: Query<Entity, With<RegistersViz>>,
    mut began_programs: EventReader<BeganProgram>,
    regs_list: Query<(Entity, &RegistersList)>,
    mut regs_stickers: Query<
        (
            &mut MeshMaterial2d<ColorMaterial>,
            &FaceletIdx,
            &CycleViz,
            &Sticker,
        ),
        Without<StickerLabel>,
    >,
    mut sticker_labels: Query<(&mut Text2d, &StickerLabel, &FaceletIdx), Without<Sticker>>,
    window: Single<&Window>,
) {
    let Some(program) = began_programs.read().last() else {
        return;
    };

    let Some((regs_list, RegistersList)) = regs_list.iter().next() else {
        unreachable!();
    };

    for entity in registers_viz {
        if let Ok(mut entity) = commands.get_entity(entity) {
            entity.despawn_related::<Children>();
            entity.despawn();
        }
    }

    let program_info = PROGRAMS.get(&program.0).unwrap();
    let arch = Arc::clone(&program_info.architecture);

    *current_arch = CurrentArch(Some((Arc::clone(&arch), &program_info.solved_goto_pieces)));

    for (i, reg) in arch.registers().iter().enumerate() {
        #[expect(clippy::cast_possible_wrap)]
        #[expect(clippy::cast_possible_truncation)]
        let row = GridPlacement::start_span(i as i16 + 1, 1);

        commands
            .spawn((
                Node {
                    grid_column: GridPlacement::start_span(1, 1),
                    grid_row: row,
                    ..Default::default()
                },
                RegistersViz,
                ChildOf(regs_list),
            ))
            .with_child((
                Text::new(NAMES[i]),
                TextColor::WHITE,
                TextFont {
                    font_size: window.size().x / 30.,
                    ..Default::default()
                },
            ));

        commands
            .spawn((
                Node {
                    grid_column: GridPlacement::start_span(2, 1),
                    grid_row: row,
                    ..Default::default()
                },
                RegistersViz,
                ChildOf(regs_list),
            ))
            .with_child((
                Text::new(format!("= 0/{}  ", reg.order())),
                TextColor::WHITE,
                TextFont {
                    font_size: window.size().x / 45.,
                    ..Default::default()
                },
                RegisterValueText(i),
            ));

        for (j, cycle) in reg.unshared_cycles().iter().enumerate() {
            let cycle_box = commands
                .spawn((
                    Node {
                        #[expect(clippy::cast_possible_wrap)]
                        #[expect(clippy::cast_possible_truncation)]
                        grid_column: GridPlacement::start_span(j as i16 + 3, 1),
                        grid_row: row,
                        justify_self: JustifySelf::Stretch,
                        padding: UiRect::all(Val::Px(4.)),
                        display: Display::Grid,
                        ..Default::default()
                    },
                    RegistersViz,
                    BackgroundColor(cycle_color(i, j)),
                    ChildOf(regs_list),
                ))
                .id();

            let text_container = commands
                .spawn((
                    Node {
                        justify_self: JustifySelf::Center,
                        ..Default::default()
                    },
                    ChildOf(cycle_box),
                ))
                .id();

            commands.spawn((
                Text::new(format!("0/{}", cycle.chromatic_order())),
                TextColor::WHITE,
                TextFont {
                    font_size: window.size().x / 60.,
                    ..Default::default()
                },
                TextLayout::new_with_justify(JustifyText::Center),
                CycleValueText(i, j),
                ChildOf(text_container),
            ));
        }
    }

    let grey = colors.named.get(&ArcIntern::from("Grey")).unwrap();

    regs_stickers
        .par_iter_mut()
        .for_each(|(mut color_material, facelet, CycleViz, Sticker)| {
            *color_material = MeshMaterial2d(grey.clone());

            for (i, reg) in arch.registers().iter().enumerate() {
                for (j, cycle) in reg.unshared_cycles().iter().enumerate() {
                    if cycle.facelet_cycle().contains(&facelet.0) {
                        *color_material =
                            MeshMaterial2d(colors.cycles.get(&(i, j)).unwrap().clone());

                        return;
                    }
                }
            }
        });

    sticker_labels
        .par_iter_mut()
        .for_each(|(mut text, StickerLabel, FaceletIdx(idx))| {
            *text = Text2d::new(String::new());

            for reg in arch.registers() {
                for cycle in reg.unshared_cycles() {
                    if let Some((spot, _)) = cycle
                        .facelet_cycle()
                        .iter()
                        .enumerate()
                        .find(|(_, found_idx)| *found_idx == idx)
                    {
                        *text = Text2d::new(spot.to_string());

                        return;
                    }
                }
            }
        });
}

fn executed_instruction(
    colors: Res<Colors>,
    mut executed_instructions: EventReader<ExecutingInstruction>,
    mut backgrounds: Query<(&mut MeshMaterial2d<ColorMaterial>, &StateViz, &Border)>,
    mut solved_goto_statement: Single<(&mut Text, &SolvedGotoStatement)>,
) {
    let Some(_) = executed_instructions.read().last() else {
        return;
    };

    let transparent = colors.named.get(&ArcIntern::from("Transparent")).unwrap();

    backgrounds
        .par_iter_mut()
        .for_each(|(mut color, StateViz, Border)| *color = MeshMaterial2d(transparent.to_owned()));

    *solved_goto_statement.0 = Text::new("");
}

fn state_visualizer(
    colors: Res<Colors>,
    current_arch: Res<CurrentArch>,
    mut current_state: ResMut<CurrentState>,
    mut cube_states: EventReader<CubeState>,
    mut state_stickers: Query<
        (
            &mut MeshMaterial2d<ColorMaterial>,
            &FaceletIdx,
            &StateViz,
            &Sticker,
        ),
        (Without<RegisterValueText>, Without<CycleValueText>),
    >,
    mut register_value_text: Query<
        (&mut Text, &RegisterValueText),
        (Without<StateViz>, Without<CycleValueText>),
    >,
    mut cycle_value_text: Query<
        (&mut Text, &CycleValueText),
        (Without<StateViz>, Without<RegisterValueText>),
    >,
) {
    let Some(state) = cube_states.read().last() else {
        return;
    };

    state.0.clone_into(&mut current_state.0);

    let mut state_inv = state.0.clone();
    state_inv.exponentiate(-Int::<I>::one());

    state_stickers
        .par_iter_mut()
        .for_each(|(mut color_material, facelet, StateViz, Sticker)| {
            // Qter uses the active "goes to" representation whereas a rubik's cube is effectively displayed in a passive "comes from" representation. If the UFR piece is in the DBL spot, that means that the DBL spot is colored with UFR colors because that's where the piece comes from.
            // We need to invert the puzzle to convert the active representation to the passive one and then display that.

            let new_color = colors
                .named
                .get(&CUBE3.facelet_colors()[state_inv.mapping()[facelet.0]])
                .unwrap()
                .clone();

            *color_material = MeshMaterial2d(new_color);
        });

    let CurrentArch(Some(arch)) = &*current_arch else {
        return;
    };

    let mut regs = Vec::new();

    for reg in arch.0.registers() {
        let mut cycles = Vec::new();

        for cycle in reg.unshared_cycles() {
            let decoded = decode(&state.0, cycle.facelet_cycle(), reg.algorithm());

            cycles.push((decoded, cycle.chromatic_order()));
        }

        regs.push(cycles);
    }

    cycle_value_text
        .par_iter_mut()
        .for_each(|(mut text, CycleValueText(reg_idx, cycle_idx))| {
            let (maybe_value, order) = regs[*reg_idx][*cycle_idx];
            *text = Text::new(match maybe_value {
                Some(value) => format!("{value}/{order}"),
                None => format!("??/{order}"),
            });
        });

    register_value_text
        .par_iter_mut()
        .for_each(|(mut text, RegisterValueText(idx))| {
            let order = lcm_iter(regs[*idx].iter().map(|v| v.1));
            let maybe_value = chinese_remainder_theorem(
                regs[*idx]
                    .iter()
                    .map(|(maybe_value, order)| maybe_value.map(|value| (value, *order))),
            );

            *text = Text::new(match maybe_value {
                Some(value) => format!("= {value}/{order}  "),
                None => format!("= ??/{order}  "),
            });
        });
}

fn translate_solved_goto_pieces(
    arch: &Architecture,
    available_pieces: &[Vec<usize>],
    pieces_got: &[usize],
) -> Vec<usize> {
    let mut out = Vec::new();

    for reg in arch.registers() {
        'next_cycle: for cycle in reg.unshared_cycles() {
            for facelet in cycle.facelet_cycle() {
                if pieces_got.contains(facelet) {
                    for piece in available_pieces {
                        for facelet in piece {
                            if cycle.facelet_cycle().contains(facelet) {
                                out.extend_from_slice(piece);

                                continue 'next_cycle;
                            }
                        }
                    }

                    // unreachable!();
                }
            }
        }
    }

    out
}

fn solved_goto_visualizer(
    colors: Res<Colors>,
    current_state: Res<CurrentState>,
    current_arch: Res<CurrentArch>,
    mut solved_goto_statement: Single<(&mut Text, &mut TextColor, &SolvedGotoStatement)>,
    mut solved_gotos: EventReader<SolvedGoto>,
    mut facelet_borders: Query<(
        &mut MeshMaterial2d<ColorMaterial>,
        &FaceletIdx,
        &StateViz,
        &Border,
    )>,
) {
    let Some(solved_goto) = solved_gotos.read().last() else {
        return;
    };

    let CurrentArch(Some((arch, solved_goto_pieces))) = &*current_arch else {
        unreachable!();
    };

    let purple = colors.named.get(&ArcIntern::from("Purple")).unwrap();

    let color_scheme = CUBE3.facelet_colors();

    let mut taken = true;

    let pieces = translate_solved_goto_pieces(arch, solved_goto_pieces, &solved_goto.facelets.0);

    for (mut color, idx, StateViz, Border) in &mut facelet_borders {
        if pieces.contains(&idx.0) {
            *color = MeshMaterial2d(purple.to_owned());

            taken &= color_scheme[current_state.0.mapping()[idx.0]] == color_scheme[idx.0];
        }
    }

    if taken {
        *solved_goto_statement.0 = Text::new("Taken");
        *solved_goto_statement.1 = TextColor(Color::srgb_u8(0, 255, 0));
    } else {
        *solved_goto_statement.0 = Text::new("Not taken");
        *solved_goto_statement.1 = TextColor(Color::srgb_u8(255, 0, 0));
    }
}

fn start_halt(
    colors: Res<Colors>,
    current_arch: Res<CurrentArch>,
    mut started_halts: EventReader<BeginHalt>,
    mut solved_goto_statement: Single<(&mut Text, &mut TextColor, &SolvedGotoStatement)>,
    mut facelet_borders: Query<(
        &mut MeshMaterial2d<ColorMaterial>,
        &FaceletIdx,
        &StateViz,
        &Border,
    )>,
) {
    let Some(halt) = started_halts.read().last() else {
        return;
    };

    println!("Started halt");

    let CurrentArch(Some((arch, solved_goto_pieces))) = &*current_arch else {
        unreachable!();
    };

    let purple = colors.named.get(&ArcIntern::from("Purple")).unwrap();

    let pieces = translate_solved_goto_pieces(arch, solved_goto_pieces, &halt.facelets.0);
    println!("{pieces:?}");

    facelet_borders
        .par_iter_mut()
        .for_each(|(mut color, idx, StateViz, Border)| {
            if pieces.contains(&idx.0) {
                *color = MeshMaterial2d(purple.to_owned());
            }
        });

    *solved_goto_statement.0 = Text("0".to_owned());
    *solved_goto_statement.1 = TextColor::WHITE;
}

fn halt_count(
    mut halt_count_ups: EventReader<HaltCountUp>,
    mut solved_goto_statement: Single<(&mut Text, &mut TextColor, &SolvedGotoStatement)>,
) {
    let Some(count) = halt_count_ups.read().last() else {
        return;
    };

    *solved_goto_statement.0 = Text(count.0.to_string());
}

fn finished_program(
    // colors: Res<Colors>,
    // mut commands: Commands,
    mut current_arch: ResMut<CurrentArch>,
    mut executed_instructions: EventReader<FinishedProgram>,
    // mut cycle_stickers: Query<(&mut MeshMaterial2d<ColorMaterial>, &CycleViz, &Sticker)>,
    // mut cycle_sticker_labels: Query<(&mut Text2d, &StickerLabel)>,
    // registers_viz: Query<(Entity, &RegistersViz)>,
) {
    let Some(FinishedProgram) = executed_instructions.read().last() else {
        return;
    };

    *current_arch = CurrentArch(None);

    // for (entity, RegistersViz) in registers_viz {
    //     commands.entity(entity).despawn();
    // }

    // let grey = colors.named.get(&ArcIntern::<str>::from("Grey")).unwrap();

    // cycle_stickers
    //     .iter_mut()
    //     .for_each(|(mut color, CycleViz, Sticker)| {
    //         *color = MeshMaterial2d(grey.clone());
    //     });

    // cycle_sticker_labels
    //     .iter_mut()
    //     .for_each(|(mut text, StickerLabel)| {
    //         *text = Text2d::new("");
    //     });
}
