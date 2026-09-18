use std::fmt;
use std::str::FromStr;

use dioxus::prelude::*;

use crate::layout::RootLayout;
use crate::pages::entity_detail::EntityDetailPage;
use crate::pages::home::HomePage;

/// The kinds of reference entities the app knows about.
/// Declaration order must match [`KINDS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Bestiary,
    Spells,
    Items,
    Classes,
    Subclasses,
    Races,
    Backgrounds,
    Feats,
    Conditions,
    Objects,
    Actions,
    Hazards,
    Deities,
    Vehicles,
    VariantRules,
    OptionalFeatures,
    Rewards,
    Psionics,
    Recipes,
    CultsBoons,
    Languages,
    CharOptions,
    Facilities,
    ItemProperties,
    ItemMasteries,
    Skills,
    Senses,
    Tables,
    Decks,
    Cards,
    Encounters,
    LegendaryGroups,
    CrochetPatterns,
    Rules,
}

/// Per-kind presentation: the route/persistence slug, the label shown in
/// the kind tag (and matched by search), and the tag's colors. The classes
/// are literal Tailwind classes (not built from a format string) so the
/// Tailwind JIT scanner - which only sees complete class names in the
/// source text - generates the CSS for each of them.
struct KindInfo {
    slug: &'static str,
    label: &'static str,
    badge: &'static str,
}

const fn info(slug: &'static str, label: &'static str, badge: &'static str) -> KindInfo {
    KindInfo { slug, label, badge }
}

const KINDS: [(EntityKind, KindInfo); 34] = [
    (
        EntityKind::Bestiary,
        info("bestiary", "Bestiary", "bg-red-50 text-red-700"),
    ),
    (
        EntityKind::Spells,
        info("spells", "Spells", "bg-purple-50 text-purple-700"),
    ),
    (EntityKind::Items, info("items", "Items", "bg-amber-50 text-amber-700")),
    (
        EntityKind::Classes,
        info("classes", "Classes", "bg-blue-50 text-blue-700"),
    ),
    (
        EntityKind::Subclasses,
        info("subclasses", "Subclasses", "bg-sky-50 text-sky-700"),
    ),
    (EntityKind::Races, info("races", "Races", "bg-green-50 text-green-700")),
    (
        EntityKind::Backgrounds,
        info("backgrounds", "Backgrounds", "bg-teal-50 text-teal-700"),
    ),
    (EntityKind::Feats, info("feats", "Feats", "bg-pink-50 text-pink-700")),
    (
        EntityKind::Conditions,
        info("conditions", "Conditions & Diseases", "bg-orange-50 text-orange-700"),
    ),
    (
        EntityKind::Objects,
        info("objects", "Objects", "bg-stone-100 text-stone-700"),
    ),
    (
        EntityKind::Actions,
        info("actions", "Actions", "bg-cyan-50 text-cyan-700"),
    ),
    (
        EntityKind::Hazards,
        info("hazards", "Hazards", "bg-rose-50 text-rose-700"),
    ),
    (
        EntityKind::Deities,
        info("deities", "Deities", "bg-yellow-50 text-yellow-700"),
    ),
    (
        EntityKind::Vehicles,
        info("vehicles", "Vehicles", "bg-lime-50 text-lime-700"),
    ),
    (
        EntityKind::VariantRules,
        info("variantrules", "Variant Rules", "bg-emerald-50 text-emerald-700"),
    ),
    (
        EntityKind::OptionalFeatures,
        info(
            "optionalfeatures",
            "Optional Features",
            "bg-fuchsia-50 text-fuchsia-700",
        ),
    ),
    (
        EntityKind::Rewards,
        info("rewards", "Rewards", "bg-violet-50 text-violet-700"),
    ),
    (
        EntityKind::Psionics,
        info("psionics", "Psionics", "bg-indigo-50 text-indigo-700"),
    ),
    (
        EntityKind::Recipes,
        info("recipes", "Recipes", "bg-orange-100 text-orange-800"),
    ),
    (
        EntityKind::CultsBoons,
        info("cultsboons", "Cults & Boons", "bg-red-100 text-red-800"),
    ),
    (
        EntityKind::Languages,
        info("languages", "Languages", "bg-blue-100 text-blue-800"),
    ),
    (
        EntityKind::CharOptions,
        info("charoptions", "Character Options", "bg-green-100 text-green-800"),
    ),
    (
        EntityKind::Facilities,
        info("facilities", "Bastion Facilities", "bg-teal-100 text-teal-800"),
    ),
    (
        EntityKind::ItemProperties,
        info("itemproperties", "Item Properties", "bg-amber-100 text-amber-800"),
    ),
    (
        EntityKind::ItemMasteries,
        info("itemmasteries", "Item Masteries", "bg-yellow-100 text-yellow-800"),
    ),
    (
        EntityKind::Skills,
        info("skills", "Skills", "bg-cyan-100 text-cyan-800"),
    ),
    (EntityKind::Senses, info("senses", "Senses", "bg-sky-100 text-sky-800")),
    (
        EntityKind::Tables,
        info("tables", "Tables", "bg-gray-100 text-gray-700"),
    ),
    (
        EntityKind::Decks,
        info("decks", "Decks", "bg-purple-100 text-purple-800"),
    ),
    (EntityKind::Cards, info("cards", "Cards", "bg-pink-100 text-pink-800")),
    (
        EntityKind::Encounters,
        info("encounters", "Encounters", "bg-rose-100 text-rose-800"),
    ),
    (
        EntityKind::LegendaryGroups,
        info("legendarygroups", "Legendary Groups", "bg-red-50 text-red-900"),
    ),
    (
        EntityKind::CrochetPatterns,
        info("crochet", "Crochet Patterns", "bg-slate-100 text-slate-600"),
    ),
    (
        EntityKind::Rules,
        info("rules", "Book Rules", "bg-emerald-100 text-emerald-800"),
    ),
];

