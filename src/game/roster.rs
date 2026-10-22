//! Squad data: skill attributes, persons (squad members) and teams.

use super::text::Text;
use crate::data::{addr, AtariRom};

/// One of the eight player skills, in the order the game stores them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Aggression,
    Attack,
    Defence,
    Speed,
    Throwing,
    Power,
    Stamina,
    Intelligence,
}

impl Skill {
    pub const ALL: [Skill; 8] = [
        Skill::Aggression,
        Skill::Attack,
        Skill::Defence,
        Skill::Speed,
        Skill::Throwing,
        Skill::Power,
        Skill::Stamina,
        Skill::Intelligence,
    ];

    pub fn from_index(index: i32) -> Option<Skill> {
        usize::try_from(index)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }
}

/// The eight skills, each nominally 100..=250.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Skills {
    pub agr: i32,
    pub att: i32,
    pub def: i32,
    pub spd: i32,
    pub thr: i32,
    pub pow: i32,
    pub sta: i32,
    pub int: i32,
}

impl Skills {
    pub fn uniform(value: i32) -> Self {
        Self::from_array([value; 8])
    }

    pub fn from_array(v: [i32; 8]) -> Self {
        Self {
            agr: v[0],
            att: v[1],
            def: v[2],
            spd: v[3],
            thr: v[4],
            pow: v[5],
            sta: v[6],
            int: v[7],
        }
    }

    pub fn to_array(self) -> [i32; 8] {
        [
            self.agr, self.att, self.def, self.spd, self.thr, self.pow, self.sta, self.int,
        ]
    }

    pub fn get_mut(&mut self, skill: Skill) -> &mut i32 {
        match skill {
            Skill::Aggression => &mut self.agr,
            Skill::Attack => &mut self.att,
            Skill::Defence => &mut self.def,
            Skill::Speed => &mut self.spd,
            Skill::Throwing => &mut self.thr,
            Skill::Power => &mut self.pow,
            Skill::Stamina => &mut self.sta,
            Skill::Intelligence => &mut self.int,
        }
    }

    pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut i32)) {
        for skill in Skill::ALL {
            f(self.get_mut(skill));
        }
    }
}

/// An 11-byte attribute record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    pub health: i32,
    /// Direction the team attacks in: 0 (up) or 4 (down).
    pub initial_dir: i32,
    /// Formation position: 0 goalkeeper, 1 defence, 2 midfield, 3+ attack.
    pub pos: i32,
    pub skills: Skills,
}

impl Attributes {
    pub const SIZE: i32 = 0xB;

    pub fn read(rom: &AtariRom, address: i32) -> Self {
        let skill = |i: i32| rom.u8(address + 3 + i);
        Self {
            health: rom.u8(address),
            initial_dir: rom.u8(address + 1),
            pos: rom.u8(address + 2),
            skills: Skills::from_array(std::array::from_fn(|i| skill(i as i32))),
        }
    }

    /// Rounds every skill down to a multiple of ten (applied to injured players).
    pub fn round_skills_down(&mut self) {
        self.skills.for_each_mut(|value| *value = *value / 10 * 10);
    }
}

/// The attribute tables a person record can point into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributesTable {
    Team1,
    Team2,
    Temp,
    Market,
}

/// A pointer into one of the attribute tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributesRef {
    pub table: AttributesTable,
    pub index: usize,
}

impl AttributesRef {
    pub fn from_address(address: i32) -> Self {
        let (table, base) = match address {
            a if (addr::ATTRIBUTES_TEAM1..addr::ATTRIBUTES_TEAM2).contains(&a) => {
                (AttributesTable::Team1, addr::ATTRIBUTES_TEAM1)
            }
            a if (addr::ATTRIBUTES_TEAM2..addr::ATTRIBUTES_TEMP).contains(&a) => {
                (AttributesTable::Team2, addr::ATTRIBUTES_TEAM2)
            }
            a if (addr::ATTRIBUTES_TEMP..addr::ATTRIBUTES_TEMP_END).contains(&a) => {
                (AttributesTable::Temp, addr::ATTRIBUTES_TEMP)
            }
            a if (addr::ATTRIBUTES_MARKET..addr::ATTRIBUTES_MARKET_END).contains(&a) => {
                (AttributesTable::Market, addr::ATTRIBUTES_MARKET)
            }
            other => panic!("{other:#X} is not an attribute record"),
        };
        Self {
            table,
            index: ((address - base) / Attributes::SIZE) as usize,
        }
    }
}

/// A squad member.
#[derive(Clone, Debug)]
pub struct Person {
    pub attributes: AttributesRef,
    pub name: String,
    pub id: i32,
}

impl Person {
    pub const SIZE: i32 = 0xE;

