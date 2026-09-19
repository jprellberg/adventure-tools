# adventure-tools

## 1. General Instructions

This document is a specification of the app you have to build. When the user requests a feature or change that is missing from this document or contradicts this document, update it. Do not describe history (e.g., avoid "used to be X", "previously Y", or "this replaces Z").

## 2. Application Description and Requirements

adventure-tools is a Dioxus (Rust, WebAssembly) single-page application (SPA) for searching a reference database of entities (monsters, spells, items, rules, etc.), sourced from 5etools-format JSON.

The application opens to a flat white background with a single search bar horizontally centered at the top of the screen. The space below shows one of three views: the search results as entity cards, the entity cards that have been "pinned" by the user, or one entity in full (the detail view). A three-state toggle button to the left of the search bar (results, pins, detail) switches between them; the app opens on the pinned entities. Typing in the search bar, or adjusting a filter, switches to the search results. The search results are shown even while the search bar is empty (everything the filters let through, alphabetically), so that filtering for one kind or source alone works. The detail view shows the entity last opened in it, which stays the same while the other views are toggled through; without one it says how to pick it. Nameless entities are not searchable.

Clicking the title of a card or a hyperlinked entity - in any view - shows that entity in the detail view. A book is shown in the detail view by its page (see 2.3), so the toggle shows the detail state on a book's page. Every view is an address, `/search`, `/` or `/pins` and `/entities/:kind/:source/:name` (a book: `/books/:source/:..section`), and the entity of the detail view is appended to the address of the other views (`/search/:kind/:source/:name`, `/pins/:kind/:source/:name`), so that back/forward, a reload and a shared link reproduce the view and the entity. Switching views adds a history entry. A modified click (ctrl/cmd/shift/middle-click) on a title or hyperlink - "open in a new tab" - opens that address in a new tab.

Typing in the search bar should update the search results immediately (debounced briefly so fast typing doesn't re-filter the whole corpus on every keystroke). Searching runs over an index of lowercased names, sources and keywords built when the library is loaded, keeps only the best 60 matches, and is recomputed only when the query or filters or the library change. A result card builds its statblock only once it has scrolled into view (cards have a fixed size, so the layout doesn't move). Pressing ESC should clear the search bar and go back to the view the search results were opened from (the pinned entities or the detail view). The search bar should always be in focus so that typing on the page results in a search.

Search matches fuzzily against each entity's name, kind, and source, not just plain substrings: a query word matches if it's a substring of the field, or if it decomposes into consecutive prefixes of the field's words (e.g. "smogre" matches "Smoke Grenade" as "smo"+"gre"; "bes" matches the "Bestiary" kind). Multiple space-separated query words are ANDed together and can each match a different field, so "blight bes" finds only the bestiary entities named Blight.

A round button with three vertical dots to the right of the search bar opens the filter menu: a card border appears around the search bar, the button and a filter area below them (scrolling when it gets tall), and the page on either side of the card is dimmed slightly but stays visible. ESC or a click anywhere outside the card closes the menu. The area holds:

