//! Generated from the copy catalogue of docs/design/catalogue.md 10.4 (every row, in
//! table order). Edit docs/design/catalogue.md first, then this file; the tests in
//! `i18n` check the two agree.
//!
//! Typography is applied here, not in the table: U+2026 for "...", the
//! typographic apostrophe, and in French U+202F before ; : ! ? » and after
//! « and before units, U+00A0 before the dash of `update.ready`.

use super::{Msg, p, p_by, t};

catalogue! {
    CommonCancel "common.cancel" {
        en: t("Cancel"),
        fr: t("Annuler"),
    }
    CommonClose "common.close" {
        en: t("Close"),
        fr: t("Fermer"),
    }
    CommonUndo "common.undo" {
        en: t("Undo"),
        fr: t("Annuler"),
    }
    CommonRetry "common.retry" {
        en: t("Retry"),
        fr: t("Réessayer"),
    }
    CommonTryAgain "common.try_again" {
        en: t("Try again"),
        fr: t("Réessayer"),
    }
    CommonEdit "common.edit" {
        en: t("Edit"),
        fr: t("Modifier"),
    }
    CommonDelete "common.delete" {
        en: t("Delete"),
        fr: t("Supprimer"),
    }
    CommonRestore "common.restore" {
        en: t("Restore"),
        fr: t("Restaurer"),
    }
    CommonDismiss "common.dismiss" {
        en: t("Dismiss"),
        fr: t("Ignorer"),
    }
    CommonGotIt "common.got_it" {
        en: t("Got it"),
        fr: t("Compris"),
    }
    CommonCopied "common.copied" {
        en: t("Copied"),
        fr: t("Copié"),
    }
    CommonSave "common.save" {
        en: t("Save"),
        fr: t("Enregistrer"),
    }
    CommonOpen "common.open" {
        en: t("Open"),
        fr: t("Ouvrir"),
    }
    CommonMove "common.move" {
        en: t("Move"),
        fr: t("Déplacer"),
    }
    CommonHistory "common.history" {
        en: t("History"),
        fr: t("Historique"),
    }
    CommonDetails "common.details" {
        en: t("Details"),
        fr: t("Détails"),
    }
    CommonNone "common.none" {
        en: t("None"),
        fr: t("Aucun"),
    }
    CommonSearch "common.search" {
        en: t("Search"),
        fr: t("Rechercher"),
    }
    CommonClear "common.clear" {
        en: t("Clear"),
        fr: t("Effacer"),
    }
    CommonOpening "common.opening" {
        en: t("Opening\u{2026}"),
        fr: t("Ouverture\u{2026}"),
    }
    CommonChecking "common.checking" {
        en: t("Checking\u{2026}"),
        fr: t("Vérification\u{2026}"),
    }
    CommonShowAll "common.show_all" {
        en: t("Show all"),
        fr: t("Tout afficher"),
    }
    CommonShowAllN "common.show_all_n" {
        en: t("Show all {n}"),
        fr: t("Tout afficher ({n})"),
    }
    CommonBy "common.by" {
        en: t("{when} by {author}"),
        fr: t("{when} par {author}"),
    }
    TimeNow "time.now" {
        en: t("just now"),
        fr: t("à l\u{2019}instant"),
    }
    TimeMinutes "time.minutes" {
        en: t("{n} min ago"),
        fr: t("il y a {n} min"),
    }
    TimeHours "time.hours" {
        en: t("{n} h ago"),
        fr: t("il y a {n} h"),
    }
    TimeYesterday "time.yesterday" {
        en: t("yesterday {time}"),
        fr: t("hier à {time}"),
    }
    TimeWeekday "time.weekday" {
        en: t("{weekday} {time}"),
        fr: t("{weekday} à {time}"),
    }
    TimeDate "time.date" {
        en: t("{date}"),
        fr: t("le {date}"),
    }
    TimeDatetime "time.datetime" {
        en: t("{date}, {time}"),
        fr: t("{date} à {time}"),
    }
    TimeToday "time.today" {
        en: t("today, {time}"),
        fr: t("aujourd\u{2019}hui à {time}"),
    }
    TimeTodayWord "time.today_word" {
        en: t("today"),
        fr: t("aujourd\u{2019}hui"),
    }
    TimeDays "time.days" {
        en: p("{n} day ago", "{n} days ago"),
        fr: p("il y a {n} jour", "il y a {n} jours"),
    }
    TimeShortYesterday "time.short.yesterday" {
        en: t("Yesterday"),
        fr: t("Hier"),
    }
    CountProjects "count.projects" {
        en: p("{n} project", "{n} projects"),
        fr: p("{n} projet", "{n} projets"),
    }
    CountTopics "count.topics" {
        en: p("{n} topic", "{n} topics"),
        fr: p("{n} sujet", "{n} sujets"),
    }
    CountNotes "count.notes" {
        en: p("{n} note", "{n} notes"),
        fr: p("{n} note", "{n} notes"),
    }
    CountDocuments "count.documents" {
        en: p("{n} document", "{n} documents"),
        fr: p("{n} document", "{n} documents"),
    }
    CountMemories "count.memories" {
        en: p("{n} memory", "{n} memories"),
        fr: p("{n} mémoire", "{n} mémoires"),
    }
    TreeInbox "tree.inbox" {
        en: t("Inbox"),
        fr: t("Boîte de réception"),
    }
    TreeKnowledge "tree.knowledge" {
        en: t("Knowledge"),
        fr: t("Connaissances"),
    }
    TreeTrash "tree.trash" {
        en: t("Trash"),
        fr: t("Corbeille"),
    }
    TreeSettings "tree.settings" {
        en: t("Settings"),
        fr: t("Paramètres"),
    }
    TreeClosed "tree.closed" {
        en: t("{n} closed"),
        fr: t("{n} clos"),
    }
    TreeNoEpic "tree.no_epic" {
        en: t("No epic"),
        fr: t("Sans epic"),
    }
    StatusDraft "status.draft" {
        en: t("Draft"),
        fr: t("Brouillon"),
    }
    StatusPlanned "status.planned" {
        en: t("Planned"),
        fr: t("Prévu"),
    }
    StatusActive "status.active" {
        en: t("Active"),
        fr: t("En cours"),
    }
    StatusBlocked "status.blocked" {
        en: t("Blocked"),
        fr: t("Bloqué"),
    }
    StatusDone "status.done" {
        en: t("Done"),
        fr: t("Terminé"),
    }
    StatusDropped "status.dropped" {
        en: t("Dropped"),
        fr: t("Abandonné"),
    }
    EventStatus "event.status" {
        en: t("{author} set status to {status}"),
        fr: t("{author} a passé le statut à «\u{202f}{status}\u{202f}»"),
    }
    PropStatus "prop.status" {
        en: t("Status"),
        fr: t("Statut"),
    }
    PropEpic "prop.epic" {
        en: t("Epic"),
        fr: t("Epic"),
    }
    PropFolders "prop.folders" {
        en: t("Folders"),
        fr: t("Dossiers"),
    }
    PropId "prop.id" {
        en: t("Id"),
        fr: t("Identifiant"),
    }
    PropUpdated "prop.updated" {
        en: t("Updated"),
        fr: t("Modifié"),
    }
    PropProgress "prop.progress" {
        en: t("Progress"),
        fr: t("Avancement"),
    }
    PropKind "prop.kind" {
        en: t("Kind"),
        fr: t("Nature"),
    }
    PropLevel "prop.level" {
        en: t("Level"),
        fr: t("Niveau"),
    }
    PropProgressValue "prop.progress.value" {
        en: p_by("total", "{done} of {total} topic done", "{done} of {total} topics done"),
        fr: p_by("done", "{done} sujet terminé sur {total}", "{done} sujets terminés sur {total}"),
    }
    KindDocument "kind.document" {
        en: t("Document"),
        fr: t("Document"),
    }
    KindMemory "kind.memory" {
        en: t("Memory"),
        fr: t("Mémoire"),
    }
    LevelProject "level.project" {
        en: t("Project {name}"),
        fr: t("Projet {name}"),
    }
    LevelEpic "level.epic" {
        en: t("Epic {name}"),
        fr: t("Epic {name}"),
    }
    LevelTopic "level.topic" {
        en: t("Topic {name}"),
        fr: t("Sujet {name}"),
    }
    SectionState "section.state" {
        en: t("Current state"),
        fr: t("État actuel"),
    }
    SectionDescription "section.description" {
        en: t("Description"),
        fr: t("Description"),
    }
    SectionKnowledge "section.knowledge" {
        en: t("Knowledge"),
        fr: t("Connaissances"),
    }
    SectionTimeline "section.timeline" {
        en: t("Timeline"),
        fr: t("Journal"),
    }
    StateUpdated "state.updated" {
        en: t("Updated {when} by {author}"),
        fr: t("Modifié {when} par {author}"),
    }
    StatePlaceholder "state.placeholder" {
        en: t("Where does this stand? One or two sentences."),
        fr: t("Où en est-on\u{202f}? Une ou deux phrases."),
    }
    DescriptionPlaceholder "description.placeholder" {
        en: t("What is this topic about, and what does done look like?"),
        fr: t("De quoi s\u{2019}agit-il, et à quoi verra-t-on que c\u{2019}est terminé\u{202f}?"),
    }
    TimelineAll "timeline.all" {
        en: t("All"),
        fr: t("Tout"),
    }
    EntryProgress "entry.progress" {
        en: t("Progress"),
        fr: t("Avancement"),
    }
    EntryDecision "entry.decision" {
        en: t("Decision"),
        fr: t("Décision"),
    }
    EntryDecisions "entry.decisions" {
        en: t("Decisions"),
        fr: t("Décisions"),
    }
    EntryDiscussion "entry.discussion" {
        en: t("Discussion"),
        fr: t("Discussion"),
    }
    ComposerPlaceholder "composer.placeholder" {
        en: t("Add an entry\u{2026}"),
        fr: t("Ajouter une entrée\u{2026}"),
    }
    ComposerHint "composer.hint" {
        en: t("{keys} to save"),
        fr: t("{keys} pour enregistrer"),
    }
    ComposerAdd "composer.add" {
        en: t("Add entry"),
        fr: t("Ajouter l\u{2019}entrée"),
    }
    TimelineNew "timeline.new" {
        en: p("{n} new entry", "{n} new entries"),
        fr: p("{n} nouvelle entrée", "{n} nouvelles entrées"),
    }
    TimelineEmpty "timeline.empty" {
        en: t("No entries yet. Add one, or let Claude Code log progress as it works."),
        fr: t("Aucune entrée pour l\u{2019}instant. Ajoutez-en une, ou laissez Claude Code consigner l\u{2019}avancement au fil du travail."),
    }
    EntryCopyMarkdown "entry.copy_markdown" {
        en: t("Copy as Markdown"),
        fr: t("Copier en Markdown"),
    }
    EntryChangeType "entry.change_type" {
        en: t("Change type"),
        fr: t("Changer le type"),
    }
    ConflictNotice "conflict.notice" {
        en: t("Claude Code changed this while you were editing."),
        fr: t("Claude Code a modifié ceci pendant que vous écriviez."),
    }
    ConflictTheirs "conflict.theirs" {
        en: t("Show theirs"),
        fr: t("Voir sa version"),
    }
    ConflictMine "conflict.mine" {
        en: t("Keep mine"),
        fr: t("Garder la mienne"),
    }
    DeletedNotice "deleted.notice" {
        en: t("This topic was deleted by Claude Code."),
        fr: t("Ce sujet a été supprimé par Claude Code."),
    }
    DeletedRestore "deleted.restore" {
        en: t("Restore"),
        fr: t("Restaurer"),
    }
    TopicNew "topic.new" {
        en: t("New topic"),
        fr: t("Nouveau sujet"),
    }
    TopicNeedsTitle "topic.needs_title" {
        en: t("A topic needs a title."),
        fr: t("Un sujet doit avoir un titre."),
    }
    TopicDeleted "topic.deleted" {
        en: t("Topic deleted."),
        fr: t("Sujet supprimé."),
    }
    AuthorYou "author.you" {
        en: t("You"),
        fr: t("Vous"),
    }
    KbDeleted "kb.deleted" {
        en: t("Deleted from knowledge."),
        fr: t("Supprimé des connaissances."),
    }
    NoteDeleted "note.deleted" {
        en: t("Note deleted."),
        fr: t("Note supprimée."),
    }
    EntryDeleted "entry.deleted" {
        en: t("Entry deleted."),
        fr: t("Entrée supprimée."),
    }
    EpicDeleted "epic.deleted" {
        en: t("Epic deleted."),
        fr: t("Epic supprimé."),
    }
    ProjectDeleted "project.deleted" {
        en: t("Project deleted."),
        fr: t("Projet supprimé."),
    }
    TopicRestored "topic.restored" {
        en: t("Topic restored."),
        fr: t("Sujet restauré."),
    }
    TopicEmptyProject "topic.empty_project" {
        en: t("No topics yet. Capture notes and let Claude Code file them, or start one yourself."),
        fr: t("Aucun sujet pour l\u{2019}instant. Saisissez des notes et laissez Claude Code les classer, ou créez-en un vous-même."),
    }
    ErrorSaveEntry "error.save_entry" {
        en: t("Could not save the entry. The database is locked by another process."),
        fr: t("Impossible d\u{2019}enregistrer l\u{2019}entrée. La base de données est verrouillée par un autre processus."),
    }
    ErrorSave "error.save" {
        en: t("Could not save the change. Try again."),
        fr: t("Impossible d\u{2019}enregistrer la modification. Réessayez."),
    }
    UndoChanged "undo.changed" {
        en: t("Not undone: it was changed since."),
        fr: t("Non annulé\u{202f}: cela a été modifié depuis."),
    }
    TableCount "table.count" {
        en: p("{n} topic", "{n} topics"),
        fr: p("{n} sujet", "{n} sujets"),
    }
    TableFilterOpen "table.filter.open" {
        en: t("Open"),
        fr: t("Ouverts"),
    }
    TableFilterAll "table.filter.all" {
        en: t("All"),
        fr: t("Tous"),
    }
    TableFilterClosed "table.filter.closed" {
        en: t("Closed"),
        fr: t("Clos"),
    }
    TableTitle "table.title" {
        en: t("Title"),
        fr: t("Titre"),
    }
    TableEntries "table.entries" {
        en: t("Entries"),
        fr: t("Entrées"),
    }
    TableUpdated "table.updated" {
        en: t("Updated"),
        fr: t("Modifié"),
    }
    TableReset "table.reset" {
        en: t("Reset order"),
        fr: t("Rétablir l\u{2019}ordre"),
    }
    TableGroupProgress "table.group_progress" {
        en: t("{done} of {total} done"),
        fr: p_by("done", "{done} sur {total} terminé", "{done} sur {total} terminés"),
    }
    InboxCount "inbox.count" {
        en: p("{n} note", "{n} notes"),
        fr: p("{n} note", "{n} notes"),
    }
    InboxPlaceholder "inbox.placeholder" {
        en: t("Note for {project}"),
        fr: t("Note pour {project}"),
    }
    InboxAdd "inbox.add" {
        en: t("Add"),
        fr: t("Ajouter"),
    }
    InboxFilterOpen "inbox.filter.open" {
        en: t("Open"),
        fr: t("À traiter"),
    }
    InboxFilterFiled "inbox.filter.filed" {
        en: t("Filed"),
        fr: t("Classées"),
    }
    InboxFile "inbox.file" {
        en: t("File with Claude Code"),
        fr: t("Classer avec Claude Code"),
    }
    InboxFiledAs "inbox.filed_as" {
        en: t("Filed as"),
        fr: t("Classée dans"),
    }
    InboxMakeTopic "inbox.make_topic" {
        en: t("Make topic"),
        fr: t("Créer un sujet"),
    }
    InboxEmpty "inbox.empty" {
        en: t("Inbox is empty. Notes you capture land here until they become topics."),
        fr: t("La boîte de réception est vide. Les notes que vous saisissez arrivent ici jusqu\u{2019}à devenir des sujets."),
    }
    KbNewDocument "kb.new_document" {
        en: t("New document"),
        fr: t("Nouveau document"),
    }
    KbNewMemory "kb.new_memory" {
        en: t("New memory"),
        fr: t("Nouvelle mémoire"),
    }
    KbWrittenDocument "kb.written.document" {
        en: t("Written by {author} {when}"),
        fr: t("Écrit par {author} {when}"),
    }
    KbWrittenMemory "kb.written.memory" {
        en: t("Written by {author} {when}"),
        fr: t("Écrite par {author} {when}"),
    }
    KbEditedDocument "kb.edited.document" {
        en: t("edited by {author} {when}"),
        fr: t("modifié par {author} {when}"),
    }
    KbEditedMemory "kb.edited.memory" {
        en: t("edited by {author} {when}"),
        fr: t("modifiée par {author} {when}"),
    }
    KbFromEpic "kb.from_epic" {
        en: t("From epic {name}"),
        fr: t("De l\u{2019}epic {name}"),
    }
    KbFromProject "kb.from_project" {
        en: t("From project {name}"),
        fr: t("Du projet {name}"),
    }
    KbMoreInherited "kb.more_inherited" {
        en: t("{n} more from epic and project"),
        fr: p("{n} autre hérité de l\u{2019}epic et du projet", "{n} autres hérités de l\u{2019}epic et du projet"),
    }
    KbMoreEpic "kb.more_epic" {
        en: t("{n} more in the topics of this epic"),
        fr: t("{n} de plus dans les sujets de cet epic"),
    }
    KbMoreProject "kb.more_project" {
        en: t("{n} more in this project\u{2019}s epics and topics"),
        fr: t("{n} de plus dans les epics et sujets de ce projet"),
    }
    KbEmpty "kb.empty" {
        en: t("Nothing here yet. Documents and memories for this topic appear here."),
        fr: t("Rien ici pour l\u{2019}instant. Les documents et mémoires de ce sujet apparaîtront ici."),
    }
    KbSummaryMemory "kb.summary.memory" {
        en: t("One line: the fact, in short"),
        fr: t("Une ligne\u{202f}: le fait, en bref"),
    }
    KbSummaryDocument "kb.summary.document" {
        en: t("One line: what this is and when to read it"),
        fr: t("Une ligne\u{202f}: ce que c\u{2019}est et quand le lire"),
    }
    KbShowHistory "kb.show_history" {
        en: t("Show history"),
        fr: t("Afficher l\u{2019}historique"),
    }
    KbMoveEpic "kb.move_epic" {
        en: t("Move to epic \"{name}\""),
        fr: t("Déplacer vers l\u{2019}epic «\u{202f}{name}\u{202f}»"),
    }
    KbMoveProject "kb.move_project" {
        en: t("Move to project \"{name}\""),
        fr: t("Déplacer vers le projet «\u{202f}{name}\u{202f}»"),
    }
    KbMoveTo "kb.move_to" {
        en: t("Move to\u{2026}"),
        fr: t("Déplacer vers\u{2026}"),
    }
    KbChangeKind "kb.change_kind" {
        en: t("Change kind"),
        fr: t("Changer de nature"),
    }
    KbSaveFile "kb.save_file" {
        en: t("Save as file"),
        fr: t("Enregistrer comme fichier"),
    }
    KbMovedProjectDocument "kb.moved.project.document" {
        en: t("Moved to project {name}."),
        fr: t("Déplacé vers le projet {name}."),
    }
    KbMovedProjectMemory "kb.moved.project.memory" {
        en: t("Moved to project {name}."),
        fr: t("Déplacée vers le projet {name}."),
    }
    KbMovedEpicDocument "kb.moved.epic.document" {
        en: t("Moved to epic {name}."),
        fr: t("Déplacé vers l\u{2019}epic {name}."),
    }
    KbMovedEpicMemory "kb.moved.epic.memory" {
        en: t("Moved to epic {name}."),
        fr: t("Déplacée vers l\u{2019}epic {name}."),
    }
    KbMovedTopicDocument "kb.moved.topic.document" {
        en: t("Moved to topic {name}."),
        fr: t("Déplacé vers le sujet {name}."),
    }
    KbMovedTopicMemory "kb.moved.topic.memory" {
        en: t("Moved to topic {name}."),
        fr: t("Déplacée vers le sujet {name}."),
    }
    KbScope "kb.scope" {
        en: t("{project} · {documents}, {memories}"),
        fr: t("{project} · {documents}, {memories}"),
    }
    KbSearch "kb.search" {
        en: t("Search knowledge"),
        fr: t("Rechercher dans les connaissances"),
    }
    KbFilterLevel "kb.filter.level" {
        en: t("Level: {value}"),
        fr: t("Niveau\u{202f}: {value}"),
    }
    KbFilterKind "kb.filter.kind" {
        en: t("Kind: {value}"),
        fr: t("Nature\u{202f}: {value}"),
    }
    KbFilterAuthor "kb.filter.author" {
        en: t("Author: {value}"),
        fr: t("Auteur\u{202f}: {value}"),
    }
    KbValueAll "kb.value.all" {
        en: t("All"),
        fr: t("Tous"),
    }
    KbValueAllKind "kb.value.all.kind" {
        en: t("All"),
        fr: t("Toutes"),
    }
    KbValueProject "kb.value.project" {
        en: t("Project"),
        fr: t("Projet"),
    }
    KbValueEpics "kb.value.epics" {
        en: t("Epics"),
        fr: t("Epics"),
    }
    KbValueTopics "kb.value.topics" {
        en: t("Topics"),
        fr: t("Sujets"),
    }
    KbValueDocuments "kb.value.documents" {
        en: t("Documents"),
        fr: t("Documents"),
    }
    KbValueMemories "kb.value.memories" {
        en: t("Memories"),
        fr: t("Mémoires"),
    }
    KbResults "kb.results" {
        en: p("{n} result for \"{query}\"", "{n} results for \"{query}\""),
        fr: p("{n} résultat pour «\u{202f}{query}\u{202f}»", "{n} résultats pour «\u{202f}{query}\u{202f}»"),
    }
    KbNoResults "kb.no_results" {
        en: t("Nothing in this project mentions \"{query}\"."),
        fr: t("Rien dans ce projet ne mentionne «\u{202f}{query}\u{202f}»."),
    }
    DocEditing "doc.editing" {
        en: t("Editing"),
        fr: t("Modification en cours"),
    }
    DocNeeds "doc.needs" {
        en: t("A document needs a title and a summary."),
        fr: t("Un document doit avoir un titre et un résumé."),
    }
    DocGoHeading "doc.go_heading" {
        en: t("Go to heading"),
        fr: t("Aller au titre"),
    }
    HistoryVersion "history.version" {
        en: t("Version from {datetime} by {author}"),
        fr: t("Version du {datetime} par {author}"),
    }
    HistoryText "history.text" {
        en: t("Text"),
        fr: t("Texte"),
    }
    HistoryChanges "history.changes" {
        en: t("Changes"),
        fr: t("Modifications"),
    }
    HistoryRestore "history.restore" {
        en: t("Restore this version"),
        fr: t("Restaurer cette version"),
    }
    HistoryRestored "history.restored" {
        en: t("Restored the version from {datetime}."),
        fr: t("Version du {datetime} restaurée."),
    }
    HistoryReplaced "history.replaced" {
        en: t("Replaced section {name}"),
        fr: t("Section {name} remplacée"),
    }
    HistoryEditedDocument "history.edited.document" {
        en: t("Edited"),
        fr: t("Modifié"),
    }
    HistoryEditedMemory "history.edited.memory" {
        en: t("Edited"),
        fr: t("Modifiée"),
    }
    HistoryFromFileDocument "history.from_file.document" {
        en: t("Created from file"),
        fr: t("Créé depuis un fichier"),
    }
    HistoryFromFileMemory "history.from_file.memory" {
        en: t("Created from file"),
        fr: t("Créée depuis un fichier"),
    }
    FoldersTitle "folders.title" {
        en: t("Working folders"),
        fr: t("Dossiers de travail"),
    }
    FoldersFromEpic "folders.from_epic" {
        en: t("from epic {name}"),
        fr: t("de l\u{2019}epic {name}"),
    }
    FoldersFromProject "folders.from_project" {
        en: t("from project {name}"),
        fr: t("du projet {name}"),
    }
    FoldersPrimary "folders.primary" {
        en: t("Primary"),
        fr: t("Principal"),
    }
    FoldersInclude "folders.include" {
        en: t("Also include inherited folders"),
        fr: t("Inclure aussi les dossiers hérités"),
    }
    FoldersNotUsed "folders.not_used" {
        en: p("Not used here: {n} folder from epic {name}.", "Not used here: {n} folders from epic {name}."),
        fr: p("Non utilisé ici\u{202f}: {n} dossier de l\u{2019}epic {name}.", "Non utilisés ici\u{202f}: {n} dossiers de l\u{2019}epic {name}."),
    }
    FoldersAdd "folders.add" {
        en: t("Add folder\u{2026}"),
        fr: t("Ajouter un dossier\u{2026}"),
    }
    FoldersHint "folders.hint" {
        en: t("Claude Code starts in the primary folder and may also read and edit the others."),
        fr: t("Claude Code démarre dans le dossier principal et peut aussi lire et modifier les autres."),
    }
    FoldersSetHere "folders.set_here" {
        en: t("Set folder for this topic\u{2026}"),
        fr: t("Définir un dossier pour ce sujet\u{2026}"),
    }
    FoldersUses "folders.uses" {
        en: t("This topic uses the folders of epic {name}. A folder set here replaces them for this topic only."),
        fr: t("Ce sujet utilise les dossiers de l\u{2019}epic {name}. Un dossier défini ici les remplace pour ce sujet seulement."),
    }
    FoldersNone "folders.none" {
        en: t("No working folder. The terminal will ask where to start."),
        fr: t("Aucun dossier de travail. Le terminal demandera où démarrer."),
    }
    FoldersSet "folders.set" {
        en: t("Set working folder\u{2026}"),
        fr: t("Définir le dossier de travail\u{2026}"),
    }
    FoldersNotFound "folders.not_found" {
        en: t("Not found on this computer"),
        fr: t("Introuvable sur cet ordinateur"),
    }
    FoldersLocate "folders.locate" {
        en: t("Locate\u{2026}"),
        fr: t("Localiser\u{2026}"),
    }
    FoldersNNotFound "folders.n_not_found" {
        en: t("{n} not found"),
        fr: p("{n} introuvable", "{n} introuvables"),
    }
    FoldersAlsoTopic "folders.also_topic" {
        en: t("Also attached to topic {name}. A session started in this folder outside Acta opens on epic {epic} and is told about both topics."),
        fr: t("Également rattaché au sujet {name}. Une session lancée dans ce dossier hors d\u{2019}Acta s\u{2019}ouvre sur l\u{2019}epic {epic} et est informée des deux sujets."),
    }
    FoldersAlsoEpic "folders.also_epic" {
        en: t("Also attached to epic {name}. A session started in this folder outside Acta opens on project {project} and is told about both epics."),
        fr: t("Également rattaché à l\u{2019}epic {name}. Une session lancée dans ce dossier hors d\u{2019}Acta s\u{2019}ouvre sur le projet {project} et est informée des deux epics."),
    }
    FoldersAlsoMore "folders.also_more" {
        en: t("Also attached to topics {a}, {b} and {n} more."),
        fr: p("Également rattaché aux sujets {a}, {b} et {n} autre.", "Également rattaché aux sujets {a}, {b} et {n} autres."),
    }
    FoldersShowW "folders.show.w" {
        en: t("Show in Explorer"),
        fr: t("Afficher dans l\u{2019}Explorateur"),
    }
    FoldersShowM "folders.show.m" {
        en: t("Reveal in Finder"),
        fr: t("Afficher dans le Finder"),
    }
    FoldersShowL "folders.show.l" {
        en: t("Show in file manager"),
        fr: t("Afficher dans le gestionnaire de fichiers"),
    }
    FoldersCopyPath "folders.copy_path" {
        en: t("Copy path"),
        fr: t("Copier le chemin"),
    }
    FoldersMakePrimary "folders.make_primary" {
        en: t("Make primary"),
        fr: t("Définir comme principal"),
    }
    FoldersMoveUp "folders.move_up" {
        en: t("Move up"),
        fr: t("Monter"),
    }
    FoldersMoveDown "folders.move_down" {
        en: t("Move down"),
        fr: t("Descendre"),
    }
    FoldersChooseOther "folders.choose_other" {
        en: t("Choose another folder\u{2026}"),
        fr: t("Choisir un autre dossier\u{2026}"),
    }
    FoldersRemoveTopic "folders.remove.topic" {
        en: t("Remove from this topic"),
        fr: t("Retirer de ce sujet"),
    }
    FoldersRemoveEpic "folders.remove.epic" {
        en: t("Remove from this epic"),
        fr: t("Retirer de cet epic"),
    }
    FoldersRemoveProject "folders.remove.project" {
        en: t("Remove from this project"),
        fr: t("Retirer de ce projet"),
    }
    FoldersEditIn "folders.edit_in" {
        en: t("Edit in epic {name}"),
        fr: t("Modifier dans l\u{2019}epic {name}"),
    }
    FoldersRemovedTopic "folders.removed.topic" {
        en: t("{folder} removed from this topic."),
        fr: t("{folder} retiré de ce sujet."),
    }
    FoldersRemovedEpic "folders.removed.epic" {
        en: t("{folder} removed from this epic."),
        fr: t("{folder} retiré de cet epic."),
    }
    FoldersRemovedProject "folders.removed.project" {
        en: t("{folder} removed from this project."),
        fr: t("{folder} retiré de ce projet."),
    }
    FoldersLocatedFor "folders.located_for" {
        en: t("{folder} located for epic {name} and everything that inherits from it."),
        fr: t("{folder} localisé pour l\u{2019}epic {name} et tout ce qui en hérite."),
    }
    FoldersBulk "folders.bulk" {
        en: p("{n} other folder under {root} was not found either. Apply the same change to it?", "{n} other folders under {root} were not found either. Apply the same change to them?"),
        fr: p("{n} autre dossier sous {root} est également introuvable. Lui appliquer le même changement\u{202f}?", "{n} autres dossiers sous {root} sont également introuvables. Leur appliquer le même changement\u{202f}?"),
    }
    FoldersBulkApply "folders.bulk_apply" {
        en: p("Apply to {n} folder", "Apply to {n} folders"),
        fr: p("Appliquer à {n} dossier", "Appliquer à {n} dossiers"),
    }
    FoldersBulkDone "folders.bulk_done" {
        en: p("{n} folder located.", "{n} folders located."),
        fr: p("{n} dossier localisé.", "{n} dossiers localisés."),
    }
    FoldersPicker "folders.picker" {
        en: t("Choose a working folder"),
        fr: t("Choisir un dossier de travail"),
    }
    TerminalTitle "terminal.title" {
        en: t("Terminal"),
        fr: t("Terminal"),
    }
    TerminalOnTopic "terminal.on_topic" {
        en: t("on topic {name}"),
        fr: t("sur le sujet {name}"),
    }
    TerminalOnEpic "terminal.on_epic" {
        en: t("on epic {name}"),
        fr: t("sur l\u{2019}epic {name}"),
    }
    TerminalOnProject "terminal.on_project" {
        en: t("on project {name}"),
        fr: t("sur le projet {name}"),
    }
    TerminalRestartHere "terminal.restart_here" {
        en: t("Restart here"),
        fr: t("Redémarrer ici"),
    }
    TerminalEnded "terminal.ended" {
        en: t("Session ended. {keys} to restart."),
        fr: t("Session terminée. {keys} pour relancer."),
    }
    TerminalAskTitle "terminal.ask.title" {
        en: t("Where should Claude Code start?"),
        fr: t("Où Claude Code doit-il démarrer\u{202f}?"),
    }
    TerminalAskTopic "terminal.ask.topic" {
        en: t("{name} has no working folder, and neither has its epic or its project."),
        fr: t("{name} n\u{2019}a pas de dossier de travail, son epic et son projet non plus."),
    }
    TerminalAskEpic "terminal.ask.epic" {
        en: t("{name} has no working folder, and neither has its project."),
        fr: t("{name} n\u{2019}a pas de dossier de travail, son projet non plus."),
    }
    TerminalAskProject "terminal.ask.project" {
        en: t("{name} has no working folder yet."),
        fr: t("{name} n\u{2019}a pas encore de dossier de travail."),
    }
    TerminalAskSave "terminal.ask.save" {
        en: t("Save as the working folder of project {name}"),
        fr: t("Enregistrer comme dossier de travail du projet {name}"),
    }
    TerminalAskUnsaved "terminal.ask.unsaved" {
        en: t("If not saved, it is used until Acta quits."),
        fr: t("S\u{2019}il n\u{2019}est pas enregistré, il est utilisé jusqu\u{2019}à la fermeture d\u{2019}Acta."),
    }
    TerminalAskChoose "terminal.ask.choose" {
        en: t("Choose a folder\u{2026}"),
        fr: t("Choisir un dossier\u{2026}"),
    }
    TerminalMissingTitle "terminal.missing.title" {
        en: t("The working folder was not found"),
        fr: t("Le dossier de travail est introuvable"),
    }
    TerminalMissingBody "terminal.missing.body" {
        en: t("Claude Code was not started. This is the primary folder of epic {name}, and it is not on this computer:"),
        fr: t("Claude Code n\u{2019}a pas été lancé. C\u{2019}est le dossier principal de l\u{2019}epic {name}, et il n\u{2019}est pas sur cet ordinateur\u{202f}:"),
    }
    TerminalMissingLocate "terminal.missing.locate" {
        en: t("Locate folder\u{2026}"),
        fr: t("Localiser le dossier\u{2026}"),
    }
    TerminalLeftOut "terminal.left_out" {
        en: t("{folder} was not found and was left out of this session."),
        fr: t("{folder} est introuvable et n\u{2019}a pas été inclus dans cette session."),
    }
    TerminalLocated "terminal.located" {
        en: t("{folder} located. Restart the terminal to include it."),
        fr: t("{folder} localisé. Redémarrez le terminal pour l\u{2019}inclure."),
    }
    TerminalRestartTitle "terminal.restart.title" {
        en: t("Restart the terminal?"),
        fr: t("Redémarrer le terminal\u{202f}?"),
    }
    TerminalRestartBody "terminal.restart.body" {
        en: t("Claude Code is still running in it."),
        fr: t("Claude Code y est encore en cours d\u{2019}exécution."),
    }
    TerminalRestartConfirm "terminal.restart.confirm" {
        en: t("Restart"),
        fr: t("Redémarrer"),
    }
    TerminalCloseTitle "terminal.close.title" {
        en: t("Close the terminal?"),
        fr: t("Fermer le terminal\u{202f}?"),
    }
    TerminalCloseBody "terminal.close.body" {
        en: t("Claude Code is still running in it."),
        fr: t("Claude Code y est encore en cours d\u{2019}exécution."),
    }
    TerminalCloseConfirm "terminal.close.confirm" {
        en: t("Close terminal"),
        fr: t("Fermer le terminal"),
    }
    QuitTitle "quit.title" {
        en: t("Quit Acta?"),
        fr: t("Quitter Acta\u{202f}?"),
    }
    QuitBody "quit.body" {
        en: t("Claude Code is still running in the terminal of {project}."),
        fr: t("Claude Code est encore en cours d\u{2019}exécution dans le terminal de {project}."),
    }
    QuitConfirm "quit.confirm" {
        en: t("Quit"),
        fr: t("Quitter"),
    }
    ChatTitle "chat.title" {
        en: t("Chat"),
        fr: t("Chat"),
    }
    ChatOpenBrowser "chat.open_browser" {
        en: t("Open in browser"),
        fr: t("Ouvrir dans le navigateur"),
    }
    ChatReload "chat.reload" {
        en: t("Reload"),
        fr: t("Recharger"),
    }
    ChatHelp "chat.help" {
        en: t("Sign-in help"),
        fr: t("Aide à la connexion"),
    }
    ChatStartTitle "chat.start.title" {
        en: t("Chat with Claude beside your work"),
        fr: t("Discutez avec Claude à côté de votre travail"),
    }
    ChatStartBody "chat.start.body" {
        en: t("This panel shows claude.ai, the website, in your system\u{2019}s web view. You sign in with your own account, and Acta does not read or change anything on the page."),
        fr: t("Ce panneau affiche claude.ai, le site web, dans la vue web de votre système. Vous vous connectez avec votre propre compte, et Acta ne lit ni ne modifie rien sur la page."),
    }
    ChatStartMemory "chat.start.memory" {
        en: t("Once started, the web view uses about 400 MB of memory, also while the panel is hidden."),
        fr: t("Une fois lancée, la vue web utilise environ 400\u{202f}Mo de mémoire, même quand le panneau est masqué."),
    }
    ChatStartHere "chat.start.here" {
        en: t("Open claude.ai here"),
        fr: t("Ouvrir claude.ai ici"),
    }
    ChatStartBrowser "chat.start.browser" {
        en: t("Open in your browser"),
        fr: t("Ouvrir dans votre navigateur"),
    }
    ChatStarting "chat.starting" {
        en: t("Opening claude.ai\u{2026}"),
        fr: t("Ouverture de claude.ai\u{2026}"),
    }
    ChatSignin "chat.signin" {
        en: t("Sign in with email: the link in the message opens in your browser and shows a code; type that code here. Signing in with Google or Apple may be refused in this panel."),
        fr: t("Connectez-vous par e-mail\u{202f}: le lien du message s\u{2019}ouvre dans votre navigateur et affiche un code\u{202f}; saisissez ce code ici. La connexion avec Google ou Apple peut être refusée dans ce panneau."),
    }
    ChatHidden "chat.hidden" {
        en: t("The chat is hidden while this is open."),
        fr: t("Le chat est masqué tant que ceci est ouvert."),
    }
    ChatRuntimeTitle "chat.runtime.title" {
        en: t("The chat panel needs WebView2"),
        fr: t("Le panneau de chat a besoin de WebView2"),
    }
    ChatRuntimeBody "chat.runtime.body" {
        en: t("Microsoft\u{2019}s WebView2 runtime is not installed on this computer, so Acta cannot show claude.ai here. Everything else in Acta works."),
        fr: t("Le composant WebView2 de Microsoft n\u{2019}est pas installé sur cet ordinateur\u{202f}; Acta ne peut donc pas afficher claude.ai ici. Tout le reste d\u{2019}Acta fonctionne."),
    }
    ChatRuntimeGet "chat.runtime.get" {
        en: t("Get WebView2"),
        fr: t("Obtenir WebView2"),
    }
    ChatBrowser "chat.browser" {
        en: t("Open claude.ai in your browser"),
        fr: t("Ouvrir claude.ai dans votre navigateur"),
    }
    ChatFailedTitle "chat.failed.title" {
        en: t("claude.ai could not be opened here"),
        fr: t("claude.ai n\u{2019}a pas pu être ouvert ici"),
    }
    ChatFailedOffline "chat.failed.offline" {
        en: t("There is no connection."),
        fr: t("Il n\u{2019}y a pas de connexion."),
    }
    ChatSignout "chat.signout" {
        en: t("Sign out and clear chat data\u{2026}"),
        fr: t("Se déconnecter et effacer les données du chat\u{2026}"),
    }
    ChatSignoutTitle "chat.signout.title" {
        en: t("Sign out and clear chat data?"),
        fr: t("Se déconnecter et effacer les données du chat\u{202f}?"),
    }
    ChatSignoutBody "chat.signout.body" {
        en: t("This deletes the panel\u{2019}s cookies and site data from this computer. Your conversations stay in your Claude account."),
        fr: t("Cela supprime de cet ordinateur les cookies et les données de site du panneau. Vos conversations restent dans votre compte Claude."),
    }
    ChatSignoutConfirm "chat.signout.confirm" {
        en: t("Sign out and clear"),
        fr: t("Se déconnecter et effacer"),
    }
    MenuEdit "menu.edit" {
        en: t("Edit"),
        fr: t("Édition"),
    }
    MenuCut "menu.cut" {
        en: t("Cut"),
        fr: t("Couper"),
    }
    MenuCopy "menu.copy" {
        en: t("Copy"),
        fr: t("Copier"),
    }
    MenuPaste "menu.paste" {
        en: t("Paste"),
        fr: t("Coller"),
    }
    MenuSelectAll "menu.select_all" {
        en: t("Select All"),
        fr: t("Tout sélectionner"),
    }
    TabOpenNew "tab.open_new" {
        en: t("Open in new tab"),
        fr: t("Ouvrir dans un nouvel onglet"),
    }
    TabClose "tab.close" {
        en: t("Close tab"),
        fr: t("Fermer l\u{2019}onglet"),
    }
    TabCloseOthers "tab.close_others" {
        en: t("Close other tabs"),
        fr: t("Fermer les autres onglets"),
    }
    TabReopen "tab.reopen" {
        en: t("Reopen closed tab"),
        fr: t("Rouvrir l\u{2019}onglet fermé"),
    }
    PaletteRecent "palette.recent" {
        en: t("Recent"),
        fr: t("Récents"),
    }
    PaletteTopics "palette.topics" {
        en: t("Topics"),
        fr: t("Sujets"),
    }
    PaletteCommands "palette.commands" {
        en: t("Commands"),
        fr: t("Commandes"),
    }
    PaletteProjects "palette.projects" {
        en: t("Projects"),
        fr: t("Projets"),
    }
    PaletteHintOpen "palette.hint.open" {
        en: t("{keys} open"),
        fr: t("{keys} ouvrir"),
    }
    PaletteHintTab "palette.hint.tab" {
        en: t("{keys} open in new tab"),
        fr: t("{keys} ouvrir dans un nouvel onglet"),
    }
    PaletteHintClose "palette.hint.close" {
        en: t("{keys} close"),
        fr: t("{keys} fermer"),
    }
    PaletteNone "palette.none" {
        en: t("Nothing matches \"{query}\". {keys} creates a topic with this title."),
        fr: t("Aucun résultat pour «\u{202f}{query}\u{202f}». {keys} crée un sujet avec ce titre."),
    }
    CmdGoTopic "cmd.go_topic" {
        en: t("Go to topic\u{2026}"),
        fr: t("Aller au sujet\u{2026}"),
    }
    CmdCapture "cmd.capture" {
        en: t("Capture a note"),
        fr: t("Saisir une note"),
    }
    CmdNewTopic "cmd.new_topic" {
        en: t("New topic"),
        fr: t("Nouveau sujet"),
    }
    CmdAddEntry "cmd.add_entry" {
        en: t("Add entry"),
        fr: t("Ajouter une entrée"),
    }
    CmdGoInbox "cmd.go_inbox" {
        en: t("Go to inbox"),
        fr: t("Aller à la boîte de réception"),
    }
    CmdBack "cmd.back" {
        en: t("Back"),
        fr: t("Précédent"),
    }
    CmdForward "cmd.forward" {
        en: t("Forward"),
        fr: t("Suivant"),
    }
    CmdUndo "cmd.undo" {
        en: t("Undo"),
        fr: t("Annuler"),
    }
    CmdRedo "cmd.redo" {
        en: t("Redo"),
        fr: t("Rétablir"),
    }
    CmdZoom "cmd.zoom" {
        en: t("Zoom tool window"),
        fr: t("Agrandir la fenêtre d\u{2019}outil"),
    }
    CmdSetStatus "cmd.set_status" {
        en: t("Set status: {status}"),
        fr: t("Définir le statut\u{202f}: {status}"),
    }
    CmdSettings "cmd.settings" {
        en: t("Open settings"),
        fr: t("Ouvrir les paramètres"),
    }
    CmdTheme "cmd.theme" {
        en: t("Theme: {value}"),
        fr: t("Thème\u{202f}: {value}"),
    }
    CmdLanguage "cmd.language" {
        en: t("Language: {value}"),
        fr: t("Langue\u{202f}: {value}"),
    }
    CmdNewProject "cmd.new_project" {
        en: t("New project\u{2026}"),
        fr: t("Nouveau projet\u{2026}"),
    }
    CmdNewEpic "cmd.new_epic" {
        en: t("New epic\u{2026}"),
        fr: t("Nouvel epic\u{2026}"),
    }
    CmdSwitchProject "cmd.switch_project" {
        en: t("Switch project"),
        fr: t("Changer de projet"),
    }
    CmdToggleSidebar "cmd.toggle_sidebar" {
        en: t("Toggle sidebar"),
        fr: t("Afficher ou masquer la barre latérale"),
    }
    CmdToggleTerminal "cmd.toggle_terminal" {
        en: t("Toggle terminal"),
        fr: t("Afficher ou masquer le terminal"),
    }
    CmdToggleChat "cmd.toggle_chat" {
        en: t("Toggle chat"),
        fr: t("Afficher ou masquer le chat"),
    }
    CmdTerminalBottom "cmd.terminal_bottom" {
        en: t("Move terminal to bottom"),
        fr: t("Placer le terminal en bas"),
    }
    CmdTerminalRight "cmd.terminal_right" {
        en: t("Move terminal to right"),
        fr: t("Placer le terminal à droite"),
    }
    CmdChatBottom "cmd.chat_bottom" {
        en: t("Move chat to bottom"),
        fr: t("Placer le chat en bas"),
    }
    CmdChatRight "cmd.chat_right" {
        en: t("Move chat to right"),
        fr: t("Placer le chat à droite"),
    }
    CmdRestartTerminal "cmd.restart_terminal" {
        en: t("Restart terminal here"),
        fr: t("Redémarrer le terminal ici"),
    }
    CmdWidenSidebar "cmd.widen_sidebar" {
        en: t("Widen sidebar"),
        fr: t("Élargir la barre latérale"),
    }
    CmdNarrowSidebar "cmd.narrow_sidebar" {
        en: t("Narrow sidebar"),
        fr: t("Rétrécir la barre latérale"),
    }
    CmdWidenTerminal "cmd.widen_terminal" {
        en: t("Widen terminal"),
        fr: t("Élargir le terminal"),
    }
    CmdNarrowTerminal "cmd.narrow_terminal" {
        en: t("Narrow terminal"),
        fr: t("Rétrécir le terminal"),
    }
    CmdSetup "cmd.setup" {
        en: t("Set up Claude Code"),
        fr: t("Configurer Claude Code"),
    }
    CmdRestartUpdate "cmd.restart_update" {
        en: t("Restart to update"),
        fr: t("Redémarrer pour mettre à jour"),
    }
    CmdTerminalLarger "cmd.terminal_larger" {
        en: t("Increase terminal text size"),
        fr: t("Augmenter la taille du texte du terminal"),
    }
    CmdTerminalSmaller "cmd.terminal_smaller" {
        en: t("Decrease terminal text size"),
        fr: t("Réduire la taille du texte du terminal"),
    }
    CmdAutoMemoryOff "cmd.auto_memory_off" {
        en: t("Turn off Claude Code\u{2019}s own memory in attached folders"),
        fr: t("Désactiver la mémoire propre à Claude Code dans les dossiers rattachés"),
    }
    CmdAutoMemoryOn "cmd.auto_memory_on" {
        en: t("Leave Claude Code\u{2019}s own memory on in attached folders"),
        fr: t("Laisser active la mémoire propre à Claude Code dans les dossiers rattachés"),
    }
    CmdShortcuts "cmd.shortcuts" {
        en: t("Keyboard shortcuts"),
        fr: t("Raccourcis clavier"),
    }
    CmdKbOpen "cmd.kb_open" {
        en: t("Open document or memory\u{2026}"),
        fr: t("Ouvrir un document ou une mémoire\u{2026}"),
    }
    CmdKbGo "cmd.kb_go" {
        en: t("Go to knowledge base"),
        fr: t("Aller aux connaissances"),
    }
    CmdKbSearch "cmd.kb_search" {
        en: t("Search knowledge\u{2026}"),
        fr: t("Rechercher dans les connaissances\u{2026}"),
    }
    CmdKbReview "cmd.kb_review" {
        en: t("Review memories written by Claude Code"),
        fr: t("Revoir les mémoires écrites par Claude Code"),
    }
    CmdKbFromFile "cmd.kb_from_file" {
        en: t("New document from file\u{2026}"),
        fr: t("Nouveau document depuis un fichier\u{2026}"),
    }
    CmdKbToMemory "cmd.kb_to_memory" {
        en: t("Change kind to memory"),
        fr: t("Convertir en mémoire"),
    }
    CmdKbToDocument "cmd.kb_to_document" {
        en: t("Change kind to document"),
        fr: t("Convertir en document"),
    }
    CmdGoHeading "cmd.go_heading" {
        en: t("Go to heading\u{2026}"),
        fr: t("Aller au titre\u{2026}"),
    }
    CmdSaveFile "cmd.save_file" {
        en: t("Save as file\u{2026}"),
        fr: t("Enregistrer comme fichier\u{2026}"),
    }
    CmdExport "cmd.export" {
        en: t("Export\u{2026}"),
        fr: t("Exporter\u{2026}"),
    }
    CmdImport "cmd.import" {
        en: t("Import\u{2026}"),
        fr: t("Importer\u{2026}"),
    }
    CmdTrash "cmd.trash" {
        en: t("Open trash"),
        fr: t("Ouvrir la corbeille"),
    }
    CmdSnapshot "cmd.snapshot" {
        en: t("Restore from snapshot\u{2026}"),
        fr: t("Restaurer depuis un instantané\u{2026}"),
    }
    CmdFoldersAdd "cmd.folders_add" {
        en: t("Add working folder\u{2026}"),
        fr: t("Ajouter un dossier de travail\u{2026}"),
    }
    CmdFoldersShow "cmd.folders_show" {
        en: t("Show working folders"),
        fr: t("Afficher les dossiers de travail"),
    }
    CmdFolderPrimary "cmd.folder_primary" {
        en: t("Make primary folder"),
        fr: t("Définir comme dossier principal"),
    }
    CmdFolderUp "cmd.folder_up" {
        en: t("Move folder up"),
        fr: t("Monter le dossier"),
    }
    CmdFolderDown "cmd.folder_down" {
        en: t("Move folder down"),
        fr: t("Descendre le dossier"),
    }
    CmdFolderLocate "cmd.folder_locate" {
        en: t("Locate folder\u{2026}"),
        fr: t("Localiser le dossier\u{2026}"),
    }
    CmdFolderCopy "cmd.folder_copy" {
        en: t("Copy folder path"),
        fr: t("Copier le chemin du dossier"),
    }
    CmdFolderShowW "cmd.folder_show.w" {
        en: t("Show folder in Explorer"),
        fr: t("Afficher le dossier dans l\u{2019}Explorateur"),
    }
    CmdFolderShowM "cmd.folder_show.m" {
        en: t("Reveal folder in Finder"),
        fr: t("Afficher le dossier dans le Finder"),
    }
    CmdFolderShowL "cmd.folder_show.l" {
        en: t("Show folder in file manager"),
        fr: t("Afficher le dossier dans le gestionnaire de fichiers"),
    }
    CmdFolderRemoveTopic "cmd.folder_remove.topic" {
        en: t("Remove folder from this topic"),
        fr: t("Retirer le dossier de ce sujet"),
    }
    CmdFolderRemoveEpic "cmd.folder_remove.epic" {
        en: t("Remove folder from this epic"),
        fr: t("Retirer le dossier de cet epic"),
    }
    CmdFolderRemoveProject "cmd.folder_remove.project" {
        en: t("Remove folder from this project"),
        fr: t("Retirer le dossier de ce projet"),
    }
    CmdFoldersInclude "cmd.folders_include" {
        en: t("Include inherited folders"),
        fr: t("Inclure les dossiers hérités"),
    }
    CmdFoldersExclude "cmd.folders_exclude" {
        en: t("Stop including inherited folders"),
        fr: t("Ne plus inclure les dossiers hérités"),
    }
    CmdFoldersBulk "cmd.folders_bulk" {
        en: p("Apply the same change to {n} folder", "Apply the same change to {n} folders"),
        fr: p("Appliquer le même changement à {n} dossier", "Appliquer le même changement à {n} dossiers"),
    }
    SlashH1 "slash.h1" {
        en: t("Heading 1"),
        fr: t("Titre 1"),
    }
    SlashH2 "slash.h2" {
        en: t("Heading 2"),
        fr: t("Titre 2"),
    }
    SlashH3 "slash.h3" {
        en: t("Heading 3"),
        fr: t("Titre 3"),
    }
    SlashBullets "slash.bullets" {
        en: t("Bulleted list"),
        fr: t("Liste à puces"),
    }
    SlashNumbers "slash.numbers" {
        en: t("Numbered list"),
        fr: t("Liste numérotée"),
    }
    SlashTasks "slash.tasks" {
        en: t("Task list"),
        fr: t("Liste de tâches"),
    }
    SlashQuote "slash.quote" {
        en: t("Quote"),
        fr: t("Citation"),
    }
    SlashCode "slash.code" {
        en: t("Code block"),
        fr: t("Bloc de code"),
    }
    SlashTable "slash.table" {
        en: t("Table"),
        fr: t("Tableau"),
    }
    SlashDivider "slash.divider" {
        en: t("Divider"),
        fr: t("Séparateur"),
    }
    SettingsAppearance "settings.appearance" {
        en: t("Appearance"),
        fr: t("Apparence"),
    }
    SettingsTheme "settings.theme" {
        en: t("Theme"),
        fr: t("Thème"),
    }
    SettingsThemeSystem "settings.theme.system" {
        en: t("System"),
        fr: t("Système"),
    }
    SettingsThemeLight "settings.theme.light" {
        en: t("Light"),
        fr: t("Clair"),
    }
    SettingsThemeDark "settings.theme.dark" {
        en: t("Dark"),
        fr: t("Sombre"),
    }
    SettingsThemeHint "settings.theme.hint" {
        en: t("System follows your computer\u{2019}s setting."),
        fr: t("«\u{202f}Système\u{202f}» suit le réglage de votre ordinateur."),
    }
    SettingsLanguage "settings.language" {
        en: t("Language"),
        fr: t("Langue"),
    }
    SettingsLanguageHint "settings.language.hint" {
        en: t("Changes Acta\u{2019}s own text. What you and Claude Code write is never translated."),
        fr: t("Change les textes d\u{2019}Acta. Ce que vous et Claude Code écrivez n\u{2019}est jamais traduit."),
    }
    SettingsTerminalSize "settings.terminal_size" {
        en: t("Terminal text size"),
        fr: t("Taille du texte du terminal"),
    }
    SettingsFolders "settings.folders" {
        en: t("Working folders"),
        fr: t("Dossiers de travail"),
    }
    SettingsAutoMemory "settings.auto_memory" {
        en: t("Turn off Claude Code\u{2019}s own memory in attached folders"),
        fr: t("Désactiver la mémoire propre à Claude Code dans les dossiers rattachés"),
    }
    SettingsAutoMemoryHint "settings.auto_memory.hint" {
        en: t("When a folder is attached, Acta writes autoMemoryEnabled: false to its .claude/settings.local.json, so that what Claude Code learns is kept in Acta, where you can read it."),
        fr: t("Quand un dossier est rattaché, Acta écrit autoMemoryEnabled: false dans son .claude/settings.local.json, afin que ce que Claude Code apprend soit conservé dans Acta, où vous pouvez le lire."),
    }
    SettingsNotFound "settings.not_found" {
        en: t("Folders not found"),
        fr: t("Dossiers introuvables"),
    }
    SettingsClaude "settings.claude" {
        en: t("Claude Code"),
        fr: t("Claude Code"),
    }
    SettingsCheckCli "settings.check.cli" {
        en: t("Claude Code found"),
        fr: t("Claude Code trouvé"),
    }
    SettingsCheckFiles "settings.check.files" {
        en: t("Plugin files written"),
        fr: t("Fichiers du plugin écrits"),
    }
    SettingsCheckPlugin "settings.check.plugin" {
        en: t("Plugin installed"),
        fr: t("Plugin installé"),
    }
    SettingsCheckRule "settings.check.rule" {
        en: t("Permission rule added"),
        fr: t("Règle d\u{2019}autorisation ajoutée"),
    }
    SettingsCheckDone "settings.check.done" {
        en: t("Done"),
        fr: t("Fait"),
    }
    SettingsCheckMissing "settings.check.missing" {
        en: t("Missing"),
        fr: t("À faire"),
    }
    SettingsSetup "settings.setup" {
        en: t("Set up Claude Code"),
        fr: t("Configurer Claude Code"),
    }
    SettingsSetupAgain "settings.setup_again" {
        en: t("Run setup again"),
        fr: t("Relancer la configuration"),
    }
    SettingsSetupRemove "settings.setup_remove" {
        en: t("Remove from Claude Code"),
        fr: t("Retirer de Claude Code"),
    }
    SettingsSetupHint "settings.setup.hint" {
        en: t("Setup installs Acta\u{2019}s plugin in Claude Code and adds one permission line to ~/.claude/settings.json. Remove undoes both."),
        fr: t("La configuration installe le plugin d\u{2019}Acta dans Claude Code et ajoute une ligne d\u{2019}autorisation à ~/.claude/settings.json. Retirer annule les deux."),
    }
    SettingsData "settings.data" {
        en: t("Data"),
        fr: t("Données"),
    }
    SettingsExportLast "settings.export.last" {
        en: t("Last export: {days}"),
        fr: t("Dernier export\u{202f}: {days}"),
    }
    SettingsExportNever "settings.export.never" {
        en: t("Never exported"),
        fr: t("Jamais exporté"),
    }
    SettingsSnapshotsToday "settings.snapshots.today" {
        en: p("{n} snapshot kept; the newest is from today, {time}", "{n} snapshots kept; the newest is from today, {time}"),
        fr: p("{n} instantané conservé\u{202f}; le plus récent date d\u{2019}aujourd\u{2019}hui à {time}", "{n} instantanés conservés\u{202f}; le plus récent date d\u{2019}aujourd\u{2019}hui à {time}"),
    }
    SettingsSnapshotsOlder "settings.snapshots.older" {
        en: p("{n} snapshot kept; the newest is from {datetime}", "{n} snapshots kept; the newest is from {datetime}"),
        fr: p("{n} instantané conservé\u{202f}; le plus récent date du {datetime}", "{n} instantanés conservés\u{202f}; le plus récent date du {datetime}"),
    }
    SettingsTrashCount "settings.trash_count" {
        en: p("{n} item", "{n} items"),
        fr: p("{n} élément", "{n} éléments"),
    }
    SettingsDatabase "settings.database" {
        en: t("Database"),
        fr: t("Base de données"),
    }
    SettingsChat "settings.chat" {
        en: t("Chat"),
        fr: t("Chat"),
    }
    SettingsAbout "settings.about" {
        en: t("About"),
        fr: t("À propos"),
    }
    SettingsReleaseNotes "settings.release_notes" {
        en: t("Release notes"),
        fr: t("Notes de version"),
    }
    SettingsLicences "settings.licences" {
        en: t("Licences"),
        fr: t("Licences"),
    }
    StatusbarNotSetUp "statusbar.not_set_up" {
        en: t("Claude Code: not set up"),
        fr: t("Claude Code\u{202f}: non configuré"),
    }
    StatusbarNotFound "statusbar.not_found" {
        en: t("Claude Code: not found"),
        fr: t("Claude Code\u{202f}: introuvable"),
    }
    StatusbarNoFolder "statusbar.no_folder" {
        en: t("Claude Code: no folder for this project"),
        fr: t("Claude Code\u{202f}: aucun dossier pour ce projet"),
    }
    StatusbarReady "statusbar.ready" {
        en: t("Claude Code: ready"),
        fr: t("Claude Code\u{202f}: prêt"),
    }
    StatusbarLastWrote "statusbar.last_wrote" {
        en: t("Last wrote {when} in {topic}"),
        fr: t("Dernière écriture {when} dans {topic}"),
    }
    StatusbarVersion "statusbar.version" {
        en: t("Acta {version}"),
        fr: t("Acta {version}"),
    }
    UpdateReady "update.ready" {
        en: t("Update ready \u{2013} restart to apply"),
        fr: t("Mise à jour prête\u{a0}\u{2013} redémarrer pour l\u{2019}appliquer"),
    }
    UpdateDone "update.done" {
        en: t("Updated to {version}"),
        fr: t("Mise à jour {version} installée"),
    }
    UpdateFailed "update.failed" {
        en: t("Update failed"),
        fr: t("Échec de la mise à jour"),
    }
    UpdateManual "update.manual" {
        en: t("Download manually"),
        fr: t("Télécharger manuellement"),
    }
    UpdaterChecking "updater.checking" {
        en: t("Checking for updates\u{2026}"),
        fr: t("Recherche de mises à jour\u{2026}"),
    }
    UpdaterDownloading "updater.downloading" {
        en: t("Downloading update\u{2026}"),
        fr: t("Téléchargement de la mise à jour\u{2026}"),
    }
    UpdaterRestarting "updater.restarting" {
        en: t("Restarting\u{2026}"),
        fr: t("Redémarrage\u{2026}"),
    }
    ExportTitle "export.title" {
        en: t("Export"),
        fr: t("Exporter"),
    }
    ExportScopeAll "export.scope.all" {
        en: t("Everything"),
        fr: t("Tout"),
    }
    ExportScopeProject "export.scope.project" {
        en: t("This project"),
        fr: t("Ce projet"),
    }
    ExportWarning "export.warning" {
        en: t("The file is not encrypted. Anyone who can open it can read these projects. That is what lets you read it without Acta or its key."),
        fr: t("Le fichier n\u{2019}est pas chiffré. Toute personne pouvant l\u{2019}ouvrir peut lire ces projets. C\u{2019}est ce qui vous permet de le lire sans Acta ni sa clé."),
    }
    ExportPassphrase "export.passphrase" {
        en: t("Protect with a passphrase"),
        fr: t("Protéger par une phrase secrète"),
    }
    ExportPassphraseLost "export.passphrase.lost" {
        en: t("Acta cannot recover a lost passphrase."),
        fr: t("Acta ne peut pas retrouver une phrase secrète perdue."),
    }
    ExportChoose "export.choose" {
        en: t("Choose location\u{2026}"),
        fr: t("Choisir l\u{2019}emplacement\u{2026}"),
    }
    ExportDone "export.done" {
        en: p("Exported {n} project to {file}.", "Exported {n} projects to {file}."),
        fr: p("{n} projet exporté vers {file}.", "{n} projets exportés vers {file}."),
    }
    ExportShow "export.show" {
        en: t("Show in folder"),
        fr: t("Afficher dans le dossier"),
    }
    ExportSuggest "export.suggest" {
        en: t("Everything is stored on this computer only. An export is your copy if the system keychain is ever lost."),
        fr: t("Tout est stocké sur cet ordinateur uniquement. Un export vous sert de copie si le trousseau du système venait à être perdu."),
    }
    ImportTitle "import.title" {
        en: t("Import \"{file}\""),
        fr: t("Importer «\u{202f}{file}\u{202f}»"),
    }
    ImportMeta "import.meta" {
        en: t("Exported {date} · format {n} · {projects}"),
        fr: t("Exporté le {date} · format {n} · {projects}"),
    }
    ImportCreated "import.created" {
        en: t("Created"),
        fr: t("Créés"),
    }
    ImportUpdated "import.updated" {
        en: t("Updated"),
        fr: t("Mis à jour"),
    }
    ImportSkipped "import.skipped" {
        en: t("Skipped"),
        fr: t("Ignorés"),
    }
    ImportKindProjects "import.kind.projects" {
        en: t("Projects"),
        fr: t("Projets"),
    }
    ImportKindEpics "import.kind.epics" {
        en: t("Epics"),
        fr: t("Epics"),
    }
    ImportKindTopics "import.kind.topics" {
        en: t("Topics"),
        fr: t("Sujets"),
    }
    ImportKindEntries "import.kind.entries" {
        en: t("Entries"),
        fr: t("Entrées"),
    }
    ImportKindNotes "import.kind.notes" {
        en: t("Notes"),
        fr: t("Notes"),
    }
    ImportKindDocuments "import.kind.documents" {
        en: t("Documents"),
        fr: t("Documents"),
    }
    ImportKindMemories "import.kind.memories" {
        en: t("Memories"),
        fr: t("Mémoires"),
    }
    ImportSafe "import.safe" {
        en: t("Nothing here will be deleted. Skipped records are the same as, or older than, what Acta already has."),
        fr: t("Rien ne sera supprimé. Les éléments ignorés sont identiques à ceux qu\u{2019}Acta possède déjà, ou plus anciens."),
    }
    ImportShowChanges "import.show_changes" {
        en: t("Show what changes"),
        fr: t("Afficher ce qui change"),
    }
    ImportConfirm "import.confirm" {
        en: t("Import"),
        fr: t("Importer"),
    }
    ImportDone "import.done" {
        en: t("Imported: {created}, {updated}. A snapshot from before the import is kept."),
        fr: t("Import terminé\u{202f}: {created}, {updated}. Un instantané d\u{2019}avant l\u{2019}import est conservé."),
    }
    ImportCountCreated "import.count.created" {
        en: t("{n} created"),
        fr: p("{n} créé", "{n} créés"),
    }
    ImportCountUpdated "import.count.updated" {
        en: t("{n} updated"),
        fr: t("{n} mis à jour"),
    }
    ImportFolders "import.folders" {
        en: p("{n} working folder was not found on this computer. Locate it from its project, epic or topic.", "{n} working folders were not found on this computer. Locate each one from its project, epic or topic."),
        fr: p("{n} dossier de travail est introuvable sur cet ordinateur. Localisez-le depuis son projet, son epic ou son sujet.", "{n} dossiers de travail sont introuvables sur cet ordinateur. Localisez chacun depuis son projet, son epic ou son sujet."),
    }
    ImportNewer "import.newer" {
        en: t("This file was made by a newer Acta (format {n}). Update Acta, then import it."),
        fr: t("Ce fichier a été créé par une version plus récente d\u{2019}Acta (format {n}). Mettez Acta à jour, puis importez-le."),
    }
    ImportNotActa "import.not_acta" {
        en: t("This is not an Acta export."),
        fr: t("Ce n\u{2019}est pas un export Acta."),
    }
    ImportWrongPassphrase "import.wrong_passphrase" {
        en: t("Wrong passphrase."),
        fr: t("Phrase secrète incorrecte."),
    }
    TrashHint "trash.hint" {
        en: t("Deleted things stay here for 30 days."),
        fr: t("Les éléments supprimés restent ici 30 jours."),
    }
    TrashBy "trash.by" {
        en: t("by {author}"),
        fr: t("par {author}"),
    }
    TrashProjectContent "trash.project_content" {
        en: t("{topics}, {notes}"),
        fr: t("{topics}, {notes}"),
    }
    TrashRestoredWithEpic "trash.restored_with_epic" {
        en: t("Topic and its epic restored."),
        fr: t("Sujet et son epic restaurés."),
    }
    TrashRestoredItem "trash.restored" {
        en: t("Item restored."),
        fr: t("Élément restauré."),
    }
    TrashDays "trash.days" {
        en: p("{n} day left", "{n} days left"),
        fr: p("{n} jour restant", "{n} jours restants"),
    }
    TrashEmpty "trash.empty" {
        en: t("Trash is empty."),
        fr: t("La corbeille est vide."),
    }
    SnapshotDaily "snapshot.daily" {
        en: t("Daily"),
        fr: t("Quotidien"),
    }
    SnapshotWeekly "snapshot.weekly" {
        en: t("Weekly"),
        fr: t("Hebdomadaire"),
    }
    SnapshotBeforeUpdate "snapshot.before_update" {
        en: t("Before update to {version}"),
        fr: t("Avant la mise à jour vers {version}"),
    }
    SnapshotBeforeImport "snapshot.before_import" {
        en: t("Before import"),
        fr: t("Avant l\u{2019}import"),
    }
    SnapshotBeforeRestore "snapshot.before_restore" {
        en: t("Before restore"),
        fr: t("Avant la restauration"),
    }
    SnapshotConfirm "snapshot.confirm" {
        en: t("Restore and restart"),
        fr: t("Restaurer et redémarrer"),
    }
    SnapshotTitle "snapshot.title" {
        en: t("Restore the snapshot from {date}?"),
        fr: t("Restaurer l\u{2019}instantané du {date}\u{202f}?"),
    }
    SnapshotBody "snapshot.body" {
        en: t("Acta will restart with its data as of {datetime}. Everything as it is now is saved as a snapshot first, so you can come back."),
        fr: t("Acta redémarrera avec ses données du {datetime}. La situation actuelle est d\u{2019}abord enregistrée dans un instantané\u{202f}: vous pourrez y revenir."),
    }
    SnapshotRestored "snapshot.restored" {
        en: t("Restored the snapshot from {datetime}."),
        fr: t("Instantané du {datetime} restauré."),
    }
    SnapshotMigration "snapshot.migration" {
        en: t("The update to {version} could not convert the database. Acta put back the snapshot from just before; nothing was lost."),
        fr: t("La mise à jour vers {version} n\u{2019}a pas pu convertir la base de données. Acta a restauré l\u{2019}instantané pris juste avant\u{202f}; rien n\u{2019}a été perdu."),
    }
    KeylossTitle "keyloss.title" {
        en: t("Acta cannot open its database"),
        fr: t("Acta ne peut pas ouvrir sa base de données"),
    }
    KeylossBody "keyloss.body" {
        en: t("The database is on this computer and intact, but its key is missing from {keychain}. This usually follows a system reinstall, a new user profile or a move to another machine."),
        fr: t("La base de données est sur cet ordinateur et intacte, mais sa clé est absente {keychain}. Cela arrive généralement après une réinstallation du système, la création d\u{2019}un nouveau profil utilisateur ou un changement de machine."),
    }
    KeylossBodyM "keyloss.body.m" {
        en: t("The database is on this computer and intact, but its key file is missing from Acta\u{2019}s data folder. This usually follows a restore from a backup that left it out, a new user profile or a move to another machine."),
        fr: t("La base de données est sur cet ordinateur et intacte, mais son fichier de clé manque dans le dossier de données d\u{2019}Acta. Cela suit en général une restauration de sauvegarde qui l\u{2019}a omis, un nouveau profil utilisateur ou un changement de machine."),
    }
    KeychainW "keychain.w" {
        en: t("Windows Credential Manager"),
        fr: t("du Gestionnaire d\u{2019}identification Windows"),
    }
    KeychainM "keychain.m" {
        en: t("the macOS Keychain"),
        fr: t("du trousseau macOS"),
    }
    KeychainL "keychain.l" {
        en: t("the Secret Service keyring"),
        fr: t("du service Secret Service"),
    }
    KeylossUntouched "keyloss.untouched" {
        en: t("Acta has not changed the file:"),
        fr: t("Acta n\u{2019}a pas modifié le fichier\u{202f}:"),
    }
    KeylossImport "keyloss.import" {
        en: t("Import an export\u{2026}"),
        fr: t("Importer un export\u{2026}"),
    }
    KeylossLimit "keyloss.limit" {
        en: t("Without the key or an export, the content cannot be read. You can start empty; the file is kept as {file}."),
        fr: t("Sans la clé ni export, le contenu ne peut pas être lu. Vous pouvez repartir de zéro\u{202f}; le fichier est conservé sous le nom {file}."),
    }
    KeylossStartEmpty "keyloss.start_empty" {
        en: t("Start empty"),
        fr: t("Repartir de zéro"),
    }
    OpenfailTitle "openfail.title" {
        en: t("Acta could not open its database"),
        fr: t("Acta n\u{2019}a pas pu ouvrir sa base de données"),
    }
    OpenfailBody "openfail.body" {
        en: t("Something on this computer kept Acta from reading its data. Nothing was changed. Try again; if it keeps failing, the log in the data folder says what failed."),
        fr: t("Quelque chose sur cet ordinateur empêche Acta de lire ses données. Rien n\u{2019}a été modifié. Réessayez\u{202f}; si l\u{2019}échec se répète, le journal du dossier de données indique la cause."),
    }
    OpenfailNodir "openfail.nodir" {
        en: t("Acta could not find where to keep its data on this computer."),
        fr: t("Acta ne trouve pas où conserver ses données sur cet ordinateur."),
    }
    FirstrunTitle "firstrun.title" {
        en: t("Start with a project"),
        fr: t("Commencez par un projet"),
    }
    FirstrunBody "firstrun.body" {
        en: t("A project is a folder you work on with Claude Code. Acta keeps its topics, decisions and notes next to the work."),
        fr: t("Un projet est un dossier sur lequel vous travaillez avec Claude Code. Acta garde ses sujets, ses décisions et ses notes à côté du travail."),
    }
    FirstrunChoose "firstrun.choose" {
        en: t("Choose a folder"),
        fr: t("Choisir un dossier"),
    }
    FirstrunThen "firstrun.then" {
        en: t("Then"),
        fr: t("Ensuite"),
    }
    FirstrunStep1 "firstrun.step1" {
        en: t("Connect Claude Code"),
        fr: t("Connecter Claude Code"),
    }
    FirstrunStep2 "firstrun.step2" {
        en: t("Capture a first note"),
        fr: t("Saisir une première note"),
    }
    FirstrunStep3 "firstrun.step3" {
        en: t("Open the terminal"),
        fr: t("Ouvrir le terminal"),
    }
    ShortcutsFilter "shortcuts.filter" {
        en: t("Filter shortcuts"),
        fr: t("Filtrer les raccourcis"),
    }
    ShortcutsNone "shortcuts.none" {
        en: t("No shortcut matches \"{query}\"."),
        fr: t("Aucun raccourci ne correspond à «\u{202f}{query}\u{202f}»."),
    }
    ShortcutsGroupGeneral "shortcuts.group.general" {
        en: t("General"),
        fr: t("Général"),
    }
    ShortcutsGroupTabs "shortcuts.group.tabs" {
        en: t("Tabs and tool windows"),
        fr: t("Onglets et fenêtres d\u{2019}outils"),
    }
    ShortcutsGroupLists "shortcuts.group.lists" {
        en: t("Lists and tree"),
        fr: t("Listes et arborescence"),
    }
    ShortcutsGroupEditors "shortcuts.group.editors" {
        en: t("Editors"),
        fr: t("Éditeurs"),
    }
    ShortcutsGroupTerminal "shortcuts.group.terminal" {
        en: t("Terminal and chat"),
        fr: t("Terminal et chat"),
    }
}
