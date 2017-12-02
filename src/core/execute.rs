use std::collections::{HashMap, VecDeque};
use std::iter::FromIterator;
use rand::{thread_rng, Rng};
use core::map::{Dir, PosHex};
use core::{self, Moves, ObjId, PlayerId, State, TileType};
use core::command;
use core::component::{self, Component};
use core::command::Command;
use core::event::{self, ActiveEvent, Event};
use core::effect::{self, Duration, Effect, LastingEffect, TimedEffect};
use core::check::{check, check_attack_at, Error};
use core::movement::Path;
use core::ability::Ability;
use core::map;

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Phase {
    Pre,
    Post,
}

pub fn execute<F>(state: &mut State, command: &Command, cb: &mut F) -> Result<(), Error>
where
    F: FnMut(&State, &Event, Phase),
{
    debug!("Simulator: do_command: {:?}", command);
    if let Err(err) = check(state, command) {
        error!("Check failed: {:?}", err);
        return Err(err);
    }
    match *command {
        Command::Create(ref command) => execute_create(state, cb, command),
        Command::MoveTo(ref command) => execute_move_to(state, cb, command),
        Command::Attack(ref command) => execute_attack(state, cb, command),
        Command::EndTurn(ref command) => execute_end_turn(state, cb, command),
        Command::UseAbility(ref command) => execute_use_ability(state, cb, command),
    }
    Ok(())
}

fn do_event<F>(state: &mut State, cb: &mut F, event: &Event)
where
    F: FnMut(&State, &Event, Phase),
{
    cb(state, event, Phase::Pre);
    event::apply(state, event);
    cb(state, event, Phase::Post);
}

fn execute_move_to<F>(state: &mut State, cb: &mut F, command: &command::MoveTo)
where
    F: FnMut(&State, &Event, Phase),
{
    let id = command.id;
    let mut cost = Some(Moves(1));
    let mut current_path = Vec::new();
    let mut remainder = VecDeque::from_iter(command.path.tiles().iter().cloned());
    while let Some(pos) = remainder.pop_front() {
        if check_reaction_attacks_at(state, id, pos) {
            current_path.push(pos);
            let path = Path::new(current_path.split_off(0));
            do_move(state, cb, id, cost.take(), path);
            let attack_status = try_execute_reaction_attacks(state, cb, id);
            if attack_status == AttackStatus::Hit {
                return;
            }
        }
        current_path.push(pos);
    }
    do_move(state, cb, command.id, cost.take(), Path::new(current_path));
}

fn do_move<F>(state: &mut State, cb: &mut F, id: ObjId, cost: Option<Moves>, path: Path)
where
    F: FnMut(&State, &Event, Phase),
{
    let cost = cost.unwrap_or(Moves(0));
    let active_event = ActiveEvent::MoveTo(event::MoveTo { id, path, cost });
    let event = Event {
        active_event,
        actor_ids: vec![id],
        instant_effects: HashMap::new(),
        timed_effects: HashMap::new(),
    };
    do_event(state, cb, &event);
}

fn check_reaction_attacks_at(state: &mut State, target_id: ObjId, pos: PosHex) -> bool {
    let initial_player_id = state.player_id;
    let mut result = false;
    for obj_id in core::enemy_agent_ids(state, initial_player_id) {
        let command_attack = command::Attack {
            attacker_id: obj_id,
            target_id,
        };
        state.player_id = state.parts.belongs_to.get(obj_id).0;
        if check_attack_at(state, &command_attack, pos).is_ok() {
            result = true;
            break;
        }
    }
    state.player_id = initial_player_id;
    result
}

fn execute_create<F>(state: &mut State, cb: &mut F, command: &command::Create)
where
    F: FnMut(&State, &Event, Phase),
{
    let mut components = state.prototypes.0[&command.prototype].clone();
    if let Some(player_id) = command.owner {
        components.push(Component::BelongsTo(component::BelongsTo(player_id)));
    }
    components.extend_from_slice(&[
        Component::Pos(component::Pos(command.pos)),
        Component::Meta(component::Meta {
            name: command.prototype.clone(),
        }),
    ]);
    let id = state.parts.alloc_id();
    let active_event = ActiveEvent::Create(event::Create {
        pos: command.pos,
        id,
        prototype: command.prototype.clone(),
        components,
    });
    let event = Event {
        active_event,
        actor_ids: vec![id],
        instant_effects: HashMap::new(),
        timed_effects: HashMap::new(),
    };
    do_event(state, cb, &event);
}

#[derive(PartialEq, Clone, Debug)]
enum AttackStatus {
    Hit,
    Miss,
}

