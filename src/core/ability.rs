#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ability {
    Knockback,

    // causes `FlyOff` effect
    Club,

    Jump, // TODO: range?

    Poison,

    Explode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    Ready,
    Cooldown(i32), // TODO: i32 -> Rounds
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RechargeableAbility {
    pub ability: Ability,
    pub status: Status,
    // TODO: base_cooldown: Rounds,
}

impl Ability {
    pub fn to_str(&self) -> &str {
        match *self {
            Ability::Knockback => "Knockback",
            Ability::Club => "Club",
            Ability::Jump => "Jump",
            Ability::Poison => "Poison",
            Ability::Explode => "Explode",
        }
    }
}
