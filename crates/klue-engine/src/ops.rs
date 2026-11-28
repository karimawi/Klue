//! Changes to the lines, the Home tabs and the settings Klue Server keeps, as values. The app
//! applies one to its own data, and when a server is connected sends it there to be applied
//! the same way, so the two copies stay alike.
//!
//! Settings that belong to one computer (theme, language, time format, launch at startup,
//! the tray) aren't ops: they change that computer's copy only.

use klue_core::{
    Carrier,
    store::{AlertDefaults, AlertPrefs, AppData, Card, RefreshEvery},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// An empty nickname clears it.
    Rename { id: String, nickname: String },
    /// Archives a number (it keeps its password but isn't checked or listed), or brings it
    /// back.
    SetArchived { id: String, archived: bool },
    /// `None` follows the interval for all numbers.
    SetRefresh { id: String, every: Option<RefreshEvery> },
    /// `None` goes back to the default main quota.
    SetMainBundle { id: String, bundle: Option<String> },
    /// A number's alerts. New thresholds re-arm alerts that already went off.
    SetAlerts { id: String, alerts: AlertPrefs },
    /// A number's renewal reminder, on or off.
    SetReminder { id: String, on: bool },
    RemoveAccount { id: String },
    /// Puts `carrier` where `onto` is in the rail.
    MoveCarrier { carrier: Carrier, onto: Carrier },
    /// Puts number `id` where number `onto` is among its carrier's tabs.
    MoveAccount { id: String, onto: String },
    /// How often numbers refresh unless they have their own interval.
    SetRefreshAll { every: RefreshEvery },
    /// The alert settings for all numbers.
    SetAlertDefaults { alerts: AlertDefaults },
    /// Gives every number the alert settings for all numbers, with the share shown in
    /// Settings.
    ApplyAlertDefaults,
    /// A Home tab, with an id made by whoever adds it.
    AddGroup { id: String, name: Option<String> },
    /// An empty name goes back to the default one.
    RenameGroup { id: String, name: String },
    RemoveGroup { id: String },
    MoveGroup { id: String, onto: String },
    /// Puts a card on a Home tab, or takes it off.
    SetCard { group: String, card: Card, on: bool },
    MoveCard { group: String, card: Card, onto: Card },
}

impl Op {
    /// Whether it changes what the password store's index lists: the numbers, their names,
    /// their order and whether they're archived.
    pub fn touches_index(&self) -> bool {
        matches!(self, Op::Rename { .. } | Op::SetArchived { .. } | Op::RemoveAccount { .. } | Op::MoveAccount { .. })
    }
}

pub fn apply(data: &mut AppData, op: &Op) {
    match op {
        Op::Rename { id, nickname } => {
            if let Some(a) = data.account_mut(id) {
                a.nickname = Some(nickname.trim().to_string()).filter(|n| !n.is_empty());
            }
        }
        Op::SetArchived { id, archived } => {
            if let Some(a) = data.account_mut(id) {
                a.archived = *archived;
            }
        }
        Op::SetRefresh { id, every } => {
            if let Some(a) = data.account_mut(id) {
                a.refresh = *every;
            }
        }
        Op::SetMainBundle { id, bundle } => {
            if let Some(a) = data.account_mut(id) {
                a.main_bundle = bundle.clone();
            }
        }
        Op::SetAlerts { id, alerts } => {
            if let Some(a) = data.account_mut(id) {
                a.alerts = alerts.clone();
                a.fired.clear();
            }
        }
        Op::SetReminder { id, on } => {
            if let Some(a) = data.account_mut(id) {
                a.alerts.renewal.enabled = *on;
            }
        }
        Op::RemoveAccount { id } => data.remove_account(id),
        Op::MoveCarrier { carrier, onto } => data.move_carrier(*carrier, *onto),
        Op::MoveAccount { id, onto } => data.move_account(id, onto),
        Op::SetRefreshAll { every } => data.settings.refresh = *every,
        Op::SetAlertDefaults { alerts } => data.settings.alerts = *alerts,
        Op::ApplyAlertDefaults => {
            data.settings.alerts.percent = Some(data.settings.alerts.percent_or_default());
            data.apply_alert_defaults();
        }
        Op::AddGroup { id, name } => data.insert_group(id.clone(), name.clone()),
        Op::RenameGroup { id, name } => data.rename_group(id, name),
        Op::RemoveGroup { id } => data.remove_group(id),
        Op::MoveGroup { id, onto } => data.move_group(id, onto),
        Op::SetCard { group, card, on } => data.set_card(group, card.clone(), *on),
        Op::MoveCard { group, card, onto } => data.move_card(group, card, onto),
    }
}

#[cfg(test)]
mod tests {
    use klue_core::{LineType, store::Account};

    use super::*;

    #[test]
    fn ops_survive_the_trip_to_a_server_and_apply_alike() {
        let mut here = AppData::default();
        let account = Account::new(Carrier::We, LineType::Landline, "0223584417".into());
        let id = account.id.clone();
        here.accounts.push(account);
        let mut there = here.clone();

        let ops = vec![
            Op::Rename { id: id.clone(), nickname: "  Home internet ".into() },
            Op::SetRefresh { id: id.clone(), every: Some(RefreshEvery::Hours3) },
            Op::AddGroup { id: "family".into(), name: Some("Family".into()) },
            Op::SetCard { group: "family".into(), card: Card::main(&id), on: true },
            Op::SetReminder { id: id.clone(), on: false },
        ];
        for op in &ops {
            apply(&mut here, op);
            let sent: Op = serde_json::from_str(&serde_json::to_string(op).unwrap()).unwrap();
            apply(&mut there, &sent);
        }
        assert_eq!(here, there);
        assert_eq!(here.accounts[0].nickname.as_deref(), Some("Home internet"));
        assert_eq!(here.group_line_count("family"), 1);
        assert!(serde_json::to_string(&ops[0]).unwrap().starts_with(r#"{"op":"rename""#));
    }
}
