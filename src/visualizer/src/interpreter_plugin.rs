use std::{io::BufReader, net::{SocketAddr, TcpStream}, thread};

use bevy::{
    app::{Plugin, PreUpdate, Startup},
    ecs::{
        event::{Event, EventWriter},
        resource::Resource,
        system::{Commands, Res},
    },
    prelude::Deref,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use internment::Intern;
use interpreter::puzzle_states::{RemoteRobot, RobotLike, SimulatedPuzzle};
use puzzle_theory::{numbers::{I, Int, U}, permutations::Permutation};
use qter_core::{Facelets};

use super::interpreter_loop;

pub struct InterpreterPlugin {
    pub remote: Option<SocketAddr>,
}

#[derive(Event)]
pub struct Message(pub String);

#[derive(Event)]
pub struct Input(pub Int<U>);

#[derive(Event)]
pub struct GaveInput;

#[derive(Event)]
pub struct BeginHalt {
    pub facelets: Facelets,
}

#[derive(Event)]
pub struct HaltCountUp(pub Int<U>);

#[derive(Event)]
pub struct CubeState(pub Permutation);

#[derive(Event)]
pub struct SolvedGoto {
    pub facelets: Facelets,
}

#[derive(Event)]
pub struct ExecutingInstruction {
    pub which_one: usize,
}

#[derive(Event)]
pub struct DoneExecuting;

#[derive(Event)]
pub struct BeganProgram(pub Intern<str>);

#[derive(Event)]
pub struct FinishedProgram;

#[derive(Debug)]
pub enum InterpretationEvent {
    Message(String),
    Input(Int<U>),
    GaveInput,
    BeginHalt { facelets: Facelets },
    HaltCountUp(Int<U>),
    CubeState(Permutation),
    SolvedGoto { facelets: Facelets },
    ExecutingInstruction { which_one: usize },
    DoneExecuting,
    BeganProgram(Intern<str>),
    FinishedProgram,
    // Stuff for highlighting instructions
}

#[derive(Resource, Deref)]
pub struct EventRx(Receiver<InterpretationEvent>);

#[derive(Debug)]
pub enum InterpretationCommand {
    Execute(Intern<str>),
    Step,
    GiveInput(Int<I>),
    Solve,
}

#[derive(Resource, Deref)]
pub struct CommandTx(Sender<InterpretationCommand>);

fn setup<R: RobotLike + Send + 'static>(mut commands: Commands, args: R::InitializationArgs)
where
    R::InitializationArgs: Send,
{
    let (event_tx, event_rx) = unbounded::<InterpretationEvent>();
    let (command_tx, command_rx) = unbounded::<InterpretationCommand>();

    thread::spawn(move || interpreter_loop::interpreter_loop::<R>(event_tx, command_rx, args));

    commands.insert_resource(EventRx(event_rx));
    commands.insert_resource(CommandTx(command_tx));
}

impl Plugin for InterpreterPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_event::<Message>()
            .add_event::<Input>()
            .add_event::<GaveInput>()
            .add_event::<BeginHalt>()
            .add_event::<HaltCountUp>()
            .add_event::<CubeState>()
            .add_event::<SolvedGoto>()
            .add_event::<ExecutingInstruction>()
            .add_event::<DoneExecuting>()
            .add_event::<BeganProgram>()
            .add_event::<FinishedProgram>()
            .add_systems(PreUpdate, read_events);

        if let Some(addr) = self.remote {
            app.add_systems(Startup, move |commands: Commands| {
                let socket = TcpStream::connect(addr).unwrap();
                setup::<RemoteRobot<_>>(commands, BufReader::new(socket))
            });
        } else {
            app.add_systems(Startup, |commands: Commands| setup::<SimulatedPuzzle>(commands, ()));
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn read_events(
    recv: Res<EventRx>,
    mut messages: EventWriter<Message>,
    mut inputs: EventWriter<Input>,
    mut gave_inputs: EventWriter<GaveInput>,
    mut begin_halts: EventWriter<BeginHalt>,
    mut halt_count_ups: EventWriter<HaltCountUp>,
    mut cube_states: EventWriter<CubeState>,
    mut solved_gotos: EventWriter<SolvedGoto>,
    mut executed_instructions: EventWriter<ExecutingInstruction>,
    mut done_executings: EventWriter<DoneExecuting>,
    mut began_programs: EventWriter<BeganProgram>,
    mut finished_programs: EventWriter<FinishedProgram>,
) {
    for event in recv.try_iter() {
        match event {
            InterpretationEvent::Message(msg) => {
                println!("{msg}");
                messages.write(Message(msg));
            }
            InterpretationEvent::Input(int) => {
                inputs.write(Input(int));
            }
            InterpretationEvent::GaveInput => {
                gave_inputs.write(GaveInput);
            }
            InterpretationEvent::BeginHalt { facelets } => {
                begin_halts.write(BeginHalt { facelets });
            }
            InterpretationEvent::HaltCountUp(int) => {
                halt_count_ups.write(HaltCountUp(int));
            }
            InterpretationEvent::CubeState(permutation) => {
                cube_states.write(CubeState(permutation));
            }
            InterpretationEvent::SolvedGoto { facelets } => {
                solved_gotos.write(SolvedGoto { facelets });
            }
            InterpretationEvent::ExecutingInstruction {
                which_one: next_one,
            } => {
                executed_instructions.write(ExecutingInstruction {
                    which_one: next_one,
                });
            }
            InterpretationEvent::DoneExecuting => {
                done_executings.write(DoneExecuting);
            }
            InterpretationEvent::BeganProgram(intern) => {
                began_programs.write(BeganProgram(intern));
            }
            InterpretationEvent::FinishedProgram => {
                finished_programs.write(FinishedProgram);
            }
        }
    }
}