fn execute_attack_internal<F>(
    state: &mut State,
    cb: &mut F,
    command: &command::Attack,
    mode: event::AttackMode,
) -> AttackStatus
where
    F: FnMut(&State, &Event, Phase),
{
    let active_event = ActiveEvent::Attack(event::Attack {
        attacker_id: command.attacker_id,
        target_id: command.target_id,
        mode,
    });
    let mut target_effects = Vec::new();
    // TODO: Extract `thread_rng().gen_range(0, 6)` to `roll_dice` func
    // TODO: WE NEED SOME ACTUAL MATH HERE
    if thread_rng().gen_range(0, 6) < 4 {
        let strength = state.parts.strength.get(command.target_id);
        if strength.strength.0 > 1 {
            let damage = core::Strength(1);
            target_effects.push(Effect::Wound(effect::Wound { damage }));
        } else {
            target_effects.push(Effect::Kill);
        }
    }
    let status = if target_effects.is_empty() {
        target_effects.push(Effect::Miss);
        AttackStatus::Miss
    } else {
        AttackStatus::Hit
    };
    let mut effects = HashMap::new();
    effects.insert(command.target_id, target_effects);
    let event = Event {
        active_event,
        actor_ids: vec![command.attacker_id],
        instant_effects: effects,
        timed_effects: HashMap::new(),
    };
    do_event(state, cb, &event);
    status
}

fn try_execute_reaction_attacks<F>(state: &mut State, cb: &mut F, target_id: ObjId) -> AttackStatus
where
    F: FnMut(&State, &Event, Phase),
{
    let target_owner = state.parts.belongs_to.get(target_id).0;
    let mut status = AttackStatus::Miss;
    let initial_player_id = state.player_id;
    for obj_id in core::enemy_agent_ids(state, initial_player_id) {
        if state.parts.agent.get_opt(obj_id).is_none() {
            // check if target is killed
            continue;
        }
        let this_unit_owner = state.parts.belongs_to.get(obj_id).0;
        if this_unit_owner == target_owner {
            continue;
        }
        let command_attack = command::Attack {
            attacker_id: obj_id,
            target_id,
        };
        let command = command::Command::Attack(command_attack.clone());
        state.player_id = this_unit_owner;
        if check(state, &command).is_err() {
            continue;
        }
        let mode = event::AttackMode::Reactive;
        let this_attack_status = execute_attack_internal(state, cb, &command_attack, mode);
        if this_attack_status != AttackStatus::Miss {
            status = this_attack_status;
        }
    }
    state.player_id = initial_player_id;
    status
}

fn execute_attack<F>(state: &mut State, cb: &mut F, command: &command::Attack)
where
    F: FnMut(&State, &Event, Phase),
{
    execute_attack_internal(state, cb, command, event::AttackMode::Active);
    try_execute_reaction_attacks(state, cb, command.attacker_id);
}

