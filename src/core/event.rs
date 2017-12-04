use std::collections::HashMap;
use core::{Attacks, Jokers, Moves, ObjId, PlayerId, PosHex, State};
use core::component::Component;
use core::ability::{self, Ability};
use core::effect::{self, Effect, LastingEffect, TimedEffect};
use core::movement::Path;

#[derive(Clone, Debug)]
pub struct Event {
    pub active_event: ActiveEvent,
    pub actor_ids: Vec<ObjId>,
    pub instant_effects: HashMap<ObjId, Vec<Effect>>,
    pub timed_effects: HashMap<ObjId, Vec<TimedEffect>>,
}

#[derive(Debug, Clone)]
pub enum ActiveEvent {
    Create(Create),
    EndTurn(EndTurn),
    BeginTurn(BeginTurn),
    UseAbility(UseAbility),

    // TODO: convert these to abilities?
    MoveTo(MoveTo),
    Attack(Attack),

    // TODO: Add a special Event for LastingEffect's Tick/End's
    EffectTick(EffectTick),
    EffectEnd(EffectEnd),
}

#[derive(Debug, Clone)]
pub struct Create {
    pub id: ObjId,
    pub pos: PosHex,
    pub prototype: String,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone)]
pub struct MoveTo {
    pub path: Path,
    pub cost: Moves,
    pub id: ObjId,
}

#[derive(PartialEq, Clone, Debug)]
pub enum AttackMode {
    Active,
    Reactive,
}

#[derive(Debug, Clone)]
pub struct Attack {
    pub attacker_id: ObjId,
    pub target_id: ObjId,
    pub mode: AttackMode,
}

#[derive(Debug, Clone)]
pub struct EndTurn {
    pub player_id: PlayerId,
}

#[derive(Debug, Clone)]
pub struct BeginTurn {
    pub player_id: PlayerId,
}

#[derive(Debug, Clone)]
pub struct UseAbility {
    pub id: ObjId,
    pub pos: PosHex,
    pub ability: Ability,
}

#[derive(Debug, Clone)]
pub struct EffectTick {
    pub id: ObjId,
    pub effect: LastingEffect,
}

#[derive(Debug, Clone)]
pub struct EffectEnd {
    pub id: ObjId,
    pub effect: LastingEffect,
}

pub fn apply(state: &mut State, event: &Event) {
    debug!("event::apply: {:?}", event);
    for (&obj_id, effects) in &event.instant_effects {
        for effect in effects {
            effect::apply_instant(state, obj_id, effect);
        }
    }
    for (&obj_id, effects) in &event.timed_effects {
        for effect in effects {
            effect::apply_timed(state, obj_id, effect);
        }
    }
    apply_event(state, event);
}

pub fn apply_event(state: &mut State, event: &Event) {
    match event.active_event {
        ActiveEvent::Create(ref event) => apply_event_create(state, event),
        ActiveEvent::MoveTo(ref event) => apply_event_move_to(state, event),
        ActiveEvent::Attack(ref event) => apply_event_attack(state, event),
        ActiveEvent::EndTurn(ref event) => apply_event_end_turn(state, event),
        ActiveEvent::BeginTurn(ref event) => apply_event_begin_turn(state, event),
        ActiveEvent::UseAbility(ref event) => apply_event_use_ability(state, event),
        ActiveEvent::EffectTick(ref e) => apply_event_effect_tick(state, e),
        ActiveEvent::EffectEnd(ref e) => apply_event_effect_end(state, e),
    }
}

fn apply_event_create(state: &mut State, event: &Create) {
    let id = event.id;
    for component in &event.components {
        match component.clone() {
            Component::Pos(c) => state.parts.pos.insert(id, c),
            Component::Strength(c) => state.parts.strength.insert(id, c),
            Component::Meta(c) => state.parts.meta.insert(id, c),
            Component::BelongsTo(c) => state.parts.belongs_to.insert(id, c),
            Component::Agent(c) => state.parts.agent.insert(id, c),
            Component::Blocker(c) => state.parts.blocker.insert(id, c),
            Component::Abilities(c) => state.parts.abilities.insert(id, c),
            Component::Effects(c) => state.parts.effects.insert(id, c),
            Component::Schedule(c) => state.parts.schedule.insert(id, c),
        }
    }
}

