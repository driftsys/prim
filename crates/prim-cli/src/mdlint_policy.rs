//! Resolve prim's Markdown lint policy for one file: which tier applies, and
//! which rules that path excludes.
//!
//! Lives in the CLI crate because it reads `.editorconfig`; `prim-fmt` takes
//! the resolved policy as data and stays pure.

use std::collections::HashSet;
use std::path::Path;

use ec4rs::Properties;

use crate::editorconfig::{self, Resolver};
use crate::provenance::{self, SettingOrigin};
use crate::ui;

pub(crate) const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";
pub(crate) const MDLINT_ENABLE_KEY: &str = "prim_mdlint_enable";
pub(crate) const MDLINT_DISABLE_KEY: &str = "prim_mdlint_disable";

/// Read `prim_mdlint_strict` out of already-resolved properties. Unset or
/// non-`true` values mean the floor tier.
pub(crate) fn strict_from(props: &Properties) -> bool {
    editorconfig::prim_bool_from(props, MDLINT_STRICT_KEY).unwrap_or(false)
}

/// The Markdown lint policy for one file: the tier that applies, and the rules
/// that path excludes from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MdLintPolicy {
    /// The rules prim runs for this path, ready to hand to the engine.
    pub selection: prim_fmt::MdLintSelection,
    /// Ids `prim_mdlint_enable` or `prim_mdlint_disable` listed that prim
    /// will not act on, in the order written. Resolving a policy never
    /// reports these itself — see [`RejectedRuleReporter`] for where and how
    /// often that happens.
    pub rejected: Vec<RejectedRuleId>,
}

/// An id a `prim_mdlint_*` key listed that prim will not act on, with the key
/// that listed it, why it was refused, and where it was written.
///
/// The origin is carried per id because two keys can each carry rejects, and a
/// message that names the wrong line sends the reader to a line with nothing
/// to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRuleId {
    /// The id as written, uppercased.
    pub id: String,
    /// The `.editorconfig` key that listed it.
    pub key: &'static str,
    /// Why prim refused it. Never [`prim_fmt::RuleReach::Selectable`].
    pub reach: prim_fmt::RuleReach,
    /// The `.editorconfig` file, line and section that set `key`.
    pub origin: SettingOrigin,
}

/// Parse one comma-separated rule-id list out of already-resolved properties.
///
/// Entries are trimmed and uppercased, then split into ids prim can select
/// (kept) and ids it refuses (returned separately, dropped from the list
/// either way). This stays pure — reporting a refusal is a caller's job, so a
/// warning fires once per run per section rather than once per file (see
/// [`RejectedRuleReporter`]).
///
/// Returns `(accepted, rejected)`. The rejects carry a placeholder origin;
/// [`attribute`] fills it in only when there is something to report, because
/// recovering the section header re-reads the `.editorconfig`.
fn rule_ids_from(props: &Properties, key: &'static str) -> (Vec<String>, Vec<RejectedRuleId>) {
    let Some(raw) = props.get_raw_for_key(key).into_option() else {
        return (Vec::new(), Vec::new());
    };

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        // `unset` is EditorConfig's own reserved word for "clear the inherited
        // value"; `none` is prim's own spelling of the same intent. Neither
        // names a rule, so neither is a refusal to report.
        if entry.eq_ignore_ascii_case("unset") || entry.eq_ignore_ascii_case("none") {
            continue;
        }
        let reach = prim_fmt::rule_reach(entry);
        if reach == prim_fmt::RuleReach::Selectable {
            accepted.push(entry.to_ascii_uppercase());
        } else {
            rejected.push(RejectedRuleId {
                id: entry.to_ascii_uppercase(),
                key,
                reach,
                origin: SettingOrigin::Default,
            });
        }
    }
    (accepted, rejected)
}

/// Resolve `key`'s `.editorconfig` origin once and stamp it on every id that
/// key rejected.
fn attribute(rejected: &mut [RejectedRuleId], props: &Properties, key: &str) {
    if rejected.is_empty() {
        return;
    }
    let origin = provenance::origin_of(props, key);
    for reject in rejected {
        reject.origin = origin.clone();
    }
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve(path: &Path) -> MdLintPolicy {
    Resolver::new().resolve_mdlint_policy(path)
}

/// Assemble the whole Markdown lint policy from one file's resolved
/// `.editorconfig` properties. Pure — emits no warnings; see
/// [`RejectedRuleReporter`] for reporting `rejected` at the point of use.
pub(crate) fn policy_from(props: &Properties) -> MdLintPolicy {
    let (enabled, mut rejected) = rule_ids_from(props, MDLINT_ENABLE_KEY);
    attribute(&mut rejected, props, MDLINT_ENABLE_KEY);
    let (disabled, mut disable_rejects) = rule_ids_from(props, MDLINT_DISABLE_KEY);
    attribute(&mut disable_rejects, props, MDLINT_DISABLE_KEY);
    rejected.append(&mut disable_rejects);

    MdLintPolicy {
        selection: prim_fmt::MdLintSelection {
            strict: strict_from(props),
            enabled,
            disabled,
        },
        rejected,
    }
}

/// Warns about each refused `prim_mdlint_enable` or `prim_mdlint_disable` id
/// once per run for each `.editorconfig` section that carries it, rather than
/// once per file a matching glob happens to cover. A refused rule id is a
/// mistake in one `.editorconfig` line; it deserves one line of stderr
/// output, not a repeat for every file that inherits it — and two sections
/// carrying the same refused id are two mistakes to fix, not one.
#[derive(Default)]
pub struct RejectedRuleReporter {
    /// Already-warned `(key, location, id)` triples. Two keys cannot set the
    /// same `.editorconfig` line, so `location` alone already tells apart an
    /// id refused by both keys; `key` is part of the identity only for the
    /// case `location` cannot cover — a [`SettingOrigin::Default`] origin,
    /// where every id shares the same empty location regardless of which key
    /// refused it.
    reported: HashSet<(&'static str, String, String)>,
}

impl RejectedRuleReporter {
    /// A reporter that has not yet warned about anything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Warn about each refused id in `policy`, attributed to the
    /// `.editorconfig` file, line and section that set the key — that is
    /// where it has to be fixed. Ids this reporter already warned about for
    /// the same key and location are skipped.
    pub fn report(&mut self, policy: &MdLintPolicy) {
        for reject in &policy.rejected {
            let location = provenance::location_of(&reject.origin);
            if !self
                .reported
                .insert((reject.key, location.clone(), reject.id.clone()))
            {
                continue;
            }
            let attribution = if location.is_empty() {
                String::new()
            } else {
                format!("{location}: ")
            };
            let reason = match reject.reach {
                prim_fmt::RuleReach::Withheld => "which prim does not run at any tier",
                prim_fmt::RuleReach::Unknown => "which is not a rule prim knows",
                prim_fmt::RuleReach::Selectable => {
                    unreachable!("a RejectedRuleId's reach is never Selectable")
                }
            };
            ui::warning(&format!(
                "{attribution}{} lists '{}', {reason} — ignoring it",
                reject.key, reject.id
            ));
        }
    }
}

#[cfg(test)]
mod tests;