fn execute_end_turn<F>(state: &mut State, cb: &mut F, _: &command::EndTurn)
where
    F: FnMut(&State, &Event, Phase),
{
    {
        let player_id_old = state.player_id();
        let active_event = ActiveEvent::EndTurn(event::EndTurn {
            player_id: player_id_old,
        });
        let actor_ids = core::players_agent_ids(state, player_id_old);
        let event = Event {
            active_event,
            actor_ids,
            instant_effects: HashMap::new(),
            timed_effects: HashMap::new(),
        };
        do_event(state, cb, &event);
    }
    {
        let player_id_new = next_player_id(state);
        let active_event = ActiveEvent::BeginTurn(event::BeginTurn {
            player_id: player_id_new,
        });
        let actor_ids = core::players_agent_ids(state, player_id_new);
        let event = Event {
            active_event,
            actor_ids,
            instant_effects: HashMap::new(),
            timed_effects: HashMap::new(),
        };
        do_event(state, cb, &event);
    }

    // TODO: move this block to a separate function
    {
        let phase = effect::Phase(state.player_id().0 as _);
        let ids: Vec<_> = state.parts.schedule.ids().collect();
        for obj_id in ids {
            let pos = state.parts.pos.get(obj_id).0;
            let mut activated = Vec::new();
            {
                let schedule = state.parts.schedule.get_mut(obj_id);
                for planned in &mut schedule.planned {
                    if planned.phase != phase.0 {
                        println!("wrong phase");
                        continue;
                    }
                    planned.rounds -= 1;
                    println!("TICK");
                    if planned.rounds == 0 {
                        println!("BOOM");
                        // TODO: use straight events or something
                        let c = command::UseAbility {
                            ability: planned.ability,
                            id: obj_id,
                            pos,
                        };
                        activated.push(c);
                    }
                }
                schedule.planned.retain(|p| p.rounds > 0);
            }
            for command in activated {
                if state.parts.is_exist(obj_id) {
                    execute_use_ability(state, cb, &command);
                }
            }
        }
    }

    // TODO: move this block to a separate function
    {
        // TODO: tick and kill all the lasting effects here
        // Ouch. This will not work for non-player objects like gas, fire ob bombs.
        // But when should I trigger unit's effects like Poisoned?
        let phase = effect::Phase(state.player_id().0 as _);
        let ids: Vec<_> = state.parts.effects.ids().collect();
        for obj_id in ids {
            for effect in &mut state.parts.effects.get_mut(obj_id).0 {
                if effect.phase == phase {
                    // debug!("TICK: {:?}", effect); // TODO
                    if let Duration::Rounds(ref mut n) = effect.duration {
                        *n -= 1;
                    }
                }
            }

            for effect in &mut state.parts.effects.get_mut(obj_id).0.clone() {
                if effect.phase != phase {
                    continue;
                }
                assert!(state.parts().is_exist(obj_id));
                // TODO: simplify this block!
                {
                    let active_event = event::EffectTick {
                        id: obj_id,
                        effect: effect.effect.clone(),
                    };
                    let mut target_effects = Vec::new();
                    if state.parts.strength.get(obj_id).strength.0 > 1 {
                        let damage = core::Strength(1);
                        target_effects.push(Effect::Wound(effect::Wound { damage }));
                    } else {
                        target_effects.push(Effect::Kill);
                    }
                    let mut instant_effects = HashMap::new();
                    instant_effects.insert(obj_id, target_effects);
                    let event = Event {
                        active_event: ActiveEvent::EffectTick(active_event),
                        actor_ids: vec![obj_id],
                        instant_effects,
                        timed_effects: HashMap::new(),
                    };
                    do_event(state, cb, &event);
                }
                if !state.parts().is_exist(obj_id) {
                    break;
                }
                if effect.duration.is_over() {
                    let active_event = event::EffectEnd {
                        id: obj_id,
                        effect: effect.effect.clone(),
                    };
                    let event = Event {
                        active_event: ActiveEvent::EffectEnd(active_event),
                        actor_ids: vec![obj_id],
                        instant_effects: HashMap::new(),
                        timed_effects: HashMap::new(),
                    };
                    do_event(state, cb, &event);
                }
            }

            if !state.parts().is_exist(obj_id) {
                continue;
            }

            let effects = state.parts.effects.get_mut(obj_id);
            effects.0.retain(|effect| match effect.duration {
                effect::Duration::Rounds(n) => n > 0,
                _ => true,
            });
        }
    }
}

