//! Product options (modifiers) such as sweetness or toppings. A choice may add
//! to the unit price; checkout prices options from the catalog, never the UI.
use crate::{DomainError, Money};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_GROUPS: usize = 10;
pub const MAX_CHOICES: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroup {
    pub id: Uuid,
    pub name: String,
    /// At least one choice must be picked.
    pub required: bool,
    /// More than one choice may be picked.
    pub multiple: bool,
    pub choices: Vec<OptionChoice>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionChoice {
    pub id: Uuid,
    pub name: String,
    /// Added to the product price for each unit. Zero for free choices.
    pub price: Money,
}
/// A choice selected on a cart line, by stable IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineOption {
    pub group_id: Uuid,
    pub choice_id: Uuid,
}
/// Immutable receipt snapshot of a chosen option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaleOption {
    pub group: String,
    pub name: String,
    pub price: Money,
}

fn valid_name(name: &str) -> bool {
    let n = name.trim().chars().count();
    (1..=80).contains(&n)
}
pub fn validate_groups(groups: &[OptionGroup]) -> Result<(), DomainError> {
    if groups.len() > MAX_GROUPS {
        return Err(DomainError::InvalidOptions);
    }
    let mut ids = std::collections::HashSet::new();
    for g in groups {
        if !valid_name(&g.name)
            || g.choices.is_empty()
            || g.choices.len() > MAX_CHOICES
            || !ids.insert(g.id)
        {
            return Err(DomainError::InvalidOptions);
        }
        for c in &g.choices {
            if !valid_name(&c.name) || !ids.insert(c.id) {
                return Err(DomainError::InvalidOptions);
            }
        }
    }
    Ok(())
}
/// Unit price including chosen options, plus their receipt snapshot (in group order).
/// Refuses unknown or duplicate choices, several picks in a single-choice group,
/// and missing required groups.
pub fn price_with_options(
    base: Money,
    groups: &[OptionGroup],
    chosen: &[LineOption],
) -> Result<(Money, Vec<SaleOption>), DomainError> {
    let mut seen = std::collections::HashSet::new();
    for c in chosen {
        if !seen.insert(c.choice_id) {
            return Err(DomainError::InvalidOptions);
        }
        let known = groups
            .iter()
            .find(|g| g.id == c.group_id)
            .is_some_and(|g| g.choices.iter().any(|x| x.id == c.choice_id));
        if !known {
            return Err(DomainError::InvalidOptions);
        }
    }
    let mut unit = base;
    let mut snapshot = Vec::new();
    for g in groups {
        let picked: Vec<&OptionChoice> = g
            .choices
            .iter()
            .filter(|x| {
                chosen
                    .iter()
                    .any(|c| c.group_id == g.id && c.choice_id == x.id)
            })
            .collect();
        if (g.required && picked.is_empty()) || (!g.multiple && picked.len() > 1) {
            return Err(DomainError::InvalidOptions);
        }
        for choice in picked {
            unit = unit.checked_add(choice.price)?;
            snapshot.push(SaleOption {
                group: g.name.clone(),
                name: choice.name.clone(),
                price: choice.price,
            });
        }
    }
    Ok((unit, snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn money(s: &str) -> Money {
        s.parse().unwrap_or_default()
    }
    fn groups() -> Vec<OptionGroup> {
        vec![
            OptionGroup {
                id: Uuid::from_u128(1),
                name: "ความหวาน".into(),
                required: true,
                multiple: false,
                choices: vec![
                    OptionChoice {
                        id: Uuid::from_u128(11),
                        name: "ปกติ".into(),
                        price: Money::ZERO,
                    },
                    OptionChoice {
                        id: Uuid::from_u128(12),
                        name: "หวานน้อย".into(),
                        price: Money::ZERO,
                    },
                ],
            },
            OptionGroup {
                id: Uuid::from_u128(2),
                name: "ท็อปปิ้ง".into(),
                required: false,
                multiple: true,
                choices: vec![
                    OptionChoice {
                        id: Uuid::from_u128(21),
                        name: "ไข่มุก".into(),
                        price: money("10"),
                    },
                    OptionChoice {
                        id: Uuid::from_u128(22),
                        name: "วิปครีม".into(),
                        price: money("15.50"),
                    },
                ],
            },
        ]
    }
    fn pick(g: u128, c: u128) -> LineOption {
        LineOption {
            group_id: Uuid::from_u128(g),
            choice_id: Uuid::from_u128(c),
        }
    }
    #[test]
    fn options_add_exact_prices_in_group_order() -> Result<(), DomainError> {
        let (unit, snap) = price_with_options(
            money("45"),
            &groups(),
            &[pick(2, 22), pick(1, 12), pick(2, 21)],
        )?;
        assert_eq!(unit, money("70.50"));
        let names: Vec<_> = snap.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["หวานน้อย", "ไข่มุก", "วิปครีม"]);
        Ok(())
    }
    #[test]
    fn invalid_selections_are_refused() {
        let g = groups();
        let base = money("45");
        for chosen in [
            vec![],                         // required group missing
            vec![pick(1, 11), pick(1, 12)], // two in a single-choice group
            vec![pick(1, 11), pick(1, 11)], // duplicate
            vec![pick(1, 11), pick(2, 11)], // choice from another group
            vec![pick(1, 11), pick(9, 99)], // unknown
        ] {
            assert_eq!(
                price_with_options(base, &g, &chosen),
                Err(DomainError::InvalidOptions),
                "{chosen:?}"
            );
        }
        assert!(price_with_options(base, &[], &[]).is_ok());
    }
    #[test]
    fn group_definitions_are_validated() {
        let mut g = groups();
        assert!(validate_groups(&g).is_ok());
        g[1].choices.clear();
        assert_eq!(validate_groups(&g), Err(DomainError::InvalidOptions));
        let mut g = groups();
        g[0].name = "  ".into();
        assert_eq!(validate_groups(&g), Err(DomainError::InvalidOptions));
        let mut g = groups();
        g[1].choices[0].id = g[0].choices[0].id;
        assert_eq!(validate_groups(&g), Err(DomainError::InvalidOptions));
    }
}
