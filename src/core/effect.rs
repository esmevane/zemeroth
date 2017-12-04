use core::{self, ObjId, PosHex, State, Strength};
use core::component;

/*
short TODO:
- [x] schedule abilities
- [x] UI: cancel ability mode
- [ ] UI: don't show any ability buttons if there're no APs/JPs
- [.] bomb
  - [ ] explodes in one round
  - [ ] self destroys (before?) after that (or just make it a part of the ability)
- [ ] bomb animation
- [x] stun ability
- [ ] gas cloud - 'reaction' abilities
  - [ ] poison every entering agent
  - [ ] self-destroy in
- [ ] passive ability
  - [ ] knockback on hit
  - [ ] gas cloud on death
- [ ] fix knockback logic
  - [ ] multiple tiles
  - [ ] push everyone around
  - [ ] small dust clouds
- [ ] fix fly-off logic
  - [ ] multiple tiles
  - [ ] arc animation
  - [ ] no collisions with intermediate tiles
- [ ] add a few more unit types
- [ ] clean commits and merge the branch

------

Ok, I want to implements the bombs.
What are the steps?
- bomb throwing is a special ability (at least this is not just a melee attack)
- A bomb object must be created dynamically during the throw
- A bomb must be animated:
  - its flight to the target tile
  - its explosion (a few moving/blending sprites)
- a bomb should explode on the next turn
- an explosion must damage nearby objects
- The explosion knocks every back every neighbor object one tile (effect)

How the timer should be implemented? Is this an effect?..

How other effects should work? 'Poisoned', 'Stunned', etc. should end?
Yes, they only affect their direct target, but
their end should be designated by some visualizations.

So, we need some special event 'Event::EffectIsOver' or something like that.
It'll have a parameter with the type of the effect.


Effects are separated into two groups: Instant and Rounds(i32).

- EffectInstant
- EffectStart/EffectEnd

Fire and Poison objects should functions kinda like bombs.

What confuses me is that TimedEffect with time = Rounds should
create other instant Event every turn.


- Unit hits other unit:
  - Event { Attack, effects: [{Damage, Instant}]}

- Unit throws a bomb:
  - Event { Ability { ThrowBomb}} // ???
  - create a bomb object
  - activate it in N rounds
  - Event Ability Explosion? Effects every one around with Damage and Knockback effects

- Unit enters a Poison Cloud:
  - Event { Ability = Poison, effects: [[target_id, Poisoned { time: TurRounds(3)} ]]}
  - every turn unit must receive some damage through some event (with visualization!)
  - on the last turn some event must disable Poisoned effect (with visualization!)

Should I rename 'effects' to something else?
Bomb or Fire is not effected by anything, it's just their behaviour.

- Add effect `Stunned` for one turn.
- Add effect `Poisoned` for two Rounds.

If demons will be able to create Bomb objects,
I can create an enemy that can create other enemies!

Add a (summoned?) daemon that explodes when dies.

Add PoisonCloud (adds a Poisoned effect) and Fire (damages every turn) objects.

TODO: Add passive abilities (like '100% dash the first reactive attack',
'knockback on every strike', 'explode after death')
*/

// TODO: use this for cooldowns
#[derive(Clone, Debug, Copy, PartialEq, Serialize, Deserialize)]
pub enum Duration {
    Forever, // TODO: ??
    Rounds(i32),
}

impl Duration {
    pub fn is_over(&self) -> bool {
        match *self {
            Duration::Rounds(n) => n <= 0,
            Duration::Forever => false,
        }
    }
}

// TODO: this should equal to `PlayerId`s
// TODO: Document WTF is this
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phase(pub u8);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimedEffect {
    pub duration: Duration,
    pub phase: Phase,
    pub effect: LastingEffect,
}

// Instant effects
#[derive(Clone, Debug)]
pub enum Effect {
    Kill,
    Stun,
    Wound(Wound),
    Knockback(Knockback),
    FlyOff(FlyOff),

    Miss, // TODO: is this really an effect? Maybe rename it to 'Dodged'?
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LastingEffect {
    Poison,
    Stun,
    // Burn,
    // KnockDown,

    // WillExplode, // TODO: I think this should be a separate component/part `plan: Plan,`
}

#[derive(Clone, Debug)]
pub struct Wound {
    pub damage: Strength,
}

#[derive(Clone, Debug)]
pub struct FlyOff {
    pub from: PosHex,
    pub to: PosHex,
}

#[derive(Clone, Debug)]
pub struct Knockback {
    // TODO: store only a direction
    pub from: PosHex,
    pub to: PosHex,
}

pub fn apply_timed(state: &mut State, id: ObjId, timed_effect: &TimedEffect) {
    debug!("effect::apply_timed: {:?}", timed_effect);
    let effects = &mut state.parts.effects;
    if effects.get_opt(id).is_none() {
        effects.insert(id, component::Effects(Vec::new()))
    }
    effects.get_mut(id).0.push(timed_effect.clone());
}

pub fn apply_instant(state: &mut State, id: ObjId, effect: &Effect) {
    debug!("effect::apply_instant: {:?}", effect);
    match *effect {
        Effect::Kill => apply_kill(state, id),
        Effect::Stun => apply_stun(state, id),
        Effect::Wound(ref effect) => apply_wound(state, id, effect),
        Effect::Knockback(ref effect) => apply_knockback(state, id, effect),
        Effect::FlyOff(ref effect) => apply_fly_off(state, id, effect),
        Effect::Miss => (),
    }
}

fn apply_kill(state: &mut State, id: ObjId) {
    state.parts.remove(id);
}

fn apply_stun(state: &mut State, id: ObjId) {
    let agent = state.parts.agent.get_mut(id);
    agent.moves.0 = 0;
    agent.attacks.0 = 0;
    agent.jokers.0 = 0;
}

fn apply_wound(state: &mut State, id: ObjId, effect: &Wound) {
    let strength = state.parts.strength.get_mut(id);
    strength.strength.0 -= effect.damage.0;
    assert!(strength.strength.0 > 0);
}

fn apply_knockback(state: &mut State, id: ObjId, effect: &Knockback) {
    assert!(state.map().is_inboard(effect.from));
    assert!(state.map().is_inboard(effect.to));
    assert!(!core::is_tile_blocked(state, effect.to));
    state.parts.pos.get_mut(id).0 = effect.to;
    // TODO: push anyone who's in the way aside
    // TODO: remove one AP/JP from target
}

fn apply_fly_off(state: &mut State, id: ObjId, effect: &FlyOff) {
    assert!(state.map().is_inboard(effect.from));
    assert!(state.map().is_inboard(effect.to));
    assert!(!core::is_tile_blocked(state, effect.to));
    state.parts.pos.get_mut(id).0 = effect.to;
}