fn execute_use_ability<F>(state: &mut State, cb: &mut F, command: &command::UseAbility)
where
    F: FnMut(&State, &Event, Phase),
{
    let active_event = ActiveEvent::UseAbility(event::UseAbility {
        id: command.id,
        pos: command.pos,
        ability: command.ability,
    });

    // TODO: make sure that reaction attacks hit both agents (Or I want something stupid)

    let mut actor_ids = vec![command.id];
    let mut instant_effects = HashMap::new();
    let mut timed_effects = HashMap::new();
    match command.ability {
        Ability::Knockback => {
            let object_ids = core::object_ids_at(state, command.pos);
            assert_eq!(object_ids.len(), 1);
            let id = object_ids[0];

            // TODO: code duplication
            let actor_pos = state.parts().pos.get(command.id).0;
            let dir = Dir::get_dir_from_to(actor_pos, command.pos);
            let from = command.pos;
            let to = Dir::get_neighbor_pos(command.pos, dir);

            if state.map().is_inboard(to) && !core::is_tile_blocked(state, to) {
                let effect = Effect::Knockback(effect::Knockback { from, to });
                instant_effects.insert(id, vec![effect]);
            }
            actor_ids.push(id);
        }
        Ability::Club => {
            let object_ids = core::object_ids_at(state, command.pos);
            assert_eq!(object_ids.len(), 1);
            let id = object_ids[0];

            // TODO: code duplication
            let actor_pos = state.parts().pos.get(command.id).0;
            let dir = Dir::get_dir_from_to(actor_pos, command.pos);
            let from = command.pos;
            let to = Dir::get_neighbor_pos(command.pos, dir);

            if state.map().is_inboard(to) && !core::is_tile_blocked(state, to) {
                let effect = Effect::FlyOff(effect::FlyOff { from, to });
                instant_effects.insert(id, vec![effect]);
            }
            actor_ids.push(id);
        }
        Ability::Jump => {}
        Ability::Explode => {
            let from = state.parts().pos.get(command.id).0;
            for id in state.parts().agent.ids() {
                let strength = state.parts().strength.get(id);
                let pos = state.parts().pos.get(id).0;
                let distance = map::distance_hex(from, pos);
                if distance.0 > 1 || command.id == id {
                    continue;
                }
                let dir = Dir::get_dir_from_to(from, pos);
                let to = Dir::get_neighbor_pos(pos, dir);
                let mut effects = Vec::new();
                if state.map().is_inboard(to) && !core::is_tile_blocked(state, to) {
                    effects.push(Effect::Knockback(effect::Knockback { from: pos, to }));
                }
                // TODO: this pattern is repeated A LOT:
                if strength.strength.0 > 1 {
                    let damage = core::Strength(1);
                    effects.push(Effect::Wound(effect::Wound { damage }));
                } else {
                    effects.push(Effect::Kill);
                }
                instant_effects.insert(id, effects);
            }
        }
        Ability::Poison => {
            // TODO: de-duplicate these three lines
            let object_ids = core::object_ids_at(state, command.pos);
            assert_eq!(object_ids.len(), 1);
            let id = object_ids[0];
            let owner = state.parts().belongs_to.get(id).0;
            let phase = effect::Phase(owner.0 as _); // TODO: ugly hack

            // TODO: this is just a test block
            {
                if state.parts.schedule.get_opt(id).is_none() {
                    let component = component::Schedule {
                        planned: Vec::new(),
                    };
                    state.parts.schedule.insert(id, component);
                }
                let schedule = state.parts.schedule.get_mut(id);
                let planned_ability = component::PlannedAbility {
                    rounds: 1, // explode on next turn
                    phase: state.player_id.0 as _,
                    ability: Ability::Explode,
                };
                schedule.planned.push(planned_ability);
            }

            let effect = TimedEffect {
                duration: effect::Duration::Rounds(2), // TODO: get from the config
                phase,
                effect: LastingEffect::Poison,
            };
            timed_effects.insert(id, vec![effect]);
            actor_ids.push(id);
        }
    }
    let event = Event {
        active_event,
        actor_ids,
        instant_effects,
        timed_effects,
    };
    do_event(state, cb, &event);
    try_execute_reaction_attacks(state, cb, command.id);
}

fn next_player_id(state: &State) -> PlayerId {
    let current_player_id = PlayerId(state.player_id().0 + 1);
    if current_player_id.0 < state.players_count {
        current_player_id
    } else {
        PlayerId(0)
    }
}

fn random_free_pos(state: &State) -> Option<PosHex> {
    let attempts = 30;
    let radius = state.map().radius();
    for _ in 0..attempts {
        let pos = PosHex {
            q: thread_rng().gen_range(-radius.0, radius.0),
            r: thread_rng().gen_range(-radius.0, radius.0),
        };
        if state.map().is_inboard(pos) && !core::is_tile_blocked(state, pos) {
            return Some(pos);
        }
    }
    None
}

fn random_free_sector_pos(state: &State, player_id: PlayerId) -> Option<PosHex> {
    let attempts = 30;
    let radius = state.map().radius();
    let start_sector_width = radius.0;
    for _ in 0..attempts {
        let q = radius.0 - thread_rng().gen_range(0, start_sector_width);
        let pos = PosHex {
            q: match player_id.0 {
                0 => -q,
                1 => q,
                _ => unimplemented!(),
            },
            r: thread_rng().gen_range(-radius.0, radius.0),
        };
        if state.map().is_inboard(pos) && !core::is_tile_blocked(state, pos) {
            return Some(pos);
        }
    }
    None
}

pub fn create_terrain(state: &mut State) {
    for _ in 0..15 {
        let pos = match random_free_pos(state) {
            Some(pos) => pos,
            None => continue,
        };
        state.map.set_tile(pos, TileType::Rocks);
    }
}

// TODO: improve the API
pub fn create_objects<F>(state: &mut State, cb: &mut F)
where
    F: FnMut(&State, &Event, Phase),
{
    let player_id_initial = state.player_id;
    for &(owner, typename, count) in &[
        (None, "boulder", 10),
        (Some(PlayerId(0)), "swordsman", 2),
        (Some(PlayerId(0)), "spearman", 1),
        (Some(PlayerId(1)), "imp", 7),
    ] {
        if let Some(player_id) = owner {
            state.player_id = player_id;
        }
        for _ in 0..count {
            let pos = match owner {
                Some(player_id) => random_free_sector_pos(state, player_id),
                None => random_free_pos(state),
            }.unwrap();
            let command = Command::Create(command::Create {
                prototype: typename.into(),
                pos,
                owner,
            });
            execute(state, &command, cb).expect("Can't create object");
        }
    }
    state.player_id = player_id_initial;
}
