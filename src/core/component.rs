use core::{self, map, Attacks, Jokers, MovePoints, Moves, PlayerId};
use core::ability::{Ability, RechargeableAbility};
use core::effect::TimedEffect;

// TODO: Component should declare their dependencies

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pos(pub map::PosHex);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blocker;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Strength {
    pub base_strength: core::Strength,
    pub strength: core::Strength,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BelongsTo(pub PlayerId);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    // dynamic
    pub moves: Moves,
    pub attacks: Attacks,
    pub jokers: Jokers,

    // static
    pub attack_distance: map::Distance,
    pub move_points: MovePoints,
    pub reactive_attacks: Attacks,
    pub base_moves: Moves,
    pub base_attacks: Attacks,
    pub base_jokers: Jokers,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Abilities(pub Vec<RechargeableAbility>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effects(pub Vec<TimedEffect>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannedAbility {
    // TODO: use real types + take effect::Duration into consideration
    pub rounds: u8,
    pub phase: u8,
    pub ability: Ability,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub planned: Vec<PlannedAbility>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Component {
    Pos(Pos),
    Strength(Strength),
    Meta(Meta),
    BelongsTo(BelongsTo),
    Agent(Agent),
    Blocker(Blocker),
    Abilities(Abilities),
    Effects(Effects),
    Schedule(Schedule),
}
