//! Append-only activity snapshots. Insertions share the business write transaction.
use crate::{Pos, Result, schema::activity_events};
use diesel::prelude::*;
use pos_domain::Attribution;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    Product,
    Discount,
    Refund,
    Void,
    Stock,
    Staff,
    Role,
    /// Dishes cancelled after the kitchen had them.
    Kitchen,
}
impl ActivityKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Discount => "discount",
            Self::Refund => "refund",
            Self::Void => "void",
            Self::Stock => "stock",
            Self::Staff => "staff",
            Self::Role => "role",
            Self::Kitchen => "kitchen",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActivityEvent {
    pub number: i64,
    pub created_at: i64,
    pub kind: ActivityKind,
    pub entity_id: String,
    pub staff: Attribution,
    /// Business snapshots only. Never contains PINs or PIN hashes.
    pub detail: serde_json::Value,
}
#[derive(Clone, Debug, Default)]
pub struct ActivityQuery {
    pub kind: Option<ActivityKind>,
    pub search: String,
    pub offset: u32,
}
pub(crate) fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
pub(crate) fn record(
    conn: &mut SqliteConnection,
    kind: ActivityKind,
    entity_id: &str,
    created_at: i64,
    staff: &Attribution,
    detail: &serde_json::Value,
) -> Result<()> {
    let event = ActivityEvent {
        number: 0,
        created_at,
        kind,
        entity_id: entity_id.into(),
        staff: staff.clone(),
        detail: detail.clone(),
    };
    diesel::insert_into(activity_events::table)
        .values((
            activity_events::created_at.eq(created_at),
            activity_events::kind.eq(kind.key()),
            activity_events::entity_id.eq(entity_id),
            activity_events::event_json.eq(serde_json::to_string(&event)?),
        ))
        .execute(conn)?;
    Ok(())
}
impl Pos {
    /// Newest first, 50 per page. Search treats SQL wildcard characters literally.
    /// Pre-upgrade events are not fabricated from today's product/staff values.
    pub fn activity(&mut self, query: &ActivityQuery) -> Result<Vec<ActivityEvent>> {
        let mut rows = activity_events::table.into_boxed();
        if let Some(kind) = query.kind {
            rows = rows.filter(activity_events::kind.eq(kind.key()));
        }
        let search = query.search.trim();
        if !search.is_empty() {
            let pattern = format!(
                "%{}%",
                search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            rows = rows.filter(activity_events::event_json.like(pattern).escape('\\'));
        }
        rows.order(activity_events::number.desc())
            .limit(50)
            .offset(i64::from(query.offset))
            .select((activity_events::number, activity_events::event_json))
            .load::<(i64, String)>(&mut self.conn)?
            .into_iter()
            .map(|(number, json)| {
                let mut event: ActivityEvent = serde_json::from_str(&json)?;
                event.number = number;
                Ok(event)
            })
            .collect()
    }
}