impl EntityKind {
    pub fn all() -> impl Iterator<Item = EntityKind> {
        KINDS.iter().map(|(kind, _)| *kind)
    }

    fn info(&self) -> &'static KindInfo {
        &KINDS[*self as usize].1
    }

    pub fn label(&self) -> &'static str {
        self.info().label
    }

    /// A distinct tag color per kind so the kind tag is
    /// scannable at a glance across a grid mixing many kinds.
    pub fn badge_classes(&self) -> &'static str {
        self.info().badge
    }

    pub fn slug(&self) -> &'static str {
        self.info().slug
    }
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

#[derive(Debug, Clone)]
pub struct UnknownEntityKind(String);

impl fmt::Display for UnknownEntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown entity kind '{}'", self.0)
    }
}

impl FromStr for EntityKind {
    type Err = UnknownEntityKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        EntityKind::all()
            .find(|k| k.slug() == s)
            .ok_or_else(|| UnknownEntityKind(s.to_string()))
    }
}

/// Round-trips through the route slug rather than deriving directly, so the
/// serialized form stays decoupled from the enum's variant order.
impl serde::Serialize for EntityKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for EntityKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Routable)]
pub enum Route {
    #[layout(RootLayout)]
    #[route("/")]
    Home {},

    #[route("/entities/:kind/:source/:name")]
    EntityDetail {
        kind: EntityKind,
        source: String,
        name: String,
    },
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

#[component]
fn Home() -> Element {
    rsx! {
        HomePage {}
    }
}

#[component]
fn EntityDetail(kind: EntityKind, source: String, name: String) -> Element {
    rsx! {
        EntityDetailPage { kind, source, name }
    }
}

#[component]
fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");
    rsx! {
        div { class: "p-8 text-center text-gray-500", "Nothing here: /{path}" }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_table_follows_declaration_order() {
        for (i, (kind, _)) in KINDS.iter().enumerate() {
            assert_eq!(*kind as usize, i, "{kind:?} is out of order in KINDS");
        }
    }

    #[test]
    fn slugs_round_trip() {
        for kind in EntityKind::all() {
            assert_eq!(kind.to_string().parse::<EntityKind>().unwrap(), kind);
        }
    }
}