fn apply_event_move_to(state: &mut State, event: &MoveTo) {
    let agent = state.parts.agent.get_mut(event.id);
    let pos = state.parts.pos.get_mut(event.id);
    pos.0 = *event.path.tiles().last().unwrap();
    if agent.moves.0 > 0 {
        agent.moves.0 -= event.cost.0;
    } else {
        agent.jokers.0 -= event.cost.0;
    }
    assert!(agent.moves >= Moves(0));
    assert!(agent.jokers >= Jokers(0));
}

fn apply_event_attack(state: &mut State, event: &Attack) {
    let agent = state.parts.agent.get_mut(event.attacker_id);
    if agent.attacks.0 > 0 {
        agent.attacks.0 -= 1;
    } else {
        agent.jokers.0 -= 1;
    }
    assert!(agent.attacks >= Attacks(0));
    assert!(agent.jokers >= Jokers(0));
}

fn apply_event_end_turn(state: &mut State, event: &EndTurn) {
    let ids: Vec<_> = state.parts.agent.ids().collect();
    for id in ids {
        let agent = state.parts.agent.get_mut(id);
        let player_id = state.parts.belongs_to.get(id).0;
        if player_id == event.player_id {
            agent.attacks.0 += agent.reactive_attacks.0;
        }
        if let Some(effects) = state.parts.effects.get_opt(id) {
            for effect in &effects.0 {
                match effect.effect {
                    LastingEffect::Stun => agent.attacks.0 = 0,
                    _ => {}
                }
            }
        }
    }
}

fn apply_event_begin_turn(state: &mut State, event: &BeginTurn) {
    // TODO: updated the cooldowns
    state.player_id = event.player_id;
    let ids: Vec<_> = state.parts.agent.ids().collect();
    for id in ids {
        let agent = state.parts.agent.get_mut(id);
        let player_id = state.parts.belongs_to.get(id).0;
        if player_id == event.player_id {
            agent.moves = agent.base_moves;
            agent.attacks = agent.base_attacks;
            agent.jokers = agent.base_jokers;
            if let Some(effects) = state.parts.effects.get_opt(id) {
                for effect in &effects.0 {
                    match effect.effect {
                        LastingEffect::Stun => {
                            agent.moves.0 = 0;
                            agent.attacks.0 = 0;
                            agent.jokers.0 = 0;
                        }
                        _ => {}
                    }
                }
            }
            let abilities = match state.parts.abilities.get_opt_mut(id) {
                Some(abilities) => &mut abilities.0,
                None => continue,
            };
            for ability in abilities {
                // TODO: Move to Status's impl as `update` method
                if let ability::Status::Cooldown(ref mut n) = ability.status {
                    *n -= 1;
                }
                if ability.status == ability::Status::Cooldown(0) {
                    ability.status = ability::Status::Ready;
                }
            }
        }
    }
}

fn apply_event_use_ability(state: &mut State, event: &UseAbility) {
    let abilities = &mut state.parts.abilities.get_mut(event.id).0;
    for ability in abilities {
        if ability.ability == event.ability {
            assert_eq!(ability.status, ability::Status::Ready); // TODO: check this in `check`
            ability.status = ability::Status::Cooldown(2); // TODO: get from the ability
        }
    }

    let agent = state.parts.agent.get_mut(event.id);
    if agent.attacks.0 > 0 {
        agent.attacks.0 -= 1;
    } else if agent.jokers.0 > 0 {
        agent.jokers.0 -= 1;
    } else {
        panic!("internal error: can't use ability if there're not attacks or jokers");
    }

    // TODO: abilities should cause reaction attacks!
    match event.ability {
        Ability::Jump => {
            // TODO: Add some asserts
            state.parts.pos.get_mut(event.id).0 = event.pos;
        }
        Ability::Knockback | Ability::Club | Ability::Poison | Ability::Explode => {}
    }
}

fn apply_event_effect_tick(_: &mut State, effect: &EffectTick) {
    match effect.effect {
        LastingEffect::Poison | LastingEffect::Stun => {
            // TODO: ?
        }
    }
}

fn apply_event_effect_end(_: &mut State, _: &EffectEnd) {
    // TODO: should I delete effect here on in BeginTurn handler?
}