    pub fn read(rom: &AtariRom, address: i32) -> Self {
        Self {
            attributes: AttributesRef::from_address(rom.u32(address)),
            name: rom.c_string(rom.u32(address + 4)),
            id: rom.u16(address + 8),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonRef {
    Team1(usize),
    Team2(usize),
    Temp,
}

/// A league team: names and the skill template for each formation group.
#[derive(Clone, Debug)]
pub struct Team {
    pub full_name: Text,
    pub short_name: Text,
    defence: [i32; 8],
    midfield: [i32; 8],
    attack: [i32; 8],
    substitutes: [i32; 8],
}

impl Team {
    pub const SIZE: i32 = 0x40;

    pub fn read(rom: &AtariRom, address: i32) -> Self {
        // Played/won/drawn/lost/points and league placing (24 bytes) are
        // league bookkeeping the match engine never reads.
        let group = |offset: i32| std::array::from_fn(|i| rom.u8(address + offset + i as i32));
        Self {
            full_name: Text::read(rom, rom.u32(address)),
            short_name: Text::read(rom, rom.u32(address + 4)),
            defence: group(24),
            midfield: group(32),
            attack: group(40),
            substitutes: group(48),
        }
    }

    /// The skill templates for defence, midfield, attack and substitutes.
    pub fn groups(&self) -> [[i32; 8]; 4] {
        [self.defence, self.midfield, self.attack, self.substitutes]
    }

    /// Copies the group templates into a 12-player attribute table: three
    /// defenders, three midfielders, three attackers, three substitutes
    /// (`sub_903E`).
    pub fn apply_skills_to(&self, attributes: &mut [Attributes]) {
        let groups = [self.defence, self.midfield, self.attack, self.substitutes];
        for (chunk, group) in attributes.chunks_mut(3).zip(groups) {
            for member in chunk {
                member.skills = Skills::from_array(group);
            }
        }
    }
}

/// Every attribute table and person record a match can touch.
#[derive(Clone, Debug)]
pub struct Roster {
    pub attributes_team1: Vec<Attributes>,
    pub attributes_team2: Vec<Attributes>,
    pub attributes_temp: Attributes,
    pub attributes_market: Vec<Attributes>,
    pub persons_team1: Vec<Person>,
    pub persons_team2: Vec<Person>,
    pub person_temp: Person,
}

impl Roster {
    pub const SQUAD_SIZE: usize = 12;
    const MARKET_SIZE: usize = 18;

    pub fn read(rom: &AtariRom) -> Self {
        let attributes = |base: i32, count: usize| -> Vec<Attributes> {
            (0..count as i32)
                .map(|i| Attributes::read(rom, base + i * Attributes::SIZE))
                .collect()
        };
        let persons = |base: i32| -> Vec<Person> {
            (0..Self::SQUAD_SIZE as i32)
                .map(|i| Person::read(rom, base + i * Person::SIZE))
                .collect()
        };
        Self {
            attributes_team1: attributes(addr::ATTRIBUTES_TEAM1, Self::SQUAD_SIZE),
            attributes_team2: attributes(addr::ATTRIBUTES_TEAM2, Self::SQUAD_SIZE),
            attributes_temp: Attributes::read(rom, addr::ATTRIBUTES_TEMP),
            attributes_market: attributes(addr::ATTRIBUTES_MARKET, Self::MARKET_SIZE),
            persons_team1: persons(addr::PERSONS_TEAM1),
            persons_team2: persons(addr::PERSONS_TEAM2),
            person_temp: Person::read(rom, addr::PERSON_TEMP),
        }
    }

    pub fn attributes(&self, r: AttributesRef) -> &Attributes {
        match r.table {
            AttributesTable::Team1 => &self.attributes_team1[r.index],
            AttributesTable::Team2 => &self.attributes_team2[r.index],
            AttributesTable::Temp => &self.attributes_temp,
            AttributesTable::Market => &self.attributes_market[r.index],
        }
    }

    pub fn attributes_mut(&mut self, r: AttributesRef) -> &mut Attributes {
        match r.table {
            AttributesTable::Team1 => &mut self.attributes_team1[r.index],
            AttributesTable::Team2 => &mut self.attributes_team2[r.index],
            AttributesTable::Temp => &mut self.attributes_temp,
            AttributesTable::Market => &mut self.attributes_market[r.index],
        }
    }

    pub fn person(&self, r: PersonRef) -> &Person {
        match r {
            PersonRef::Team1(i) => &self.persons_team1[i],
            PersonRef::Team2(i) => &self.persons_team2[i],
            PersonRef::Temp => &self.person_temp,
        }
    }

    fn person_mut(&mut self, r: PersonRef) -> &mut Person {
        match r {
            PersonRef::Team1(i) => &mut self.persons_team1[i],
            PersonRef::Team2(i) => &mut self.persons_team2[i],
            PersonRef::Temp => &mut self.person_temp,
        }
    }

    /// Copies identity and skills from one squad slot to another; each slot
    /// keeps pointing at its own attribute record (`CopySubstituteTo`).
    pub fn copy_person(&mut self, from: PersonRef, to: PersonRef) {
        let source = self.person(from).clone();
        let skills = self.attributes(source.attributes).skills;
        let target = self.person_mut(to);
        target.id = source.id;
        target.name = source.name;
        let target_attributes = target.attributes;
        self.attributes_mut(target_attributes).skills = skills;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_pointers_resolve_to_tables() {
        let r = AttributesRef::from_address(addr::ATTRIBUTES_TEAM2 + 2 * Attributes::SIZE);
        assert_eq!(
            r,
            AttributesRef {
                table: AttributesTable::Team2,
                index: 2
            }
        );
        let r = AttributesRef::from_address(addr::ATTRIBUTES_TEMP);
        assert_eq!(r.table, AttributesTable::Temp);
    }

    #[test]
    fn skills_round_down() {
        let mut attributes = Attributes {
            skills: Skills::uniform(187),
            ..Attributes::default()
        };
        attributes.round_skills_down();
        assert_eq!(attributes.skills, Skills::uniform(180));
    }
}
