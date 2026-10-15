//! Paired devices of a shop hub (multi-device phase 1b). Pairing itself is
//! authorized by the one-time code the hub shows (the hub checks it before
//! calling `pair_device`); revoking needs the Settings permission.
use crate::{
    Error, Pos, Result,
    audit::ActivityKind,
    schema::{devices, registers},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    /// A second register with its own drawer.
    Register,
    /// A kitchen screen.
    Kitchen,
    /// An ordering tablet (no payment by default).
    Ordering,
}
impl DeviceKind {
    fn key(self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Kitchen => "kitchen",
            Self::Ordering => "ordering",
        }
    }
    fn parse(key: &str) -> Result<Self> {
        match key {
            "register" => Ok(Self::Register),
            "kitchen" => Ok(Self::Kitchen),
            "ordering" => Ok(Self::Ordering),
            _ => Err(Error::CorruptData),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub kind: DeviceKind,
    pub register_id: Uuid,
    pub paired_at: i64,
    pub revoked_at: Option<i64>,
    pub last_seen_at: Option<i64>,
}

type Row = (
    String,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
);
fn device(row: Row) -> Result<Device> {
    let (id, name, kind, register, paired_at, revoked_at, last_seen_at) = row;
    Ok(Device {
        id: Uuid::parse_str(&id).map_err(|_| Error::CorruptData)?,
        name,
        kind: DeviceKind::parse(&kind)?,
        register_id: Uuid::parse_str(&register).map_err(|_| Error::CorruptData)?,
        paired_at,
        revoked_at,
        last_seen_at,
    })
}
fn columns() -> (
    devices::id,
    devices::name,
    devices::kind,
    devices::register_id,
    devices::paired_at,
    devices::revoked_at,
    devices::last_seen_at,
) {
    (
        devices::id,
        devices::name,
        devices::kind,
        devices::register_id,
        devices::paired_at,
        devices::revoked_at,
        devices::last_seen_at,
    )
}

impl Pos {
    /// Every device ever paired, newest first.
    pub fn devices(&mut self) -> Result<Vec<Device>> {
        devices::table
            .order((devices::paired_at.desc(), devices::id))
            .select(columns())
            .load::<Row>(&mut self.conn)?
            .into_iter()
            .map(device)
            .collect()
    }
    /// Records a newly paired device. The caller has verified the hub's
    /// one-time pairing code; `token_hash` is the SHA-256 (hex) of the
    /// device's secret token. A register device gets its own register (same
    /// ID as the device, named after it); kitchen and ordering devices use
    /// the main register. Re-pairing a device ID replaces its token.
    pub fn pair_device(
        &mut self,
        id: Uuid,
        name: &str,
        kind: DeviceKind,
        token_hash: &str,
        now: i64,
    ) -> Result<Device> {
        let register = match kind {
            DeviceKind::Register => id,
            DeviceKind::Kitchen | DeviceKind::Ordering => crate::MAIN_REGISTER,
        };
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || token_hash.len() != 64 {
            return Err(Error::InvalidDevice);
        }
        self.conn.immediate_transaction(|conn| {
            let open = registers::table
                .find(register.to_string())
                .select(registers::retired_at)
                .first::<Option<i64>>(conn)
                .optional()?;
            match (open, kind) {
                (Some(None), _) => {}
                (None, DeviceKind::Register) => {
                    diesel::insert_into(registers::table)
                        .values((
                            registers::id.eq(register.to_string()),
                            registers::name.eq(name),
                            registers::created_at.eq(now),
                        ))
                        .execute(conn)?;
                }
                _ => return Err(Error::RegisterNotFound),
            }
            diesel::replace_into(devices::table)
                .values((
                    devices::id.eq(id.to_string()),
                    devices::name.eq(name),
                    devices::kind.eq(kind.key()),
                    devices::register_id.eq(register.to_string()),
                    devices::token_hash.eq(token_hash),
                    devices::paired_at.eq(now),
                    devices::revoked_at.eq(None::<i64>),
                    devices::last_seen_at.eq(None::<i64>),
                ))
                .execute(conn)?;
            crate::audit::record(
                conn,
                ActivityKind::Device,
                &id.to_string(),
                now,
                &pos_domain::Attribution::default(),
                &serde_json::json!({ "action": "paired", "name": name, "kind": kind.key() }),
            )?;
            Ok(Device {
                id,
                name: name.to_owned(),
                kind,
                register_id: register,
                paired_at: now,
                revoked_at: None,
                last_seen_at: None,
            })
        })
    }
    /// Stops a device from connecting (Settings permission).
    pub fn revoke_device(&mut self, id: Uuid, now: i64) -> Result<()> {
        let session = self.session.clone();
        self.conn.immediate_transaction(|conn| {
            let staff =
                crate::staff::authorize(conn, &session, Some(pos_domain::Permission::Settings))?;
            let name = devices::table
                .find(id.to_string())
                .select(devices::name)
                .first::<String>(conn)
                .optional()?
                .ok_or(Error::DeviceNotFound)?;
            diesel::update(devices::table.find(id.to_string()))
                .set(devices::revoked_at.eq(Some(now)))
                .execute(conn)?;
            crate::audit::record(
                conn,
                ActivityKind::Device,
                &id.to_string(),
                now,
                &staff,
                &serde_json::json!({ "action": "revoked", "name": name }),
            )?;
            Ok(())
        })
    }
    /// The register of an active device whose token hashes to `token_hash`;
    /// records when it was last seen.
    pub fn device_login(&mut self, id: Uuid, token_hash: &str, now: i64) -> Result<Option<Uuid>> {
        let row = devices::table
            .find(id.to_string())
            .filter(devices::revoked_at.is_null())
            .select((devices::token_hash, devices::register_id))
            .first::<(String, String)>(&mut self.conn)
            .optional()?;
        let Some((stored, register)) = row else {
            return Ok(None);
        };
        // Compare without an early exit on the first differing byte.
        if stored.len() != token_hash.len()
            || stored
                .bytes()
                .zip(token_hash.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                != 0
        {
            return Ok(None);
        }
        diesel::update(devices::table.find(id.to_string()))
            .set(devices::last_seen_at.eq(Some(now)))
            .execute(&mut self.conn)?;
        Ok(Some(
            Uuid::parse_str(&register).map_err(|_| Error::CorruptData)?,
        ))
    }
}
