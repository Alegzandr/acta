//! The labelled fixture that fixes the checker's thresholds (ADR 0015, 3):
//! 88 pairs of memories, half English, half French, labelled by hand
//! before the checker was run on them. Duplicates say the same fact in
//! other words; conflicts say incompatible things about the same subject;
//! related pairs share a subject but state different, compatible facts;
//! distinct pairs are about different things. Each memory is a title, a
//! summary and a body, as agents write them.
//!
//! The targets: duplicates precision and recall 0.8, conflicts precision
//! and recall 0.6. Any change to a threshold or a word list re-runs this
//! and reports both.

use super::{Profile, Text, Thresholds, Verdict, compare, compare_at, profile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum L {
    Dup,
    Conflict,
    Related,
    Distinct,
}

type M = [&'static str; 3];

const PAIRS: &[(L, M, M)] = &[
    // ---------------------------------------------------------------- English
    // Duplicates
    (
        L::Dup,
        [
            "Run tests with nextest",
            "Use cargo nextest run for the test suite",
            "The workspace tests run with `cargo nextest run`; plain cargo test is slower.",
        ],
        [
            "Test runner is nextest",
            "Tests are run via cargo nextest run",
            "Use `cargo nextest run` to run the workspace tests.",
        ],
    ),
    (
        L::Dup,
        [
            "Alexandre prefers short commit subjects",
            "Commit subjects under 60 chars, imperative",
            "Alexandre wants imperative commit subjects shorter than 60 characters.",
        ],
        [
            "Commit subject style",
            "Imperative mood, under 60 characters, for Alexandre",
            "Keep commit subjects imperative and under 60 chars; Alexandre reviews them.",
        ],
    ),
    (
        L::Dup,
        [
            "Perl needed for OpenSSL on Windows",
            "Vendored OpenSSL builds need a native Perl",
            "Set OPENSSL_SRC_PERL to Strawberry Perl; Git's Perl fails.",
        ],
        [
            "Windows OpenSSL build needs Strawberry Perl",
            "Native Perl required for vendored OpenSSL",
            "Git's Perl does not work; point OPENSSL_SRC_PERL at Strawberry Perl.",
        ],
    ),
    (
        L::Dup,
        [
            "Database key in the keychain",
            "The SQLCipher key is stored in the OS keychain",
            "On first run the key goes to the OS keychain under the entry acta.db.",
        ],
        [
            "SQLCipher key storage",
            "Key for the database lives in the OS keychain",
            "The database key is kept in the OS keychain, entry name acta.db.",
        ],
    ),
    (
        L::Dup,
        [
            "Long target paths break OpenSSL",
            "Keep CARGO_TARGET_DIR short on Windows",
            "OpenSSL Configure fails on long paths; use a short 8.3 target dir.",
        ],
        [
            "Short target dir on Windows",
            "OpenSSL Configure fails with a long CARGO_TARGET_DIR",
            "Use a short (8.3) CARGO_TARGET_DIR, or OpenSSL's Configure step fails.",
        ],
    ),
    (
        L::Dup,
        [
            "Snapshots before migrations",
            "A migration takes a database snapshot first",
            "Every schema upgrade snapshots the database before running its step.",
        ],
        [
            "Migration snapshot",
            "Schema upgrades snapshot the database first",
            "Before a migration step runs, a snapshot of the database is taken.",
        ],
    ),
    (
        L::Dup,
        [
            "Inter font license",
            "Inter ships under the SIL Open Font License",
            "The bundled Inter font is OFL licensed; keep its LICENSE file next to it.",
        ],
        [
            "Font licensing for Inter",
            "Inter is licensed under the SIL OFL",
            "Inter is under the SIL Open Font License; ship the license file with the font.",
        ],
    ),
    (
        L::Dup,
        [
            "Clippy must pass with -D warnings",
            "CI runs clippy with warnings as errors",
            "Every change must build clean with `cargo clippy --workspace --all-targets -- -D warnings`.",
        ],
        [
            "Clippy warnings are errors",
            "clippy -D warnings is required before commit",
            "Run `cargo clippy --workspace --all-targets -- -D warnings`; CI fails on any warning.",
        ],
    ),
    (
        L::Dup,
        [
            "Alexandre reviews on Fridays",
            "Weekly review happens Friday afternoon",
            "Alexandre does the weekly review of open topics on Friday afternoons.",
        ],
        [
            "Weekly review day",
            "Alexandre's weekly review is on Friday afternoon",
            "Open topics get reviewed by Alexandre every Friday afternoon.",
        ],
    ),
    (
        L::Dup,
        [
            "GPUI pinned to a fork",
            "The workspace pins gpui-pre 0.3.12",
            "GPUI comes from the gpui-pre crate pinned at 0.3.12; do not bump it casually.",
        ],
        [
            "Pinned GPUI version",
            "gpui-pre is pinned at 0.3.12",
            "The app uses gpui-pre 0.3.12, pinned in the workspace; bumps need a spike.",
        ],
    ),
    (
        L::Dup,
        [
            "Export files are sorted by UUID",
            "Arrays in .acta.json are sorted by UUID",
            "The exporter sorts every array by UUID so the same data gives the same file.",
        ],
        [
            "Deterministic exports",
            "Export arrays sorted by UUID for stable files",
            "Sorting arrays by UUID keeps the export file identical for identical data.",
        ],
    ),
    (
        L::Dup,
        [
            "Terminal uses ConPTY on Windows",
            "The embedded terminal runs on ConPTY",
            "On Windows the terminal dock spawns shells through ConPTY.",
        ],
        [
            "ConPTY for the terminal",
            "Windows terminal sessions go through ConPTY",
            "The embedded terminal spawns its shell via ConPTY on Windows.",
        ],
    ),
    (
        L::Dup,
        [
            "Hooks must answer within 50 ms",
            "Session start hook budget is 50 ms",
            "acta hook session-start must finish in under 50 ms or Claude Code waits.",
        ],
        [
            "Session start hook budget",
            "The session-start hook must take under 50 ms",
            "Keep acta hook session-start below 50 ms; it blocks Claude Code's start.",
        ],
    ),
    (
        L::Dup,
        [
            "Release builds are unsigned",
            "No code signing, by decision",
            "Acta builds are unsigned: no certificate, zero budget.",
        ],
        [
            "Unsigned builds",
            "We do not sign releases",
            "Releases are unsigned by decision; there is no code-signing certificate (zero budget).",
        ],
    ),
    // Conflicts
    (
        L::Conflict,
        [
            "Tests run in parallel",
            "cargo test runs the tests concurrently",
            "The store tests are safe to run in parallel.",
        ],
        [
            "Store tests are serial",
            "Do not run the store tests in parallel",
            "The store tests share a key store; never run them in parallel.",
        ],
    ),
    (
        L::Conflict,
        [
            "Request timeout",
            "The API client timeout is 30 s",
            "The update client gives up after a timeout of 30 seconds.",
        ],
        [
            "Update client timeout",
            "The update client timeout is 60 s",
            "The update client waits up to a timeout of 60 seconds before giving up.",
        ],
    ),
    (
        L::Conflict,
        [
            "Telemetry setting",
            "Crash reports are enabled by default",
            "Crash reports are enabled by default; users can turn them off in Settings.",
        ],
        [
            "Crash reports default",
            "Crash reports are disabled by default",
            "Crash reports are disabled by default and need an opt-in in Settings.",
        ],
    ),
    (
        L::Conflict,
        [
            "Dev server port",
            "The local test server listens on port 8080",
            "The downloader tests start a local server on port 8080.",
        ],
        [
            "Test server port",
            "Local test server uses port 3000",
            "The downloader tests start their local server on port 3000.",
        ],
    ),
    (
        L::Conflict,
        [
            "Node version for the docs site",
            "The docs site builds with Node 18",
            "Use Node version 18 to build the docs site.",
        ],
        [
            "Docs site Node version",
            "Docs site needs Node 20",
            "The docs site build requires Node version 20.",
        ],
    ),
    (
        L::Conflict,
        [
            "Commit on main",
            "Agents may commit directly on main",
            "Agents commit their work directly on main.",
        ],
        [
            "Never commit on main",
            "Agents work on their own branches",
            "Agents never commit on main; each works on its own branch and the lead merges.",
        ],
    ),
    (
        L::Conflict,
        [
            "Theme default",
            "The app starts in the light theme",
            "On first run the app uses the light theme until the user picks one.",
        ],
        [
            "First run theme",
            "The app follows the system theme on first run",
            "On first run the theme follows the system setting, not light.",
        ],
    ),
    (
        L::Conflict,
        [
            "Snapshot retention",
            "Keep the last 10 snapshots",
            "The snapshot rotation keeps 10 snapshots and deletes older ones.",
        ],
        [
            "Snapshot rotation",
            "Keep the last 5 snapshots",
            "Snapshot rotation keeps 5 snapshots; older ones are deleted.",
        ],
    ),
    (
        L::Conflict,
        [
            "Clippy config",
            "Clippy lints are allowed to warn",
            "Clippy warnings are fine locally; CI only reports them.",
        ],
        [
            "Clippy warnings are errors in CI",
            "CI fails on clippy warnings",
            "CI runs clippy with -D warnings and fails on any warning.",
        ],
    ),
    (
        L::Conflict,
        [
            "Hook output format",
            "Hooks print JSON on stdout",
            "acta hook writes its answer as JSON on stdout.",
        ],
        [
            "Hook output",
            "Hooks must not print JSON on stdout, only plain text",
            "acta hook writes plain text; JSON on stdout is never used.",
        ],
    ),
    (
        L::Conflict,
        [
            "Attachments limit",
            "An attachment may be up to 100 MB",
            "The maximum attachment size is 100 MB per file.",
        ],
        [
            "Attachment size limit",
            "Files are limited to 50 MB each",
            "The maximum attachment size is 50 MB per file.",
        ],
    ),
    (
        L::Conflict,
        [
            "Folder watcher",
            "Acta watches project folders for file changes",
            "A file watcher tracks changes in every project folder.",
        ],
        [
            "No file watching",
            "Acta does not watch folders for file changes",
            "Nothing watches files; git is read at read time only.",
        ],
    ),
    // Related
    (
        L::Related,
        [
            "Release checklist",
            "Steps before tagging a release",
            "Bump the version, update the changelog, then tag.",
        ],
        [
            "Changelog format",
            "The changelog follows Keep a Changelog",
            "Sections: Added, Changed, Fixed, under each version heading.",
        ],
    ),
    (
        L::Related,
        [
            "Database key in the keychain",
            "The SQLCipher key is stored in the OS keychain",
            "On first run the key goes to the OS keychain.",
        ],
        [
            "Key file fallback on Linux",
            "Without a keychain the key goes to acta.key",
            "On Linux without a secret service the key is written to the acta.key file with 0600 permissions.",
        ],
    ),
    (
        L::Related,
        [
            "Run tests with nextest",
            "Use cargo nextest run for the test suite",
            "Use `cargo nextest run`.",
        ],
        [
            "Timing tests are ignored",
            "Timing tests run only with --ignored",
            "The overview timing test is marked ignore; run it in release with --ignored.",
        ],
    ),
    (
        L::Related,
        [
            "Terminal uses ConPTY on Windows",
            "The embedded terminal runs on ConPTY",
            "Spawns shells through ConPTY.",
        ],
        [
            "Terminal colors",
            "The terminal theme follows the app theme",
            "The embedded terminal maps its 16 colors from the app's light and dark theme tokens.",
        ],
    ),
    (
        L::Related,
        [
            "Alexandre reviews on Fridays",
            "Weekly review happens Friday afternoon",
            "Alexandre reviews open topics on Fridays.",
        ],
        [
            "Alexandre's meeting slots",
            "Alexandre is in meetings on Monday mornings",
            "Do not expect answers from Alexandre before Monday noon.",
        ],
    ),
    (
        L::Related,
        [
            "Export files are sorted by UUID",
            "Arrays sorted by UUID",
            "Same data, same file.",
        ],
        [
            "Export excludes local values",
            "Read markers and local values are never exported",
            "Read markers, local values and the quality log stay on this machine.",
        ],
    ),
    (
        L::Related,
        [
            "Snapshots before migrations",
            "A migration takes a snapshot first",
            "Every schema upgrade snapshots the database.",
        ],
        [
            "Snapshot schedule",
            "A daily snapshot is taken by the app",
            "The app takes one snapshot a day in its background maintenance.",
        ],
    ),
    (
        L::Related,
        [
            "Hooks must answer within 50 ms",
            "Session start hook budget is 50 ms",
            "Keep it under 50 ms.",
        ],
        [
            "Hook pre-check file",
            "Hooks read a link index before opening the database",
            "The link index lets a hook skip opening the database when the folder is not linked.",
        ],
    ),
    // Distinct
    (
        L::Distinct,
        [
            "Inter font license",
            "Inter ships under the SIL OFL",
            "Keep its LICENSE file.",
        ],
        [
            "Icon set",
            "Icons come from Lucide",
            "The app's icons are Lucide SVGs, ISC licensed.",
        ],
    ),
    (
        L::Distinct,
        [
            "Alexandre prefers short commit subjects",
            "Imperative, under 60 chars",
            "For Alexandre's reviews.",
        ],
        [
            "Alexandre's time zone",
            "Alexandre works on Paris time",
            "Schedule anything for Alexandre in Europe/Paris time.",
        ],
    ),
    (
        L::Distinct,
        [
            "Perl needed for OpenSSL on Windows",
            "Vendored OpenSSL builds need a native Perl",
            "Strawberry Perl on Windows.",
        ],
        [
            "macOS uses CommonCrypto",
            "SQLCipher on macOS links CommonCrypto",
            "No OpenSSL build on macOS: SQLCipher uses CommonCrypto there.",
        ],
    ),
    (
        L::Distinct,
        [
            "GPUI pinned to a fork",
            "The workspace pins gpui-pre 0.3.12",
            "Do not bump it casually.",
        ],
        [
            "Webview for the chat panel",
            "The chat panel uses the system web view",
            "WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux.",
        ],
    ),
    (
        L::Distinct,
        [
            "Dictation model sizes",
            "Whisper Small is about 190 MB",
            "The Small model downloads 181 MiB.",
        ],
        [
            "Update channel",
            "Updates come from GitHub releases",
            "The updater reads the latest GitHub release feed.",
        ],
    ),
    (
        L::Distinct,
        [
            "Questions due dates",
            "A question can have an expected date",
            "Expected dates show in the Overview when overdue.",
        ],
        [
            "Contacts merge",
            "Merging contacts moves references",
            "Merge into... moves every reference and keeps one undo step.",
        ],
    ),
    (
        L::Distinct,
        [
            "Short target dir on Windows",
            "Keep CARGO_TARGET_DIR short",
            "Or OpenSSL's Configure fails.",
        ],
        [
            "Disk C is small",
            "Delete build dirs after use",
            "Remove target dirs and downloads from C: as soon as they are not needed.",
        ],
    ),
    (
        L::Distinct,
        ["Unsigned builds", "We do not sign releases", "Zero budget."],
        [
            "Linux packages",
            "Linux ships as an AppImage and a tarball",
            "No deb or rpm for now; AppImage plus tar.gz.",
        ],
    ),
    (
        L::Distinct,
        [
            "Weekly review day",
            "Friday afternoon",
            "Alexandre reviews open topics.",
        ],
        [
            "Inbox triage",
            "Notes older than 7 days show in the inbox line",
            "The Overview counts notes left in the inbox for 7 days or more.",
        ],
    ),
    (
        L::Distinct,
        [
            "Clippy warnings are errors",
            "CI runs clippy with -D warnings",
            "No warning is tolerated.",
        ],
        [
            "Formatting",
            "cargo fmt runs before every commit",
            "Run `cargo fmt --all` before committing; CI checks formatting.",
        ],
    ),
    // ----------------------------------------------------------------- French
    // Duplicates
    (
        L::Dup,
        [
            "Lancer les tests avec nextest",
            "La suite de tests se lance avec cargo nextest run",
            "Les tests du workspace se lancent avec `cargo nextest run`.",
        ],
        [
            "Les tests passent par nextest",
            "On lance les tests avec cargo nextest run",
            "Utiliser `cargo nextest run` pour lancer les tests du workspace.",
        ],
    ),
    (
        L::Dup,
        [
            "Alexandre relit le vendredi",
            "La revue hebdomadaire a lieu le vendredi après-midi",
            "Alexandre relit les sujets ouverts le vendredi après-midi.",
        ],
        [
            "Jour de la revue hebdomadaire",
            "Alexandre fait sa revue hebdomadaire le vendredi après-midi",
            "Les sujets ouverts sont relus par Alexandre chaque vendredi après-midi.",
        ],
    ),
    (
        L::Dup,
        [
            "Perl natif pour OpenSSL sous Windows",
            "OpenSSL embarqué exige un Perl natif",
            "Pointer OPENSSL_SRC_PERL vers Strawberry Perl ; le Perl de Git échoue.",
        ],
        [
            "Strawberry Perl pour compiler OpenSSL",
            "Il faut un Perl natif pour OpenSSL embarqué sous Windows",
            "Le Perl de Git échoue : OPENSSL_SRC_PERL doit pointer vers Strawberry Perl.",
        ],
    ),
    (
        L::Dup,
        [
            "Clé de la base dans le trousseau",
            "La clé SQLCipher est stockée dans le trousseau du système",
            "Au premier lancement, la clé va dans le trousseau du système, entrée acta.db.",
        ],
        [
            "Stockage de la clé SQLCipher",
            "La clé de la base est dans le trousseau du système",
            "La clé de la base est gardée dans le trousseau du système, sous l'entrée acta.db.",
        ],
    ),
    (
        L::Dup,
        [
            "Instantané avant migration",
            "Chaque migration prend d'abord un instantané de la base",
            "Avant chaque étape de schéma, un instantané de la base est pris.",
        ],
        [
            "Migration et instantané",
            "Une montée de schéma commence par un instantané",
            "Un instantané de la base est pris avant chaque étape de migration.",
        ],
    ),
    (
        L::Dup,
        [
            "Licence de la police Inter",
            "Inter est sous licence SIL OFL",
            "La police Inter est sous licence OFL ; garder son fichier LICENSE à côté.",
        ],
        [
            "Inter et sa licence",
            "La police Inter est distribuée sous SIL Open Font License",
            "Inter est sous licence SIL OFL : livrer le fichier de licence avec la police.",
        ],
    ),
    (
        L::Dup,
        [
            "Clippy sans avertissement",
            "La CI lance clippy avec -D warnings",
            "Chaque modification doit passer `cargo clippy --workspace --all-targets -- -D warnings`.",
        ],
        [
            "Avertissements clippy bloquants",
            "clippy -D warnings est exigé avant chaque commit",
            "Lancer `cargo clippy --workspace --all-targets -- -D warnings` ; la CI échoue au moindre avertissement.",
        ],
    ),
    (
        L::Dup,
        [
            "Sujets de commit courts",
            "Alexandre veut des sujets de commit à l'impératif de moins de 60 caractères",
            "Sujet de commit à l'impératif, moins de 60 caractères.",
        ],
        [
            "Style des sujets de commit",
            "Impératif et moins de 60 caractères pour Alexandre",
            "Garder les sujets de commit à l'impératif et sous 60 caractères.",
        ],
    ),
    (
        L::Dup,
        [
            "Le terminal utilise ConPTY",
            "Sous Windows, le terminal intégré passe par ConPTY",
            "Le dock du terminal lance les shells via ConPTY sous Windows.",
        ],
        [
            "ConPTY pour le terminal",
            "Les sessions du terminal Windows passent par ConPTY",
            "Le terminal intégré lance son shell avec ConPTY sous Windows.",
        ],
    ),
    (
        L::Dup,
        [
            "Builds non signés",
            "Pas de signature de code, par décision",
            "Les builds d'Acta ne sont pas signés : pas de certificat, budget nul.",
        ],
        [
            "Pas de signature des versions",
            "Les versions ne sont pas signées",
            "Aucun certificat de signature : les versions sont non signées, par décision (budget nul).",
        ],
    ),
    (
        L::Dup,
        [
            "Exports triés par UUID",
            "Les tableaux du fichier .acta.json sont triés par UUID",
            "L'export trie chaque tableau par UUID pour que les mêmes données donnent le même fichier.",
        ],
        [
            "Exports déterministes",
            "Tableaux de l'export triés par UUID",
            "Trier les tableaux par UUID garde le fichier d'export identique pour des données identiques.",
        ],
    ),
    (
        L::Dup,
        [
            "Crochet de démarrage en 50 ms",
            "Le hook session-start doit répondre en moins de 50 ms",
            "acta hook session-start doit finir en moins de 50 ms.",
        ],
        [
            "Budget du hook session-start",
            "Le hook de démarrage de session a 50 ms",
            "Garder acta hook session-start sous 50 ms : il bloque le démarrage de Claude Code.",
        ],
    ),
    (
        L::Dup,
        [
            "Dossier cible court sous Windows",
            "Configure d'OpenSSL échoue sur un chemin long",
            "Utiliser un CARGO_TARGET_DIR court (8.3) sinon Configure échoue.",
        ],
        [
            "Chemins longs et OpenSSL",
            "Garder CARGO_TARGET_DIR court sous Windows",
            "Le Configure d'OpenSSL échoue avec un chemin long ; prendre un dossier cible court.",
        ],
    ),
    (
        L::Dup,
        [
            "GPUI épinglé",
            "Le workspace épingle gpui-pre 0.3.12",
            "GPUI vient de gpui-pre épinglé en 0.3.12 ; ne pas le monter à la légère.",
        ],
        [
            "Version épinglée de GPUI",
            "gpui-pre est épinglé en 0.3.12",
            "L'app utilise gpui-pre 0.3.12, épinglé dans le workspace ; une montée demande un essai.",
        ],
    ),
    // Conflicts
    (
        L::Conflict,
        [
            "Tests en parallèle",
            "cargo test lance les tests en parallèle",
            "Les tests du store peuvent tourner en parallèle.",
        ],
        [
            "Tests du store en série",
            "Ne pas lancer les tests du store en parallèle",
            "Les tests du store partagent un trousseau : jamais en parallèle.",
        ],
    ),
    (
        L::Conflict,
        [
            "Délai du client de mise à jour",
            "Le délai d'attente est de 30 s",
            "Le client de mise à jour abandonne après un délai de 30 secondes.",
        ],
        [
            "Délai d'attente des mises à jour",
            "Le délai d'attente est de 60 s",
            "Le client de mise à jour attend un délai de 60 secondes avant d'abandonner.",
        ],
    ),
    (
        L::Conflict,
        [
            "Rapports de plantage",
            "Les rapports de plantage sont activés par défaut",
            "Activés par défaut ; on peut les couper dans les Réglages.",
        ],
        [
            "Rapports de plantage par défaut",
            "Les rapports de plantage sont désactivés par défaut",
            "Désactivés par défaut, il faut les activer dans les Réglages.",
        ],
    ),
    (
        L::Conflict,
        [
            "Port du serveur de test",
            "Le serveur local écoute sur le port 8080",
            "Les tests du téléchargeur démarrent un serveur local sur le port 8080.",
        ],
        [
            "Port du serveur local",
            "Le serveur de test utilise le port 3000",
            "Les tests du téléchargeur démarrent leur serveur local sur le port 3000.",
        ],
    ),
    (
        L::Conflict,
        [
            "Version de Node du site",
            "Le site de doc se construit avec Node 18",
            "Utiliser Node version 18 pour le site de doc.",
        ],
        [
            "Node pour le site de doc",
            "Le site de doc exige Node 20",
            "La construction du site de doc demande Node version 20.",
        ],
    ),
    (
        L::Conflict,
        [
            "Commits sur main",
            "Les agents peuvent committer directement sur main",
            "Les agents committent leur travail directement sur main.",
        ],
        [
            "Jamais de commit sur main",
            "Chaque agent travaille sur sa branche",
            "Les agents ne committent jamais sur main ; le responsable fusionne.",
        ],
    ),
    (
        L::Conflict,
        [
            "Conservation des instantanés",
            "Garder les 10 derniers instantanés",
            "La rotation garde 10 instantanés et supprime les plus anciens.",
        ],
        [
            "Rotation des instantanés",
            "Garder les 5 derniers instantanés",
            "La rotation garde 5 instantanés ; les plus anciens sont supprimés.",
        ],
    ),
    (
        L::Conflict,
        [
            "Taille des pièces jointes",
            "Une pièce jointe peut faire jusqu'à 100 Mo",
            "La taille maximale d'une pièce jointe est de 100 Mo.",
        ],
        [
            "Limite des pièces jointes",
            "Les fichiers sont limités à 50 Mo",
            "La taille maximale d'une pièce jointe est de 50 Mo.",
        ],
    ),
    (
        L::Conflict,
        [
            "Surveillance des dossiers",
            "Acta surveille les dossiers du projet",
            "Un observateur de fichiers suit les changements de chaque dossier.",
        ],
        [
            "Aucune surveillance de fichiers",
            "Acta ne surveille pas les dossiers",
            "Rien ne surveille les fichiers ; git est lu à la lecture seulement.",
        ],
    ),
    (
        L::Conflict,
        [
            "Sortie des hooks",
            "Les hooks écrivent du JSON sur stdout",
            "acta hook écrit sa réponse en JSON sur stdout.",
        ],
        [
            "Format de sortie des hooks",
            "Les hooks n'écrivent jamais de JSON, seulement du texte",
            "acta hook écrit du texte brut ; jamais de JSON sur stdout.",
        ],
    ),
    (
        L::Conflict,
        [
            "Thème au premier lancement",
            "L'app démarre en thème clair",
            "Au premier lancement, l'app utilise le thème clair.",
        ],
        [
            "Thème initial",
            "L'app suit le thème du système au premier lancement",
            "Au premier lancement, le thème suit le réglage du système, pas le clair.",
        ],
    ),
    (
        L::Conflict,
        [
            "Avertissements clippy en local",
            "Les avertissements clippy sont tolérés en local",
            "En local, les avertissements de clippy sont acceptés ; la CI les signale seulement.",
        ],
        [
            "Clippy bloque la CI",
            "La CI échoue sur tout avertissement clippy",
            "La CI lance clippy en -D warnings et échoue au moindre avertissement.",
        ],
    ),
    // Related
    (
        L::Related,
        [
            "Liste de contrôle de version",
            "Étapes avant de taguer une version",
            "Monter la version, mettre à jour le changelog, puis taguer.",
        ],
        [
            "Format du changelog",
            "Le changelog suit Keep a Changelog",
            "Sections Added, Changed, Fixed sous chaque version.",
        ],
    ),
    (
        L::Related,
        [
            "Clé dans le trousseau",
            "La clé SQLCipher est dans le trousseau du système",
            "Au premier lancement la clé va dans le trousseau.",
        ],
        [
            "Fichier de clé sous Linux",
            "Sans trousseau, la clé va dans acta.key",
            "Sous Linux sans service de secrets, la clé est écrite dans le fichier acta.key en 0600.",
        ],
    ),
    (
        L::Related,
        [
            "Lancer les tests avec nextest",
            "cargo nextest run pour la suite",
            "Utiliser `cargo nextest run`.",
        ],
        [
            "Tests de chronométrage ignorés",
            "Les tests de temps ne tournent qu'avec --ignored",
            "Le test de temps de l'Overview est marqué ignore ; le lancer en release avec --ignored.",
        ],
    ),
    (
        L::Related,
        [
            "Le terminal utilise ConPTY",
            "Le terminal intégré passe par ConPTY",
            "Lance les shells via ConPTY.",
        ],
        [
            "Couleurs du terminal",
            "Le thème du terminal suit celui de l'app",
            "Le terminal intégré prend ses 16 couleurs dans les jetons des thèmes clair et sombre.",
        ],
    ),
    (
        L::Related,
        [
            "Alexandre relit le vendredi",
            "Revue hebdomadaire le vendredi après-midi",
            "Alexandre relit les sujets ouverts le vendredi.",
        ],
        [
            "Créneaux de réunion d'Alexandre",
            "Alexandre est en réunion le lundi matin",
            "Ne pas attendre de réponse d'Alexandre avant lundi midi.",
        ],
    ),
    (
        L::Related,
        [
            "Exports triés par UUID",
            "Tableaux triés par UUID",
            "Mêmes données, même fichier.",
        ],
        [
            "L'export exclut les valeurs locales",
            "Les marqueurs de lecture ne sont jamais exportés",
            "Les marqueurs de lecture, les valeurs locales et le journal de qualité restent sur cette machine.",
        ],
    ),
    (
        L::Related,
        [
            "Instantané avant migration",
            "Une migration prend d'abord un instantané",
            "Chaque montée de schéma prend un instantané de la base.",
        ],
        [
            "Instantanés quotidiens",
            "L'app prend un instantané par jour",
            "L'app prend un instantané par jour pendant sa maintenance en arrière-plan.",
        ],
    ),
    (
        L::Related,
        [
            "Hook de démarrage en 50 ms",
            "Le hook session-start a 50 ms",
            "Le garder sous 50 ms.",
        ],
        [
            "Fichier d'index des liens",
            "Les hooks lisent un index des liens avant d'ouvrir la base",
            "L'index des liens permet à un hook de ne pas ouvrir la base si le dossier n'est pas lié.",
        ],
    ),
    // Distinct
    (
        L::Distinct,
        [
            "Licence de la police Inter",
            "Inter est sous SIL OFL",
            "Garder son fichier LICENSE.",
        ],
        [
            "Jeu d'icônes",
            "Les icônes viennent de Lucide",
            "Les icônes de l'app sont des SVG Lucide, sous licence ISC.",
        ],
    ),
    (
        L::Distinct,
        [
            "Sujets de commit courts",
            "Impératif, moins de 60 caractères",
            "Pour Alexandre.",
        ],
        [
            "Fuseau horaire d'Alexandre",
            "Alexandre travaille à l'heure de Paris",
            "Planifier tout ce qui concerne Alexandre en heure de Paris.",
        ],
    ),
    (
        L::Distinct,
        [
            "Perl natif pour OpenSSL",
            "OpenSSL embarqué exige un Perl natif",
            "Strawberry Perl sous Windows.",
        ],
        [
            "macOS utilise CommonCrypto",
            "SQLCipher sous macOS s'appuie sur CommonCrypto",
            "Pas de build d'OpenSSL sous macOS : SQLCipher y utilise CommonCrypto.",
        ],
    ),
    (
        L::Distinct,
        [
            "GPUI épinglé",
            "gpui-pre 0.3.12",
            "Ne pas monter à la légère.",
        ],
        [
            "Web view du panneau de chat",
            "Le panneau de chat utilise la web view du système",
            "WebView2 sous Windows, WKWebView sous macOS, WebKitGTK sous Linux.",
        ],
    ),
    (
        L::Distinct,
        [
            "Taille des modèles de dictée",
            "Whisper Small pèse environ 190 Mo",
            "Le modèle Small télécharge 181 Mio.",
        ],
        [
            "Canal de mise à jour",
            "Les mises à jour viennent des versions GitHub",
            "Le programme de mise à jour lit le flux de la dernière version GitHub.",
        ],
    ),
    (
        L::Distinct,
        [
            "Dates attendues des questions",
            "Une question peut avoir une date attendue",
            "Les dates dépassées apparaissent dans l'Overview.",
        ],
        [
            "Fusion des contacts",
            "Fusionner des contacts déplace les références",
            "Fusionner dans... déplace chaque référence et garde une seule étape d'annulation.",
        ],
    ),
    (
        L::Distinct,
        [
            "Dossier cible court",
            "CARGO_TARGET_DIR court sous Windows",
            "Sinon Configure échoue.",
        ],
        [
            "Le disque C est petit",
            "Supprimer les dossiers de build après usage",
            "Retirer les dossiers cible et les téléchargements de C: dès qu'ils ne servent plus.",
        ],
    ),
    (
        L::Distinct,
        [
            "Builds non signés",
            "Pas de signature de code",
            "Budget nul.",
        ],
        [
            "Paquets Linux",
            "Linux est livré en AppImage et en archive",
            "Pas de deb ni de rpm pour l'instant : AppImage et tar.gz.",
        ],
    ),
    (
        L::Distinct,
        [
            "Jour de la revue hebdomadaire",
            "Vendredi après-midi",
            "Alexandre relit les sujets ouverts.",
        ],
        [
            "Tri de la boîte de réception",
            "Les notes de plus de 7 jours apparaissent",
            "L'Overview compte les notes restées 7 jours ou plus dans la boîte de réception.",
        ],
    ),
    (
        L::Distinct,
        [
            "Clippy sans avertissement",
            "La CI lance clippy en -D warnings",
            "Aucun avertissement toléré.",
        ],
        [
            "Formatage",
            "cargo fmt avant chaque commit",
            "Lancer `cargo fmt --all` avant de committer ; la CI vérifie le formatage.",
        ],
    ),
];

/// Precision and recall of one class: `(tp, predicted, actual)`.
fn ratio(tp: usize, n: usize) -> f64 {
    if n == 0 { 1.0 } else { tp as f64 / n as f64 }
}

#[test]
fn the_labelled_fixture_meets_the_adr_targets() {
    assert!(PAIRS.len() >= 80, "{} pairs", PAIRS.len());
    let (mut dup, mut conflict) = ([0usize; 3], [0usize; 3]);
    let mut misses = Vec::new();
    for (i, (label, a, b)) in PAIRS.iter().enumerate() {
        let text = |m: &M| Text {
            title: m[0],
            summary: m[1],
            body: m[2],
        };
        let c = compare(&profile(&text(a)), &profile(&text(b)));
        let said_dup = matches!(c.verdict, Verdict::Same | Verdict::Duplicate);
        let said_conflict = c.verdict == Verdict::Conflict;
        let is_dup = *label == L::Dup;
        let is_conflict = *label == L::Conflict;
        dup[0] += usize::from(said_dup && is_dup);
        dup[1] += usize::from(said_dup);
        dup[2] += usize::from(is_dup);
        conflict[0] += usize::from(said_conflict && is_conflict);
        conflict[1] += usize::from(said_conflict);
        conflict[2] += usize::from(is_conflict);
        let expected = match label {
            L::Dup => matches!(c.verdict, Verdict::Same | Verdict::Duplicate),
            L::Conflict => said_conflict,
            L::Related | L::Distinct => !said_dup && !said_conflict,
        };
        if std::env::var_os("ACTA_FIXTURE_ALL").is_some() {
            println!(
                "  #{i} {label:?} {:?} t {:.2} w {:.2} s {:.2} {:?}",
                c.verdict, c.t, c.w, c.s, c.signal
            );
        }
        if !expected {
            misses.push(format!(
                "#{i} {label:?} as {:?} (t {:.2} w {:.2} s {:.2} {:?}): {}",
                c.verdict, c.t, c.w, c.s, c.signal, a[0]
            ));
        }
    }
    let dp = ratio(dup[0], dup[1]);
    let dr = ratio(dup[0], dup[2]);
    let cp = ratio(conflict[0], conflict[1]);
    let cr = ratio(conflict[0], conflict[2]);
    println!(
        "kb_quality fixture: {} pairs; duplicates precision {dp:.2} recall {dr:.2}; conflicts precision {cp:.2} recall {cr:.2}",
        PAIRS.len()
    );
    for m in &misses {
        println!("  {m}");
    }
    assert!(dp >= 0.8 && dr >= 0.8, "duplicates {dp:.2} / {dr:.2}");
    assert!(cp >= 0.6 && cr >= 0.6, "conflicts {cp:.2} / {cr:.2}");
}

/// Precision and recall of duplicates and conflicts on `pairs` at `th`.
fn measure(pairs: &[(L, Profile, Profile)], th: Thresholds) -> [f64; 4] {
    let (mut dup, mut conflict) = ([0usize; 3], [0usize; 3]);
    for (label, a, b) in pairs {
        let v = compare_at(a, b, th).verdict;
        let said_dup = matches!(v, Verdict::Same | Verdict::Duplicate);
        let said_conflict = v == Verdict::Conflict;
        dup[0] += usize::from(said_dup && *label == L::Dup);
        dup[1] += usize::from(said_dup);
        dup[2] += usize::from(*label == L::Dup);
        conflict[0] += usize::from(said_conflict && *label == L::Conflict);
        conflict[1] += usize::from(said_conflict);
        conflict[2] += usize::from(*label == L::Conflict);
    }
    [
        ratio(dup[0], dup[1]),
        ratio(dup[0], dup[2]),
        ratio(conflict[0], conflict[1]),
        ratio(conflict[0], conflict[2]),
    ]
}

fn f1(p: f64, r: f64) -> f64 {
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}

/// The thresholds that maximise the duplicate and conflict F1 on `pairs`
/// (a grid of 0.05 steps; ties go to the higher, stricter value).
fn tune(pairs: &[(L, Profile, Profile)]) -> Thresholds {
    let mut best = (f64::MIN, Thresholds::DEFAULT);
    for d in 6..=16 {
        for c in 4..=12 {
            let th = Thresholds {
                duplicate: d as f64 * 0.05,
                related: (d as f64 * 0.05 - 0.15).max(0.0),
                conflict: c as f64 * 0.05,
            };
            let m = measure(pairs, th);
            let score = f1(m[0], m[1]) + f1(m[2], m[3]);
            if score >= best.0 - 1e-12 {
                best = (score, th);
            }
        }
    }
    best.1
}

fn profiles(pairs: &[(L, M, M)]) -> Vec<(L, Profile, Profile)> {
    let p = |m: &M| {
        profile(&Text {
            title: m[0],
            summary: m[1],
            body: m[2],
        })
    };
    pairs.iter().map(|(l, a, b)| (*l, p(a), p(b))).collect()
}

/// Hold-out (review of 2026-11-22): thresholds tuned on one language,
/// measured on the other, both ways, so the reported figures are not the
/// ones the tuning saw. Printed and asserted at the ADR targets: the
/// figures of record are these hold-out ones.
#[test]
fn hold_out_tuned_on_one_language_measured_on_the_other() {
    let split = PAIRS
        .iter()
        .position(|(_, a, _)| a[0] == "Lancer les tests avec nextest")
        .unwrap();
    let en = profiles(&PAIRS[..split]);
    let fr = profiles(&PAIRS[split..]);
    for (name, train, test) in [("en->fr", &en, &fr), ("fr->en", &fr, &en)] {
        let th = tune(train);
        let m = measure(test, th);
        let d = measure(test, Thresholds::DEFAULT);
        println!(
            "kb_quality hold-out {name}: tuned dup {:.2} conflict {:.2}; on held-out half: duplicates P {:.2} R {:.2}, conflicts P {:.2} R {:.2} (shipped thresholds there: {:.2} {:.2} {:.2} {:.2})",
            th.duplicate, th.conflict, m[0], m[1], m[2], m[3], d[0], d[1], d[2], d[3]
        );
        assert!(m[0] >= 0.8 && m[1] >= 0.8, "{name} duplicates: {m:?}");
        assert!(m[2] >= 0.6 && m[3] >= 0.6, "{name} conflicts: {m:?}");
    }
}

#[test]
fn the_fixture_is_half_english_half_french() {
    // The French half starts at the first French title.
    let split = PAIRS
        .iter()
        .position(|(_, a, _)| a[0] == "Lancer les tests avec nextest")
        .unwrap();
    assert_eq!(split * 2, PAIRS.len());
    for half in [&PAIRS[..split], &PAIRS[split..]] {
        let n = |l: L| half.iter().filter(|(x, ..)| *x == l).count();
        assert!(
            n(L::Dup) >= 10 && n(L::Conflict) >= 10,
            "{:?}",
            (n(L::Dup), n(L::Conflict))
        );
    }
}