* A 2014/2024 rules toggle. 2024 (the default) switches off the source buttons of the three 2014 core rulebooks and on those of their 2024 revisions; 2014 is the inverse. The toggle only sets those six source buttons, which can afterwards be changed individually; every other source is unaffected by it.
* One on/off button per entity kind, colored like the kind tag on the cards while on and greyed out while off, with "all" and "none" buttons to switch every kind on or off.
* The same for every source in the library, colored like the source tag (tooltip: the sourcebook's full title), with the core rulebooks (the ones with their own tag color) listed first, then the rest alphabetically.

Everything except the 2014 core rulebooks is on by default; entities of a kind or source that is off are hidden. The filters affect the search results only, not the pinned grid (whatever is pinned is shown) and not entity content reached by following a link (which prefers the edition the rules toggle selects).

When the app opens for the first time, the required JSON data needs to be downloaded and cached. On subsequent startups, if data already exists, the app only checks if a newer version of the data is available for download and, if so, shows an icon button in the top right corner to start the update. While a download is in progress, the app shows a full-page centered loading screen with a spinner and text naming the resource currently being fetched or processed (e.g. the data file being downloaded).

The entity cards are fixed-width and fixed-height cards that display the entity in an appropriate format, e.g. a monster statblock in typical D&D fashion. Each entity card has a button in the top right corner to select a card for display in the pinned card area or remove it again. Since they are fixed-size, cards contain a vertical scrollbar if necessary. The cards are arranged in a grid that fills the available horizontal width; when the window is too narrow for two columns it becomes a single column, whose card width is the minimum: the page scrolls horizontally rather than squeezing cards. A card is 20rem × 29rem (approximately 3:4) with tight padding so most of that space holds content. In the pinned and search grids the cards grow uniformly to 1×2 or 2×2 (height × width, as factors of that size) when the window is large enough for all of them to be visible at once without scrolling: the largest of 2×2, 1×2 and 1×1 at which all cards fit, and 1×1 when even that doesn't fit (so a handful of pins fill the screen while a long result list stays compact).

Every rendering of an entity (card, hover popup, detail view) carries two small tag labels inline right after its title, wrapping with the title text (the pin button sits in the top-right corner and the title flows around it): an entity-kind tag (a distinct text color per kind) and, next to it, a source tag naming the sourcebook the entity comes from, with its own distinct color and a tooltip giving the sourcebook's full title. The core rulebooks, in either edition, get their own tag color, distinct from every other source.

Hyper-linked entities in the cards create a popup on mouse hover that shows the linked entity card, except when a card links to itself. The popup floats above every other card, never clipped by a card's own scroll boundary.

The set of pinned entities is stored in the URL, not in browser storage, so that a bookmark or a shared link reproduces it: every page's address carries a `?p=` query (rewritten in place, without adding history entries, whenever the pins change or a navigation drops it) that is the concatenation of one fixed-width id per pin, in pin order. An id is 42 bits of a hash of the entity's kind, source and name (case-insensitive) written as 7 base64url characters, so 17 pins fit into 128 characters of URL and an id stays valid across data updates for as long as the entity keeps its kind, source and name; ids that match no entity in the current data are ignored, but stay in the URL. A test guards that no two entities share an id and that the ids of known entities never change. Navigating to `/entities/:kind/:source/:name` shows that single entity's card at full size in the detail view; this is how a card is opened as its own page (e.g. for sharing a link, or a modified click as described above).

### 2.2 Entities

Every kind is searchable, pinnable and rendered in its own layout; the kind tag on a card (and the search) uses the kind's name:

* Creatures and rules: Bestiary (monsters, with the legendary group's lair actions and regional effects), Spells, Items (magic items, item groups, base equipment and generic magic variants), Classes, Subclasses, Races (including subraces), Backgrounds, Feats, Conditions/Diseases, Objects, Actions, Hazards (traps and hazards), Variant Rules, Optional Features, Character Options.
* Books: every book listed in `books.json` and every adventure listed in `adventures.json` (see 2.3).
* World and lore: Deities, Vehicles (and vehicle upgrades), Rewards, Psionics, Cults & Boons, Languages, Recipes, Bastion Facilities, Legendary Groups, Encounters, Tables, Decks, Cards, Crochet Patterns.
* Reference glossary entries: Item Properties, Item Masteries (weapon mastery properties), Skills, Senses.

Two kinds are extracted from the text of books and adventures: Tables (every captioned table, plus the standalone tables file) and Book Rules (named `entries` blocks one or two levels below a chapter that have prose of their own; books only), each with a breadcrumb path to where it sits, in which the book's title links to the book's page and the rest to the chapter, or the section of the chapter, the entity sits in (or is, for a section). The chapters and deeper blocks of books and adventures are not entities of their own. Data that only feeds 5etools' generators and tools (loot and name generators, the encounter builder, homebrew maker tables, life-event tables, item-type descriptions, fluff/lore and images files) is not loaded.

Cards show what the 5etools renderer shows for the kind, in the layout the current edition uses: monsters use the 2024 statblock (AC and initiative on one line, then HP, speed, a compact three-column ability table with score, modifier and saving throw, then skills/resistances/immunities/senses/languages/CR with XP and proficiency bonus, traits, actions, bonus actions, reactions, legendary and mythic actions, lair actions, regional effects, variants, habitat and treasure). Spells list their classes, subclasses and other sources. Structured fields whose content the entity's own text already spells out (a background's proficiencies, a race's darkvision) are not shown twice. Every field of every entity is either shown or declared as pure metadata (filter tags, art and token assets, SRD flags) in `corpus_tests.rs`, which fails when a new field appears in the data.

Named blocks in `entries` render according to their nesting depth, as in 5etools: a section and its first two levels of named sub-blocks are headings, and from the third level down the name is a bold run-in lead-in ("**Cleave.** If you hit a creature..."), so a deep outline such as the weapon Mastery Properties stays compact.

Every source present in the corpus is loaded for every kind (no curation to a fixed sourcebook list). Entities that rely on 5etools' `_copy`/`_mod` data-inheritance convention (a variant defined only as a diff against another entity) are resolved before parsing: the referenced base entity's fields are merged in, then the `_mod` operations (text replacement, and array insert/remove/replace/append/prepend on fields like traits/actions) are applied. A handful of narrow `_mod` modes that target data this app doesn't otherwise model or render (e.g. spellcasting lists) are accepted but have no effect. Text injectors are resolved at load too: `{=prop/mods}` (a recipe ingredient's amount, a magic variant's bonus) and `{#itemEntry Name|Source}` templates spliced into an item's entries.

### 2.3 Books

An adventure is a book: it is listed and paged like one (its text is read from `adventure/adventure-*.json`). A book's card carries only what is needed to pick it: the title with its tags, the pin button, and its group, author, publication date and, for an adventure, its storyline and level range. It is not meant to be read at card size, so it has no statblock and a click on its title (or on its "Read the book" link) opens the book's own page, `/books/:source/:..section`, which is the detail view of a book.

The book page shows the book's table of contents to the left of a center column with the text of the part selected in it. The column is 63rem wide (1.5 times the full-size card), and the text scrolls with the page, whose scrollbar sits at the window's right edge and reacts everywhere but over the table of contents, which stays in view beside the text and scrolls on its own; in a narrow window the table of contents sits above the text. The table of contents lists the chapters and, below each, the named sections inside it. Chapters fold their sections away with a toggle next to the chapter name; the selected chapter is always open. Selecting a chapter shows the whole chapter, selecting a section shows just that section, and the selection is part of the address: the index of the chapter, then the index of the section (`/books/:source/2/1`), with the first chapter shown when there is none. The text of a book is not part of the prepared library: it is read from the cached files when the page is opened.

## 3. Technical Requirements

### 3.1 Technology Stack & Deployment

Language & Framework: Rust with Dioxus Web targeting WebAssembly (wasm32-unknown-unknown).

Deployment Format: Single-Page Application (SPA) exported to static HTML, WebAssembly (.wasm), and JS bundles for pure static file hosting (e.g., GitHub Pages, Cloudflare Pages, Netlify, or local web servers).

Routing: dioxus-router managing application paths (/, /pins/:..entity, /search/:..entity, /entities/:..entity, /books/:source/:..section), where `:..entity` is empty or `:kind/:source/:name`.

### 3.2 Data Acquisition, Synchronisation & Storage

Data Source: Fetched directly from GitHub raw file hosting:
[https://github.com/5etools-mirror-3/5etools-src/tree/main/data](https://github.com/5etools-mirror-3/5etools-src/tree/main/data)

All persistent data, including fetched raw 5etools JSON files, commit/version metadata, are stored in IndexedDB.

The downloaded files are kept, and an update downloads only what changed: the repository tree lists a content sha per file, files whose sha is unchanged are not fetched again, and files that disappeared are deleted.

Startup is three composable steps: create the library (parse and resolve the cached files into the fully typed `Library`: `_copy`/`_mod` resolution, text injectors, book extraction, subrace/variant normalisation), serialize it (MessagePack, into its own IndexedDB store) and deserialize it. The serialized library is keyed by the data commit and a fingerprint of the app's sources, so it is discarded whenever either changes; while it is valid, every start only deserializes it, and the raw files are read (in one bulk read) only to create the library again after a data update or a new build.

### 3.3 Data Architecture & Schema Management

Typed Structs with Flattened Catch-All:

* Core entities utilize fully typed Rust structs for known fields combined with #[serde(flatten)] extra: serde_json::Map<String, Value> to retain unmodeled data without loss during import/edit/export cycles.
* Kinds without a dedicated struct share a minimal base struct (name, source, page, entries, extra); their kind-specific fields are read from `extra`.

Recursive Entry Hierarchy:
* Represented via an Entry enum (typed for entries/section/inset/item-style blocks, lists, tables with dice-range cells and footnotes, quotes and structured attacks; everything else - images, ability lines, spellcasting, references to class features, embedded statblocks - stays `Other(Value)` and is rendered from its `type`).
* Uses a custom Deserialize implementation to fall back to Entry::Other(Value) without failing on novel or custom block types.

### 3.4 UI Styling Strategy

Use tailwind css for styling and a responsive layout that works with different screen sizes. The app should have a minimal, light, flat design that relies mostly on typography itself.

High visual density in entity cards: Inline bold lead-ins for entry titles within rendered entries, tight cell padding for statblocks and tables, and compact heading/paragraph margins to maximize visible information density on screen.

## 4. Code Style Rules

* Favor strong typing wherever possible.
* Format all Rust code with rustfmt (`cargo fmt`, configured in `rustfmt.toml`) and keep `cargo clippy` free of warnings. Run both after changing code.
* Prefer to write little code. If you can make use of mature libraries instead of writing code yourself, use them.
* If you notice something isn't needed any longer or can be made simpler, remove or simplify.
* Do not keep temporary code in the repository. 

Comments:
* Comments must describe the current state of the code. Only comment when necessary. Never write historical descriptions (e.g., "formerly used X", "updated to fix Y").
* Document items (modules, types, functions, fields, variants, constants) with `///` (or `//!` for modules) docstrings, not with plain `//` comments above them. Plain `//` comments are only for explaining a specific statement inside a function body.
* Do not use sectioning comments (e.g. `// --- Tables ---`); structure code with modules and functions instead.
* Do not reference this document (AGENTS.md) or its sections from code or comments.
