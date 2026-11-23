//! Generated from the copy catalogue of docs/design/catalogue.md 10.4 (every row, in
//! table order). Edit docs/design/catalogue.md first, then this file; the tests in
//! `i18n` check the two agree.
//!
//! Typography is applied here, not in the table: U+2026 for "...", the
//! typographic apostrophe, and in French U+00A0 before :, U+202F before
//! ; ! ? » and after « and before units.

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
    TreeRename "tree.rename" {
        en: t("Rename"),
        fr: t("Renommer"),
    }
    TreeSetStatus "tree.set_status" {
        en: t("Set status"),
        fr: t("Changer le statut"),
    }
    TreeMoveEpic "tree.move_epic" {
        en: t("Move to epic"),
        fr: t("Déplacer vers un epic"),
    }
    TreeUnread "tree.unread" {
        en: t("New changes by Claude Code"),
        fr: t("Nouvelles modifications de Claude Code"),
    }
    TreeEmpty "tree.empty" {
        en: t("No projects yet."),
        fr: t("Aucun projet pour l\u{2019}instant."),
    }
    TreeCollapseAll "tree.collapse_all" {
        en: t("Collapse all"),
        fr: t("Tout replier"),
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
    EventDueSet "event.due_set" {
        en: t("{author} set the due date to {date}"),
        fr: t("{author} a fixé l\u{2019}échéance au {date}"),
    }
    EventDueCleared "event.due_cleared" {
        en: t("{author} cleared the due date"),
        fr: t("{author} a supprimé l\u{2019}échéance"),
    }
    EventReportSet "event.report_set" {
        en: t("{author} noted to report to {name}"),
        fr: t("{author} a noté de rapporter à : {name}"),
    }
    EventReported "event.reported" {
        en: t("{author} marked it reported to {name}"),
        fr: t("{author} l’a marqué comme rapporté à : {name}"),
    }
    EventReportUnmarked "event.report_unmarked" {
        en: t("{author} marked it not reported"),
        fr: t("{author} l’a marqué comme non rapporté"),
    }
    EventReportCleared "event.report_cleared" {
        en: t("{author} removed Report to"),
        fr: t("{author} a retiré « À rapporter à »"),
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
    PropDue "prop.due" {
        en: t("Due"),
        fr: t("Échéance"),
    }
    PropReportTo "prop.report_to" {
        en: t("Report to"),
        fr: t("À rapporter à"),
    }
    PropAddDue "prop.add_due" {
        en: t("Due date"),
        fr: t("Échéance"),
    }
    PropAddReportTo "prop.add_report_to" {
        en: t("Report to"),
        fr: t("À rapporter à"),
    }
    MetaRecord "meta.record" {
        en: t("{id} \u{b7} Updated {when} by {author}"),
        fr: t("{id} \u{b7} Modifié {when} par {author}"),
    }
    MetaUpdated "meta.updated" {
        en: t("Updated {when} by {author}"),
        fr: t("Modifié {when} par {author}"),
    }
    MetaCopyId "meta.copy_id" {
        en: t("Copy id"),
        fr: t("Copier l\u{2019}identifiant"),
    }
    DueNone "due.none" {
        en: t("None"),
        fr: t("Aucune"),
    }
    DueToday "due.today" {
        en: t("Today"),
        fr: t("Aujourd\u{2019}hui"),
    }
    DueTomorrow "due.tomorrow" {
        en: t("Tomorrow"),
        fr: t("Demain"),
    }
    DueInDays "due.in_days" {
        en: p("In {n} day", "In {n} days"),
        fr: p("Dans {n} jour", "Dans {n} jours"),
    }
    DueOverdue "due.overdue" {
        en: p("Overdue by {n} day", "Overdue by {n} days"),
        fr: p("En retard de {n} jour", "En retard de {n} jours"),
    }
    DueTipToday "due.tip.today" {
        en: t("Due today"),
        fr: t("Échéance aujourd\u{2019}hui"),
    }
    DueTipTomorrow "due.tip.tomorrow" {
        en: t("Due tomorrow"),
        fr: t("Échéance demain"),
    }
    DueTipInDays "due.tip.in_days" {
        en: p("Due {date}, in {n} day", "Due {date}, in {n} days"),
        fr: p("Échéance le {date}, dans {n} jour", "Échéance le {date}, dans {n} jours"),
    }
    DueTipOverdue "due.tip.overdue" {
        en: p("Overdue by {n} day, due {date}", "Overdue by {n} days, due {date}"),
        fr: p("En retard de {n} jour, échéance le {date}", "En retard de {n} jours, échéance le {date}"),
    }
    DueTipDate "due.tip.date" {
        en: t("Due {date}"),
        fr: t("Échéance le {date}"),
    }
    DuePickerPlaceholder "due.picker.placeholder" {
        en: t("fri, in 3d, 15/12, 2026-12-15"),
        fr: t("ven., dans 3 j, 15/12, 2026-12-15"),
    }
    DuePickerInvalid "due.picker.invalid" {
        en: t("Not a date. Try fri, in 3d, 15/12 or 2026-12-15."),
        fr: t("Date non reconnue. Essayez ven., dans 3 j, 15/12 ou 2026-12-15."),
    }
    DuePickerRange "due.picker.range" {
        en: t("Choose a date between 2000 and 2099."),
        fr: t("Choisissez une date entre 2000 et 2099."),
    }
    DuePickerNone "due.picker.none" {
        en: t("No due date"),
        fr: t("Aucune échéance"),
    }
    DuePickerPrev "due.picker.prev" {
        en: t("Previous month"),
        fr: t("Mois précédent"),
    }
    DuePickerNext "due.picker.next" {
        en: t("Next month"),
        fr: t("Mois suivant"),
    }
    PropProgressValue "prop.progress.value" {
        en: p_by("total", "{done} of {total} topic done", "{done} of {total} topics done"),
        fr: p_by("done", "{done} sujet terminé sur {total}", "{done} sujets terminés sur {total}"),
    }
    ProgressNone "progress.none" {
        en: t("No topics yet"),
        fr: t("Aucun sujet"),
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
    SectionQuestions "section.questions" {
        en: t("Questions"),
        fr: t("Questions"),
    }
    SectionKnowledge "section.knowledge" {
        en: t("Knowledge"),
        fr: t("Connaissances"),
    }
    OverviewTitle "overview.title" {
        en: t("Overview"),
        fr: t("Vue d\u{2019}ensemble"),
    }
    OverviewClear "overview.clear" {
        en: t("Nothing needs attention."),
        fr: t("Rien ne demande votre attention."),
    }
    OverviewMore "overview.more" {
        en: t("More"),
        fr: t("Plus"),
    }
    OverviewLess "overview.less" {
        en: t("Less"),
        fr: t("Moins"),
    }
    OverviewAttention "overview.attention" {
        en: t("Needs attention"),
        fr: t("À traiter"),
    }
    OverviewBlocked "overview.blocked" {
        en: p("{n} blocked topic", "{n} blocked topics"),
        fr: p("{n} sujet bloqué", "{n} sujets bloqués"),
    }
    OverviewOverdue "overview.overdue" {
        en: p("{n} overdue topic", "{n} overdue topics"),
        fr: p("{n} sujet en retard", "{n} sujets en retard"),
    }
    OverviewIdle "overview.idle" {
        en: p("{n} active topic idle for 7 days", "{n} active topics idle for 7 days"),
        fr: p("{n} sujet en cours inactif depuis 7 jours", "{n} sujets en cours inactifs depuis 7 jours"),
    }
    OverviewInbox "overview.inbox" {
        en: p("{n} note in the inbox for {days} days", "{n} notes in the inbox, oldest {days} days"),
        fr: p("{n} note dans la boîte de réception depuis {days} jours", "{n} notes dans la boîte de réception, la plus ancienne depuis {days} jours"),
    }
    OverviewWeek "overview.week" {
        en: t("Last 7 days"),
        fr: t("7 derniers jours"),
    }
    OverviewWeekDone "overview.week.done" {
        en: p("{n} topic done", "{n} topics done"),
        fr: p("{n} sujet terminé", "{n} sujets terminés"),
    }
    OverviewWeekDecisions "overview.week.decisions" {
        en: p("{n} decision", "{n} decisions"),
        fr: p("{n} décision", "{n} décisions"),
    }
    OverviewWeekNew "overview.week.new" {
        en: p("{n} new topic", "{n} new topics"),
        fr: p("{n} nouveau sujet", "{n} nouveaux sujets"),
    }
    OverviewWeekNone "overview.week.none" {
        en: t("Nothing done, decided or opened this week. Last activity {when}."),
        fr: t("Rien de terminé, décidé ni ouvert cette semaine. Dernière activité {when}."),
    }
    OverviewEpics "overview.epics" {
        en: t("Epics"),
        fr: t("Epics"),
    }
    OverviewEpicsMore "overview.epics.more" {
        en: p("{n} more epic", "{n} more epics"),
        fr: p("{n} autre epic", "{n} autres epics"),
    }
    OverviewEpicsComplete "overview.epics.complete" {
        en: p("{n} complete epic", "{n} complete epics"),
        fr: p("{n} epic terminé", "{n} epics terminés"),
    }
    OverviewMemories "overview.memories" {
        en: p("{n} memory not updated for 90 days", "{n} memories not updated for 90 days"),
        fr: p("{n} mémoire non mise à jour depuis 90 jours", "{n} mémoires non mises à jour depuis 90 jours"),
    }
    OverviewQuestionsLate "overview.questions_late" {
        en: p("{n} question past its answer date", "{n} questions past their answer date"),
        fr: p("{n} question dont la date de réponse est passée", "{n} questions dont la date de réponse est passée"),
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
    DescriptionPlaceholderEpic "description.placeholder.epic" {
        en: t("What is this epic about?"),
        fr: t("De quoi parle cet epic\u{202f}?"),
    }
    DescriptionPlaceholderProject "description.placeholder.project" {
        en: t("What is this project about?"),
        fr: t("De quoi parle ce projet\u{202f}?"),
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
    ComposerHint "composer.hint" {
        en: t("{keys} to save"),
        fr: t("{keys} pour enregistrer"),
    }
    TimelineNew "timeline.new" {
        en: p("{n} new entry", "{n} new entries"),
        fr: p("{n} nouvelle entrée", "{n} nouvelles entrées"),
    }
    TimelineEmpty "timeline.empty" {
        en: t("No entries yet. Claude Code logs progress here as it works."),
        fr: t("Aucune entrée pour l\u{2019}instant. Claude Code consigne ici l\u{2019}avancement au fil du travail."),
    }
    EntryCopyMarkdown "entry.copy_markdown" {
        en: t("Copy as Markdown"),
        fr: t("Copier en Markdown"),
    }
    EntryChangeType "entry.change_type" {
        en: t("Change type"),
        fr: t("Changer le type"),
    }
    EntryEdited "entry.edited" {
        en: t("edited"),
        fr: t("modifiée"),
    }
    EntryEditedBy "entry.edited_by" {
        en: t("edited by {name}"),
        fr: t("modifiée par {name}"),
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
        en: t("Capture a note, or create a topic."),
        fr: t("Saisissez une note ou créez un sujet."),
    }
    TopicEmptyHeading "topic.empty.heading" {
        en: t("No topics yet"),
        fr: t("Aucun sujet pour l’instant"),
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
        fr: t("Non annulé\u{a0}: cela a été modifié depuis."),
    }
    RedoChanged "redo.changed" {
        en: t("Not redone: it was changed since."),
        fr: t("Non rétabli\u{a0}: cela a été modifié depuis."),
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
    TableDue "table.due" {
        en: t("Due"),
        fr: t("Échéance"),
    }
    TableQuestions "table.questions" {
        en: t("Questions"),
        fr: t("Questions"),
    }
    TableQuestionsTip "table.questions_tip" {
        en: p("{n} open question", "{n} open questions"),
        fr: p("{n} question en suspens", "{n} questions en suspens"),
    }
    TableQuestionsTipOverdue "table.questions_tip_overdue" {
        en: p("1 open question, overdue", "{n} open questions, {late} overdue"),
        fr: p("1 question en suspens, en retard", "{n} questions en suspens, dont {late} en retard"),
    }
    TableReportTo "table.report_to" {
        en: t("Report to"),
        fr: t("À rapporter à"),
    }
    TableFilterOverdue "table.filter.overdue" {
        en: t("Overdue"),
        fr: t("En retard"),
    }
    TableFilterWeek "table.filter.week" {
        en: t("Next 7 days"),
        fr: t("7 prochains jours"),
    }
    FilterBlocked "filter.blocked" {
        en: t("Blocked"),
        fr: t("Bloqués"),
    }
    FilterIdle "filter.idle" {
        en: t("No activity for 7 days"),
        fr: t("Sans activité depuis 7 jours"),
    }
    FilterDoneWeek "filter.done_week" {
        en: t("Done in the last 7 days"),
        fr: t("Terminés ces 7 derniers jours"),
    }
    FilterDecidedWeek "filter.decided_week" {
        en: t("Decided in the last 7 days"),
        fr: t("Décidés ces 7 derniers jours"),
    }
    FilterCreatedWeek "filter.created_week" {
        en: t("Created in the last 7 days"),
        fr: t("Créés ces 7 derniers jours"),
    }
    FilterStaleMemories "filter.stale_memories" {
        en: t("Not updated for 90 days"),
        fr: t("Non mises à jour depuis 90 jours"),
    }
    FilterClear "filter.clear" {
        en: t("Clear filter"),
        fr: t("Effacer le filtre"),
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
    InboxDismissed "inbox.dismissed" {
        en: t("Dismissed"),
        fr: t("Écartée"),
    }
    InboxByClaude "inbox.by_claude" {
        en: t("by Claude Code"),
        fr: t("par Claude Code"),
    }
    InboxMakeTopic "inbox.make_topic" {
        en: t("Make topic"),
        fr: t("Créer un sujet"),
    }
    InboxFileTo "inbox.file_to" {
        en: t("File\u{2026}"),
        fr: t("Classer\u{2026}"),
    }
    InboxPickNewEpic "inbox.pick.new_epic" {
        en: t("New epic"),
        fr: t("Nouvel epic"),
    }
    InboxPickInEpic "inbox.pick.in_epic" {
        en: t("New topic in {path}"),
        fr: t("Nouveau sujet dans {path}"),
    }
    InboxPickAsEntry "inbox.pick.as_entry" {
        en: t("Entry in {path}"),
        fr: t("Entrée dans {path}"),
    }
    InboxEmpty "inbox.empty" {
        en: t("Inbox is empty. Captured notes wait here until they are filed."),
        fr: t("La boîte de réception est vide. Les notes saisies attendent ici d\u{2019}être classées."),
    }
    InboxFiledEmpty "inbox.filed_empty" {
        en: t("No filed notes yet."),
        fr: t("Aucune note classée pour l\u{2019}instant."),
    }
    AttachButton "attach.button" {
        en: t("Attach files\u{2026}"),
        fr: t("Joindre des fichiers\u{2026}"),
    }
    AttachDrop "attach.drop" {
        en: t("Drop to attach"),
        fr: t("Déposer pour joindre"),
    }
    AttachAdding "attach.adding" {
        en: t("Adding\u{2026} {percent}"),
        fr: t("Ajout\u{2026} {percent}"),
    }
    AttachPastedName "attach.pasted_name" {
        en: t("Pasted image {date} {time}.png"),
        fr: t("Image collée {date} {time}.png"),
    }
    AttachTooLarge "attach.too_large" {
        en: t("{name} is {size}; the limit is 25 MB per file."),
        fr: t("{name} pèse {size}\u{202f}; la limite est de 25\u{202f}Mo par fichier."),
    }
    AttachTooMany "attach.too_many" {
        en: t("20 files is the most a note, an entry or a description can hold."),
        fr: t("Une note, une entrée ou une description contient 20 fichiers au plus."),
    }
    AttachReadFailed "attach.read_failed" {
        en: t("Could not read {name}. It may have moved or be in use."),
        fr: t("Impossible de lire {name}. Il a peut-être été déplacé, ou il est utilisé."),
    }
    AttachLargeNotice "attach.large_notice" {
        en: t("Files over 10 MB are also kept in every snapshot."),
        fr: t("Les fichiers de plus de 10\u{202f}Mo sont aussi conservés dans chaque instantané."),
    }
    AttachCount "attach.count" {
        en: p("{n} file", "{n} files"),
        fr: p("{n} fichier", "{n} fichiers"),
    }
    AttachView "attach.view" {
        en: t("View"),
        fr: t("Afficher"),
    }
    AttachSaveAs "attach.save_as" {
        en: t("Save as\u{2026}"),
        fr: t("Enregistrer sous\u{2026}"),
    }
    AttachSaved "attach.saved" {
        en: t("Saved to {folder}."),
        fr: t("Enregistré dans {folder}."),
    }
    AttachShowInFolder "attach.show_in_folder" {
        en: t("Show in folder"),
        fr: t("Afficher dans le dossier"),
    }
    AttachSaveFailed "attach.save_failed" {
        en: t("Could not save {name}. {reason}"),
        fr: t("Impossible d\u{2019}enregistrer {name}. {reason}"),
    }
    AttachRemove "attach.remove" {
        en: t("Remove"),
        fr: t("Retirer"),
    }
    AttachRemoved "attach.removed" {
        en: t("File removed."),
        fr: t("Fichier retiré."),
    }
    AttachRestored "attach.restored" {
        en: t("File restored."),
        fr: t("Fichier restauré."),
    }
    AttachMovedTo "attach.moved_to" {
        en: t("Moved to {id}"),
        fr: t("Déplacés vers {id}"),
    }
    AttachPosition "attach.position" {
        en: t("{i} of {n}"),
        fr: t("{i} sur {n}"),
    }
    AttachPrev "attach.prev" {
        en: t("Previous file"),
        fr: t("Fichier précédent"),
    }
    AttachNext "attach.next" {
        en: t("Next file"),
        fr: t("Fichier suivant"),
    }
    AttachFit "attach.fit" {
        en: t("Fit"),
        fr: t("Ajuster"),
    }
    AttachActual "attach.actual" {
        en: t("Actual size"),
        fr: t("Taille réelle"),
    }
    AttachTruncated "attach.truncated" {
        en: t("First 1 MB shown. Save the file to see all of it."),
        fr: t("Premier Mo affiché. Enregistrez le fichier pour le voir en entier."),
    }
    AttachGone "attach.gone" {
        en: t("The file was removed while the editor was open; add it again."),
        fr: t("Le fichier a été retiré pendant que l\u{2019}éditeur était ouvert\u{202f}; ajoutez-le de nouveau."),
    }
    VoiceButton "voice.button" {
        en: t("Dictate"),
        fr: t("Dicter"),
    }
    VoiceButtonSetup "voice.button_setup" {
        en: t("Dictate\u{2026}"),
        fr: t("Dicter\u{2026}"),
    }
    VoiceButtonDownloading "voice.button_downloading" {
        en: t("Dictation is downloading its model: {percent}"),
        fr: t("La dictée télécharge son modèle\u{a0}: {percent}"),
    }
    VoiceStop "voice.stop" {
        en: t("Stop dictating"),
        fr: t("Arrêter la dictée"),
    }
    VoiceStarting "voice.starting" {
        en: t("Starting\u{2026}"),
        fr: t("Démarrage\u{2026}"),
    }
    VoiceListening "voice.listening" {
        en: t("Listening"),
        fr: t("À l\u{2019}écoute"),
    }
    VoiceWriting "voice.writing" {
        en: t("Writing text\u{2026}"),
        fr: t("Écriture du texte\u{2026}"),
    }
    VoiceStoppedSilence "voice.stopped_silence" {
        en: t("Dictation stopped after a minute of silence."),
        fr: t("Dictée arrêtée après une minute de silence."),
    }
    VoiceStoppedLimit "voice.stopped_limit" {
        en: t("Dictation stops after 10 minutes. Start again to go on."),
        fr: t("La dictée s\u{2019}arrête après 10 minutes. Recommencez pour continuer."),
    }
    VoiceNothingHeard "voice.nothing_heard" {
        en: t("Nothing was heard. Check the microphone in Settings > Voice."),
        fr: t("Rien n\u{2019}a été entendu. Vérifiez le micro dans Paramètres > Voix."),
    }
    VoiceSettingsLink "voice.settings_link" {
        en: t("Voice settings"),
        fr: t("Paramètres de la voix"),
    }
    VoiceSlow "voice.slow" {
        en: t("Dictation is slow on this computer. A smaller model is faster."),
        fr: t("La dictée est lente sur cet ordinateur. Un modèle plus petit est plus rapide."),
    }
    VoiceErrorBlocked "voice.error.blocked" {
        en: t("Acta is not allowed to use the microphone."),
        fr: t("Acta n\u{2019}est pas autorisé à utiliser le micro."),
    }
    VoiceErrorOpenPrivacy "voice.error.open_privacy" {
        en: t("Open privacy settings"),
        fr: t("Ouvrir les paramètres de confidentialité"),
    }
    VoiceErrorOpenSystemSettings "voice.error.open_system_settings" {
        en: t("Open System Settings"),
        fr: t("Ouvrir Réglages Système"),
    }
    VoiceErrorNoDevice "voice.error.no_device" {
        en: t("No microphone was found."),
        fr: t("Aucun micro n\u{2019}a été trouvé."),
    }
    VoiceErrorDeviceLost "voice.error.device_lost" {
        en: t("The microphone stopped. What was heard is kept."),
        fr: t("Le micro s\u{2019}est arrêté. Ce qui a été entendu est conservé."),
    }
    VoiceErrorModel "voice.error.model" {
        en: t("The dictation model is missing or damaged."),
        fr: t("Le modèle de dictée est absent ou endommagé."),
    }
    VoiceErrorDownloadAgain "voice.error.download_again" {
        en: t("Download again"),
        fr: t("Télécharger à nouveau"),
    }
    VoiceErrorEngine "voice.error.engine" {
        en: t("Dictation stopped unexpectedly. Your text is safe."),
        fr: t("La dictée s\u{2019}est arrêtée de façon inattendue. Votre texte est intact."),
    }
    VoiceErrorSpeechBlocked "voice.error.speech_blocked" {
        en: t("Acta is not allowed to use the macOS recogniser."),
        fr: t("Acta n\u{2019}est pas autorisé à utiliser la reconnaissance de macOS."),
    }
    VoiceSetupTitle "voice.setup.title" {
        en: t("Dictate with your voice"),
        fr: t("Dicter avec votre voix"),
    }
    VoiceSetupBody "voice.setup.body" {
        en: t("Acta turns what you say into text on this computer. Your voice never leaves it and is never saved."),
        fr: t("Acta transforme ce que vous dites en texte sur cet ordinateur. Votre voix n\u{2019}en sort jamais et n\u{2019}est jamais enregistrée."),
    }
    VoiceSetupCpu "voice.setup.cpu" {
        en: t("{size} download · works on this computer\u{2019}s processor"),
        fr: t("Téléchargement de {size} · fonctionne avec le processeur de cet ordinateur"),
    }
    VoiceSetupGpu "voice.setup.gpu" {
        en: t("{size} download · uses this computer\u{2019}s graphics"),
        fr: t("Téléchargement de {size} · utilise la carte graphique de cet ordinateur"),
    }
    VoiceSetupFree "voice.setup.free" {
        en: t("Free on this disk: {free}."),
        fr: t("Libre sur ce disque\u{a0}: {free}."),
    }
    VoiceSetupNoSpace "voice.setup.no_space" {
        en: t("Not enough free space: {size} needed, {free} free."),
        fr: t("Pas assez d\u{2019}espace libre\u{a0}: {size} nécessaires, {free} libres."),
    }
    VoiceSetupSystem "voice.setup.system" {
        en: t("Use the macOS recogniser instead: nothing to download"),
        fr: t("Utiliser plutôt la reconnaissance de macOS\u{a0}: rien à télécharger"),
    }
    VoiceSetupSystemPrimary "voice.setup.system_primary" {
        en: t("Use the macOS recogniser"),
        fr: t("Utiliser la reconnaissance de macOS"),
    }
    VoiceSetupUnavailable "voice.setup.unavailable" {
        en: t("Dictation needs at least 2 GB of memory; this computer has {memory}."),
        fr: t("La dictée demande au moins 2 Go de mémoire\u{202f}; cet ordinateur en a {memory}."),
    }
    VoiceSetupUnsupportedCpu "voice.setup.unsupported_cpu" {
        en: t("Dictation needs a processor from 2013 or later; this one is older."),
        fr: t("La dictée demande un processeur de 2013 ou plus récent\u{202f}; celui-ci est plus ancien."),
    }
    VoiceSetupMore "voice.setup.more" {
        en: t("Other models are in Settings > Voice."),
        fr: t("Les autres modèles sont dans Paramètres > Voix."),
    }
    VoiceSetupDownload "voice.setup.download" {
        en: t("Download {size}"),
        fr: t("Télécharger {size}"),
    }
    VoiceDownloadProgressTitle "voice.download.progress_title" {
        en: t("Downloading Whisper {model}\u{2026}"),
        fr: t("Téléchargement de Whisper {model}\u{2026}"),
    }
    VoiceDownloadBytes "voice.download.bytes" {
        en: t("{done} of {size}"),
        fr: t("{done} sur {size}"),
    }
    VoiceDownloadChecking "voice.download.checking" {
        en: t("Checking\u{2026}"),
        fr: t("Vérification\u{2026}"),
    }
    VoiceDownloadReady "voice.download.ready" {
        en: t("Ready. Press {shortcut} in a text, or the microphone, to dictate."),
        fr: t("Prêt. Appuyez sur {shortcut} dans un texte, ou sur le micro, pour dicter."),
    }
    VoiceDownloadHide "voice.download.hide" {
        en: t("Hide"),
        fr: t("Masquer"),
    }
    VoiceDownloadCancel "voice.download.cancel" {
        en: t("Cancel download"),
        fr: t("Annuler le téléchargement"),
    }
    VoiceDownloadFailed "voice.download.failed" {
        en: t("The download stopped. {reason}"),
        fr: t("Le téléchargement s\u{2019}est arrêté. {reason}"),
    }
    VoiceDownloadMismatch "voice.download.mismatch" {
        en: t("The file did not match. Try again later."),
        fr: t("Le fichier ne correspond pas. Réessayez plus tard."),
    }
    VoiceDownloadOffline "voice.download.offline" {
        en: t("There is no connection."),
        fr: t("Il n\u{2019}y a pas de connexion."),
    }
    VoiceSettingsTitle "voice.settings.title" {
        en: t("Voice"),
        fr: t("Voix"),
    }
    VoiceSettingsEngine "voice.settings.engine" {
        en: t("Dictation"),
        fr: t("Dictée"),
    }
    VoiceSettingsEngineHint "voice.settings.engine_hint" {
        en: t("Dictation in notes, descriptions and documents. Your voice never leaves this computer."),
        fr: t("Dictée dans les notes, les descriptions et les documents. Votre voix ne quitte jamais cet ordinateur."),
    }
    VoiceEngineWhisper "voice.engine.whisper" {
        en: t("Whisper, on this computer"),
        fr: t("Whisper, sur cet ordinateur"),
    }
    VoiceEngineMacos "voice.engine.macos" {
        en: t("macOS recogniser, on this computer"),
        fr: t("Reconnaissance de macOS, sur cet ordinateur"),
    }
    VoiceEngineOff "voice.engine.off" {
        en: t("Off"),
        fr: t("Désactivée"),
    }
    VoiceSettingsModel "voice.settings.model" {
        en: t("Model"),
        fr: t("Modèle"),
    }
    VoiceSettingsModelHint "voice.settings.model_hint" {
        en: t("Recommended for this computer: {model}. Larger models are more accurate and slower."),
        fr: t("Recommandé pour cet ordinateur\u{a0}: {model}. Les modèles plus grands sont plus précis et plus lents."),
    }
    VoiceSettingsModelHintGpu "voice.settings.model_hint_gpu" {
        en: t("This computer\u{2019}s graphics can run the large model quickly."),
        fr: t("La carte graphique de cet ordinateur fait tourner le grand modèle rapidement."),
    }
    VoiceSettingsGpu "voice.settings.gpu" {
        en: t("Use the graphics card"),
        fr: t("Utiliser la carte graphique"),
    }
    VoiceSettingsGpuHint "voice.settings.gpu_hint" {
        en: t("Faster with large models. Acta uses the processor when the graphics card cannot be used."),
        fr: t("Plus rapide avec les grands modèles. Acta utilise le processeur quand la carte graphique n\u{2019}est pas utilisable."),
    }
    VoiceSettingsLanguage "voice.settings.language" {
        en: t("Language"),
        fr: t("Langue"),
    }
    VoiceSettingsLanguageHint "voice.settings.language_hint" {
        en: t("Pick one if dictation mistakes one language for the other."),
        fr: t("Choisissez-en une si la dictée confond les deux langues."),
    }
    VoiceLanguageAuto "voice.language.auto" {
        en: t("English or French, detected"),
        fr: t("Anglais ou français, détecté"),
    }
    VoiceLanguageEn "voice.language.en" {
        en: t("English"),
        fr: t("Anglais"),
    }
    VoiceLanguageFr "voice.language.fr" {
        en: t("French"),
        fr: t("Français"),
    }
    VoiceSettingsMicrophone "voice.settings.microphone" {
        en: t("Microphone"),
        fr: t("Micro"),
    }
    VoiceSettingsMicrophoneDefault "voice.settings.microphone_default" {
        en: t("System default"),
        fr: t("Celui du système"),
    }
    VoiceSettingsShortcut "voice.settings.shortcut" {
        en: t("Shortcut"),
        fr: t("Raccourci"),
    }
    VoiceSettingsShortcutHint "voice.settings.shortcut_hint" {
        en: t("Press once to start and again to stop, or hold it while you speak."),
        fr: t("Appuyez une fois pour commencer et une autre pour arrêter, ou maintenez-le pendant que vous parlez."),
    }
    VoiceModelTinyHint "voice.model.tiny_hint" {
        en: t("{size} · for older computers; rough in French"),
        fr: t("{size} · pour les ordinateurs anciens\u{202f}; approximatif en français"),
    }
    VoiceModelBaseHint "voice.model.base_hint" {
        en: t("{size} · fast; fair accuracy"),
        fr: t("{size} · rapide\u{202f}; précision correcte"),
    }
    VoiceModelSmallHint "voice.model.small_hint" {
        en: t("{size} · good for English and French"),
        fr: t("{size} · bon en anglais et en français"),
    }
    VoiceModelTurboHint "voice.model.turbo_hint" {
        en: t("{size} · most accurate; best with a graphics card"),
        fr: t("{size} · le plus précis\u{202f}; idéal avec une carte graphique"),
    }
    VoiceModelRecommended "voice.model.recommended" {
        en: t("Recommended"),
        fr: t("Recommandé"),
    }
    VoiceModelDownload "voice.model.download" {
        en: t("Download"),
        fr: t("Télécharger"),
    }
    VoiceModelInUse "voice.model.in_use" {
        en: t("In use"),
        fr: t("Utilisé"),
    }
    VoiceModelUse "voice.model.use" {
        en: t("Use"),
        fr: t("Utiliser"),
    }
    VoiceModelDelete "voice.model.delete" {
        en: t("Delete model"),
        fr: t("Supprimer le modèle"),
    }
    VoiceModelDeleteTitle "voice.model.delete_title" {
        en: t("Delete Whisper {model}?"),
        fr: t("Supprimer Whisper {model}\u{202f}?"),
    }
    VoiceModelDeleteBody "voice.model.delete_body" {
        en: t("It will be downloaded again ({size}) if you choose it later."),
        fr: t("Il sera téléchargé à nouveau ({size}) si vous le choisissez plus tard."),
    }
    VoiceModelBusy "voice.model.busy" {
        en: t("Stop dictating before deleting the model in use."),
        fr: t("Arrêtez la dictée avant de supprimer le modèle utilisé."),
    }
    VoiceModelsWhere "voice.models.where" {
        en: t("Models are kept in {path}. Free on this disk: {free}."),
        fr: t("Les modèles sont conservés dans {path}. Libre sur ce disque\u{a0}: {free}."),
    }
    VoiceModelsUsed "voice.models.used" {
        en: t("Downloaded models: {size}."),
        fr: t("Modèles téléchargés\u{a0}: {size}."),
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
    KbMoreInheritedProject "kb.more_inherited.project" {
        en: t("{n} more from project {name}"),
        fr: p("{n} autre hérité du projet {name}", "{n} autres hérités du projet {name}"),
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
        en: t("No documents or memories yet."),
        fr: t("Aucun document ni mémoire pour l\u{2019}instant."),
    }
    KbSummaryMemory "kb.summary.memory" {
        en: t("One line: the fact, in short"),
        fr: t("Une ligne\u{a0}: le fait, en bref"),
    }
    KbSummaryDocument "kb.summary.document" {
        en: t("One line: what this is and when to read it"),
        fr: t("Une ligne\u{a0}: ce que c\u{2019}est et quand le lire"),
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
    KbSavedFile "kb.saved_file" {
        en: t("Saved {name}. The file is not encrypted."),
        fr: t("{name} enregistré. Le fichier n\u{2019}est pas chiffré."),
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
        fr: t("Niveau\u{a0}: {value}"),
    }
    KbFilterKind "kb.filter.kind" {
        en: t("Kind: {value}"),
        fr: t("Nature\u{a0}: {value}"),
    }
    KbFilterAuthor "kb.filter.author" {
        en: t("Author: {value}"),
        fr: t("Auteur\u{a0}: {value}"),
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
    QuestionsNew "questions.new" {
        en: t("New question"),
        fr: t("Nouvelle question"),
    }
    QuestionsClosed "questions.closed" {
        en: t("{n} closed"),
        fr: p("{n} close", "{n} closes"),
    }
    QuestionsMoreEpic "questions.more_epic" {
        en: t("{n} more open in the topics of this epic"),
        fr: p("{n} autre en suspens dans les sujets de cet epic", "{n} autres en suspens dans les sujets de cet epic"),
    }
    QuestionsMoreEpicOverdue "questions.more_epic_overdue" {
        en: t("{n} more open in the topics of this epic, {late} overdue"),
        fr: p("{n} autre en suspens dans les sujets de cet epic, en retard", "{n} autres en suspens dans les sujets de cet epic, dont {late} en retard"),
    }
    QuestionsMoreProject "questions.more_project" {
        en: t("{n} more open in this project’s epics and topics"),
        fr: p("{n} autre en suspens dans les epics et sujets de ce projet", "{n} autres en suspens dans les epics et sujets de ce projet"),
    }
    QuestionsMoreProjectOverdue "questions.more_project_overdue" {
        en: t("{n} more open in this project’s epics and topics, {late} overdue"),
        fr: p("{n} autre en suspens dans les epics et sujets de ce projet, en retard", "{n} autres en suspens dans les epics et sujets de ce projet, dont {late} en retard"),
    }
    QuestionsEmpty "questions.empty" {
        en: t("No questions yet. Note what you are waiting to hear."),
        fr: t("Aucune question pour l’instant. Notez ce que vous attendez d’apprendre."),
    }
    QuestionsScope "questions.scope" {
        en: t("{scope} · {counts}"),
        fr: t("{scope} · {counts}"),
    }
    CountQuestionsOpen "count.questions_open" {
        en: t("{n} open"),
        fr: t("{n} en suspens"),
    }
    CountQuestionsClosed "count.questions_closed" {
        en: t("{n} closed"),
        fr: p("{n} close", "{n} closes"),
    }
    QuestionsWholeProject "questions.whole_project" {
        en: t("Whole project"),
        fr: t("Tout le projet"),
    }
    QuestionsNoneOpen "questions.none_open" {
        en: t("No open questions in {name}."),
        fr: t("Aucune question en suspens dans {name}."),
    }
    QuestionsNoneOverdue "questions.none_overdue" {
        en: t("No overdue questions."),
        fr: t("Aucune question en retard."),
    }
    QuestionsNoneClosed "questions.none_closed" {
        en: t("No closed questions yet."),
        fr: t("Aucune question close pour l’instant."),
    }
    QuestionStateOpen "question.state.open" {
        en: t("Open question"),
        fr: t("Question en suspens"),
    }
    QuestionStateAnswered "question.state.answered" {
        en: t("Answered"),
        fr: t("Répondue"),
    }
    QuestionStateDropped "question.state.dropped" {
        en: t("Dropped"),
        fr: t("Abandonnée"),
    }
    QuestionAsked "question.asked" {
        en: t("Asked {name}"),
        fr: t("Posée à : {name}"),
    }
    QuestionBy "question.by" {
        en: t("Answer by {date}"),
        fr: t("Réponse attendue le {date}"),
    }
    QuestionAnsweredBy "question.answered_by" {
        en: t("Answer recorded {date} by {author}"),
        fr: t("Réponse notée le {date} par {author}"),
    }
    QuestionDroppedBy "question.dropped_by" {
        en: t("Dropped {date} by {author}"),
        fr: t("Abandonnée le {date} par {author}"),
    }
    QuestionLogged "question.logged" {
        en: t("Logged as a decision"),
        fr: t("Consignée comme décision"),
    }
    QuestionIn "question.in" {
        en: t("in {level}"),
        fr: t("dans : {level}"),
    }
    QuestionPlaceholder "question.placeholder" {
        en: t("What do you need to know?"),
        fr: t("Que faut-il savoir ?"),
    }
    QuestionFieldAsked "question.field.asked" {
        en: t("Asked"),
        fr: t("Posée à"),
    }
    QuestionFieldBy "question.field.by" {
        en: t("Answer by"),
        fr: t("Réponse attendue"),
    }
    QuestionMarkAnswered "question.mark_answered" {
        en: t("Mark answered"),
        fr: t("Marquer comme répondue"),
    }
    QuestionAnswerPlaceholder "question.answer_placeholder" {
        en: t("What was the answer?"),
        fr: t("Quelle a été la réponse ?"),
    }
    QuestionLogDecision "question.log_decision" {
        en: t("Also log it as a decision"),
        fr: t("La consigner aussi comme décision"),
    }
    QuestionSaveAnswer "question.save_answer" {
        en: t("Save answer"),
        fr: t("Enregistrer la réponse"),
    }
    QuestionEditAnswer "question.edit_answer" {
        en: t("Edit answer"),
        fr: t("Modifier la réponse"),
    }
    QuestionSetDate "question.set_date" {
        en: t("Set answer date…"),
        fr: t("Définir la date de réponse…"),
    }
    QuestionClearDate "question.clear_date" {
        en: t("Clear answer date"),
        fr: t("Effacer la date de réponse"),
    }
    QuestionDrop "question.drop" {
        en: t("Drop question"),
        fr: t("Abandonner la question"),
    }
    QuestionReopen "question.reopen" {
        en: t("Reopen"),
        fr: t("Rouvrir"),
    }
    QuestionMove "question.move" {
        en: t("Move to…"),
        fr: t("Déplacer vers…"),
    }
    QuestionDeleted "question.deleted" {
        en: t("Question deleted."),
        fr: t("Question supprimée."),
    }
    QuestionRestored "question.restored" {
        en: t("Question restored."),
        fr: t("Question restaurée."),
    }
    QuestionCounter "question.counter" {
        en: t("{n} / 300"),
        fr: t("{n} / 300"),
    }
    PersonNone "person.none" {
        en: t("None"),
        fr: t("Personne"),
    }
    CountPersonAsked "count.person_asked" {
        en: p("{n} question to ask", "{n} questions to ask"),
        fr: p("{n} question à poser", "{n} questions à poser"),
    }
    CountPersonReport "count.person_report" {
        en: p("{n} thing to report", "{n} things to report"),
        fr: p("{n} élément à rapporter", "{n} éléments à rapporter"),
    }
    PersonSectionAsked "person.section.asked" {
        en: t("Questions asked"),
        fr: t("Questions posées"),
    }
    PersonSectionReport "person.section.report" {
        en: t("To report"),
        fr: t("À rapporter"),
    }
    PersonEmpty "person.empty" {
        en: t("Nothing open with {name}."),
        fr: t("Rien en cours avec : {name}."),
    }
    ReportTo "report.to" {
        en: t("Report to {name}"),
        fr: t("À rapporter à : {name}"),
    }
    ReportPending "report.pending" {
        en: t("Not reported yet"),
        fr: t("Pas encore rapporté"),
    }
    ReportDone "report.done" {
        en: t("Reported {date}"),
        fr: t("Rapporté le {date}"),
    }
    ReportMark "report.mark" {
        en: t("Mark reported"),
        fr: t("Marquer comme rapporté"),
    }
    ReportUnmark "report.unmark" {
        en: t("Mark not reported"),
        fr: t("Marquer comme non rapporté"),
    }
    ReportChange "report.change" {
        en: t("Change person…"),
        fr: t("Changer de personne…"),
    }
    ReportReportedCount "report.reported_count" {
        en: t("{n} reported"),
        fr: p("{n} rapporté", "{n} rapportés"),
    }
    ReportFilter "report.filter" {
        en: t("Report to: {name}"),
        fr: t("À rapporter à : {name}"),
    }
    ReportFilterAll "report.filter.all" {
        en: t("Report to: anyone"),
        fr: t("À rapporter à : tout le monde"),
    }
    ReportFilterAnyone "report.filter.anyone" {
        en: t("Anyone"),
        fr: t("Tout le monde"),
    }
    ContactPlaceholder "contact.placeholder" {
        en: t("Search contacts"),
        fr: t("Rechercher un contact"),
    }
    ContactNewNamed "contact.new_named" {
        en: t("New contact \"{name}\""),
        fr: t("Nouveau contact : {name}"),
    }
    ContactNewIn "contact.new_in" {
        en: t("in {level}"),
        fr: t("dans : {level}"),
    }
    ContactNewTag "contact.new_tag" {
        en: t("new"),
        fr: t("nouveau"),
    }
    ContactOutOfScope "contact.out_of_scope" {
        en: t("{name} is no longer visible here. Choose again."),
        fr: t("Ce contact n’est plus visible ici : {name}. Choisissez à nouveau."),
    }
    ContactLevelAll "contact.level.all" {
        en: t("All projects"),
        fr: t("Tous les projets"),
    }
    ContactLevelCurrent "contact.level.current" {
        en: t("Current"),
        fr: t("Actuel"),
    }
    ContactMoveBlocked "contact.move.blocked" {
        en: t("Used outside"),
        fr: t("Utilisé ailleurs"),
    }
    ContactMoveRefused "contact.move.refused" {
        en: t("Named outside {level}: {places}. Change those first."),
        fr: t("Cité hors de ce niveau ({level}) : {places}. Modifiez-les d’abord."),
    }
    ContactMovePlace "contact.move.place" {
        en: t("{id} {title} ({field})"),
        fr: t("{id} {title} ({field})"),
    }
    ContactMoveMore "contact.move.more" {
        en: t("+{n} more"),
        fr: p("+{n} autre", "+{n} autres"),
    }
    ContactLifted "contact.lifted" {
        en: t("{name} is now a contact of all projects, so {record} still sees them."),
        fr: t("Contact étendu à tous les projets pour rester visible depuis {record} : {name}."),
    }
    ContactLiftedProject "contact.lifted.project" {
        en: t("{name} is now a contact of project {project}, so {record} still sees them."),
        fr: t("Contact étendu au projet {project} pour rester visible depuis {record} : {name}."),
    }
    ContactsTitle "contacts.title" {
        en: t("Contacts"),
        fr: t("Contacts"),
    }
    ContactsCount "contacts.count" {
        en: p("{n} contact", "{n} contacts"),
        fr: p("{n} contact", "{n} contacts"),
    }
    ContactsFilterProject "contacts.filter.project" {
        en: t("This project"),
        fr: t("Ce projet"),
    }
    ContactsFilterAll "contacts.filter.all" {
        en: t("All"),
        fr: t("Tous"),
    }
    ContactsNew "contacts.new" {
        en: t("New contact"),
        fr: t("Nouveau contact"),
    }
    ContactsNewHere "contacts.new_here" {
        en: t("New contact here"),
        fr: t("Nouveau contact ici"),
    }
    ContactsEmpty "contacts.empty" {
        en: t("No contacts yet. A contact is added when you choose who a question is asked of or who to report to."),
        fr: t("Aucun contact pour l’instant. Un contact s’ajoute quand vous choisissez à qui poser une question ou à qui rapporter."),
    }
    ContactsNoMatch "contacts.no_match" {
        en: t("No contact matches \"{query}\"."),
        fr: t("Aucun contact ne correspond à « {query} »."),
    }
    ContactsNoneHere "contacts.none_here" {
        en: t("No contacts in {project}."),
        fr: t("Aucun contact dans {project}."),
    }
    ContactNamePlaceholder "contact.name_placeholder" {
        en: t("Name"),
        fr: t("Nom"),
    }
    ContactNameRequired "contact.name_required" {
        en: t("A contact needs a name."),
        fr: t("Un contact doit avoir un nom."),
    }
    ContactOpenCount "contact.open_count" {
        en: t("{n} open"),
        fr: t("{n} en cours"),
    }
    ContactPropJobTitle "contact.prop.job_title" {
        en: t("Job title"),
        fr: t("Fonction"),
    }
    ContactPropCompany "contact.prop.company" {
        en: t("Company"),
        fr: t("Entreprise"),
    }
    ContactPropLevel "contact.prop.level" {
        en: t("Level"),
        fr: t("Niveau"),
    }
    ContactAddJobTitle "contact.add.job_title" {
        en: t("Job title"),
        fr: t("Fonction"),
    }
    ContactAddCompany "contact.add.company" {
        en: t("Company"),
        fr: t("Entreprise"),
    }
    ContactAddAddress "contact.add.address" {
        en: t("How to address them"),
        fr: t("Comment s’adresser à ce contact"),
    }
    ContactSectionAddress "contact.section.address" {
        en: t("How to address them"),
        fr: t("Comment s’adresser à ce contact"),
    }
    ContactAddressPlaceholder "contact.address_placeholder" {
        en: t("Formal or first name, tone, what to avoid"),
        fr: t("Vouvoiement ou tutoiement, prénom ou nom, ton, ce qu’il faut éviter"),
    }
    ContactAddressCounter "contact.address_counter" {
        en: t("{n} / 500"),
        fr: t("{n} / 500"),
    }
    ContactMeta "contact.meta" {
        en: t("{id} · Added {date} by {author}"),
        fr: t("{id} · Ajouté le {date} par {author}"),
    }
    ContactMetaUpdated "contact.meta_updated" {
        en: t("{id} · Added {date} by {author} · Updated {when}"),
        fr: t("{id} · Ajouté le {date} par {author} · Modifié {when}"),
    }
    ContactScope "contact.scope" {
        en: t("{asked}, {report}"),
        fr: t("{asked}, {report}"),
    }
    ContactScopeProjects "contact.scope_projects" {
        en: t("{asked}, {report} in {n} projects"),
        fr: t("{asked}, {report} dans {n} projets"),
    }
    ContactDuplicate "contact.duplicate" {
        en: t("Another contact is also named {name}, in {level}."),
        fr: t("Un autre contact porte aussi ce nom, dans {level} : {name}."),
    }
    ContactMergeIntoIt "contact.merge_into_it" {
        en: t("Merge into it…"),
        fr: t("Fusionner avec lui…"),
    }
    ContactKeepBoth "contact.keep_both" {
        en: t("Keep both"),
        fr: t("Garder les deux"),
    }
    ContactGo "contact.go" {
        en: t("Go to {name}"),
        fr: t("Ouvrir : {name}"),
    }
    ContactRename "contact.rename" {
        en: t("Rename"),
        fr: t("Renommer"),
    }
    ContactMove "contact.move" {
        en: t("Move to…"),
        fr: t("Déplacer vers…"),
    }
    ContactMerge "contact.merge" {
        en: t("Merge into…"),
        fr: t("Fusionner avec…"),
    }
    ContactDelete "contact.delete" {
        en: t("Delete contact"),
        fr: t("Supprimer le contact"),
    }
    ContactMerged "contact.merged" {
        en: t("Merged into {name}."),
        fr: t("Fusionné avec : {name}."),
    }
    ContactRestoredMerged "contact.restored_merged" {
        en: t("Contact restored. Its uses stayed with {name}."),
        fr: t("Contact restauré. Ses usages restent à : {name}."),
    }
    ContactDeleted "contact.deleted" {
        en: t("Contact deleted."),
        fr: t("Contact supprimé."),
    }
    ContactRestored "contact.restored" {
        en: t("Contact restored."),
        fr: t("Contact restauré."),
    }
    ContactInTrash "contact.in_trash" {
        en: t("In the trash"),
        fr: t("Dans la corbeille"),
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
    DocSectionLines "doc.section_lines" {
        en: p("{n} line", "{n} lines"),
        fr: p("{n} ligne", "{n} lignes"),
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
    HistoryCreatedDocument "history.created.document" {
        en: t("Created"),
        fr: t("Créé"),
    }
    HistoryCreatedMemory "history.created.memory" {
        en: t("Created"),
        fr: t("Créée"),
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
    FoldersAdd "folders.add" {
        en: t("Add folder\u{2026}"),
        fr: t("Ajouter un dossier\u{2026}"),
    }
    FoldersHint "folders.hint" {
        en: t("Claude Code starts in the active folder and can also use the others."),
        fr: t("Claude Code démarre dans le dossier actif et peut aussi utiliser les autres."),
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
        en: t("Also attached to topic {name}."),
        fr: t("Également rattaché au sujet {name}."),
    }
    FoldersAlsoEpic "folders.also_epic" {
        en: t("Also attached to epic {name}."),
        fr: t("Également rattaché à l\u{2019}epic {name}."),
    }
    FoldersAlsoMore "folders.also_more" {
        en: t("Also attached to topics {a}, {b} and {n} more."),
        fr: p("Également rattaché aux sujets {a}, {b} et {n} autre.", "Également rattaché aux sujets {a}, {b} et {n} autres."),
    }
    FoldersAlsoTip "folders.also.tip" {
        en: t("A session started in this folder outside Acta opens on {level} and is told about both."),
        fr: t("Une session lancée dans ce dossier hors d\u{2019}Acta s\u{2019}ouvre sur\u{a0}: {level}, et est informée des deux."),
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
        en: t("{folder} located for epic {name} and everything under it."),
        fr: t("{folder} localisé pour l\u{2019}epic {name} et tout ce qu\u{2019}il contient."),
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
    FoldersActive "folders.active" {
        en: t("Active"),
        fr: t("Actif"),
    }
    FoldersSetActive "folders.set_active" {
        en: t("Set as active here"),
        fr: t("Définir comme actif ici"),
    }
    FoldersEdit "folders.edit" {
        en: t("Edit folders\u{2026}"),
        fr: t("Modifier les dossiers\u{2026}"),
    }
    FoldersSettings "folders.settings" {
        en: t("Folder settings\u{2026}"),
        fr: t("Réglages du dossier\u{2026}"),
    }
    FoldersGroupTopic "folders.group.topic" {
        en: t("This topic"),
        fr: t("Ce sujet"),
    }
    FoldersGroupEpic "folders.group.epic" {
        en: t("This epic"),
        fr: t("Cet epic"),
    }
    FoldersGroupProject "folders.group.project" {
        en: t("This project"),
        fr: t("Ce projet"),
    }
    FoldersGroupFromEpic "folders.group.from_epic" {
        en: t("From epic {name}"),
        fr: t("De l\u{2019}epic {name}"),
    }
    FoldersGroupFromProject "folders.group.from_project" {
        en: t("From project {name}"),
        fr: t("Du projet {name}"),
    }
    FoldersNoOwn "folders.no_own" {
        en: t("No folders of its own."),
        fr: t("Aucun dossier propre."),
    }
    FoldersNotFoundShort "folders.not_found_short" {
        en: t("Not found"),
        fr: t("Introuvable"),
    }
    FoldersOtherCheckout "folders.other_checkout" {
        en: t("Another checkout of {name}\u{2019}s repository. Not passed to Claude Code here."),
        fr: t("Autre copie du dépôt de {name}. Non transmise à Claude Code ici."),
    }
    FoldersGitDetached "folders.git.detached" {
        en: t("detached at {commit}"),
        fr: t("détaché sur {commit}"),
    }
    FoldersGitChanged "folders.git.changed" {
        en: t("{n} changed"),
        fr: p("{n} modifié", "{n} modifiés"),
    }
    FoldersGitAutoCommit "folders.git.auto_commit" {
        en: t("Claude Code commits automatically here"),
        fr: t("Claude Code commite automatiquement ici"),
    }
    FoldersGitAutoPush "folders.git.auto_push" {
        en: t("Claude Code commits and pushes automatically here"),
        fr: t("Claude Code commite et pousse automatiquement ici"),
    }
    FoldersName "folders.name" {
        en: t("Name"),
        fr: t("Nom"),
    }
    FoldersIcon "folders.icon" {
        en: t("Icon"),
        fr: t("Icône"),
    }
    FoldersIconAuto "folders.icon.auto" {
        en: t("Automatic"),
        fr: t("Automatique"),
    }
    FoldersIconAutoHint "folders.icon.auto_hint" {
        en: t("Chosen from the folder: worktree, repository or plain folder"),
        fr: t("Choisie d\u{2019}après le dossier\u{a0}: worktree, dépôt ou dossier simple"),
    }
    FoldersIconFolder "folders.icon.folder" {
        en: t("Folder"),
        fr: t("Dossier"),
    }
    FoldersIconRepo "folders.icon.repo" {
        en: t("Git repository"),
        fr: t("Dépôt git"),
    }
    FoldersIconWorktree "folders.icon.worktree" {
        en: t("Git worktree"),
        fr: t("Worktree git"),
    }
    FoldersIconCode "folders.icon.code" {
        en: t("Code"),
        fr: t("Code"),
    }
    FoldersIconDocs "folders.icon.docs" {
        en: t("Documentation"),
        fr: t("Documentation"),
    }
    FoldersIconPackage "folders.icon.package" {
        en: t("Package"),
        fr: t("Paquet"),
    }
    FoldersIconServer "folders.icon.server" {
        en: t("Server"),
        fr: t("Serveur"),
    }
    FoldersIconWeb "folders.icon.web" {
        en: t("Web"),
        fr: t("Web"),
    }
    FoldersIconMobile "folders.icon.mobile" {
        en: t("Mobile"),
        fr: t("Mobile"),
    }
    FoldersIconData "folders.icon.data" {
        en: t("Data"),
        fr: t("Données"),
    }
    FoldersIconLab "folders.icon.lab" {
        en: t("Experiments"),
        fr: t("Expériences"),
    }
    FoldersIconDesign "folders.icon.design" {
        en: t("Design"),
        fr: t("Design"),
    }
    FoldersCommits "folders.commits" {
        en: t("Commits by Claude Code"),
        fr: t("Commits par Claude Code"),
    }
    FoldersCommitManual "folders.commit.manual" {
        en: t("Manual"),
        fr: t("Manuel"),
    }
    FoldersCommitCommit "folders.commit.commit" {
        en: t("Commit"),
        fr: t("Commit"),
    }
    FoldersCommitPush "folders.commit.push" {
        en: t("Commit and push"),
        fr: t("Commit et push"),
    }
    FoldersCommitManualHint "folders.commit.manual_hint" {
        en: t("Claude Code commits only when asked."),
        fr: t("Claude Code ne commite que sur demande."),
    }
    FoldersCommitCommitHint "folders.commit.commit_hint" {
        en: t("Claude Code commits each finished, tested change."),
        fr: t("Claude Code commite chaque modification terminée et testée."),
    }
    FoldersCommitPushHint "folders.commit.push_hint" {
        en: t("Then pushes to the upstream branch, never forced. It stops if a push fails."),
        fr: t("Puis pousse vers la branche amont, jamais en force. Il s\u{2019}arrête si un push échoue."),
    }
    FoldersCommitAsUser "folders.commit.as_user" {
        en: t("Commit in my name, without Co-Authored-By Claude"),
        fr: t("Commiter en mon nom, sans Co-Authored-By Claude"),
    }
    FoldersCommitOnDefault "folders.commit.on_default" {
        en: t("Also on {branch}"),
        fr: t("Aussi sur {branch}"),
    }
    FoldersCommitIdentity "folders.commit.identity" {
        en: t("Commits as {name}."),
        fr: t("Commits au nom de\u{a0}: {name}."),
    }
    FoldersCommitNoIdentity "folders.commit.no_identity" {
        en: t("Git has no user name here, so commits will fail."),
        fr: t("Git n\u{2019}a pas de nom d\u{2019}utilisateur ici\u{a0}: les commits échoueront."),
    }
    FoldersCommitFile "folders.commit.file" {
        en: t("Saved in .claude/settings.local.json, kept out of git."),
        fr: t("Enregistré dans .claude/settings.local.json, exclu de git."),
    }
    FoldersCommitTracked "folders.commit.tracked" {
        en: t(".claude/settings.local.json is tracked by git here, so Acta leaves it alone. Claude Code will ask before each commit."),
        fr: t(".claude/settings.local.json est suivi par git ici, Acta n\u{2019}y touche donc pas. Claude Code demandera avant chaque commit."),
    }
    FoldersCommitUserOwn "folders.commit.user_key" {
        en: t("Your own {key} setting is kept."),
        fr: t("Votre propre réglage {key} est conservé."),
    }
    FoldersCommitRootWins "folders.commit.root_wins" {
        en: t("The repository\u{2019}s own attribution setting wins."),
        fr: t("Le réglage attribution du dépôt l\u{2019}emporte."),
    }
    FoldersGitBehind "folders.git.behind" {
        en: t("behind {n}"),
        fr: t("{n} en retard"),
    }
    FoldersGitPull "folders.git.pull" {
        en: t("Pull"),
        fr: t("Pull"),
    }
    FoldersGitFetch "folders.git.fetch" {
        en: t("Fetch"),
        fr: t("Fetch"),
    }
    FoldersGitBranchTip "folders.git.branch_tip" {
        en: t("Switch branch"),
        fr: t("Changer de branche"),
    }
    FoldersGitLocal "folders.git.local" {
        en: t("Branches"),
        fr: t("Branches"),
    }
    FoldersGitRemote "folders.git.remote" {
        en: t("Remote branches"),
        fr: t("Branches distantes"),
    }
    FoldersGitTrack "folders.git.track" {
        en: t("track"),
        fr: t("suivre"),
    }
    FoldersGitNewBranch "folders.git.new_branch" {
        en: t("New branch\u{2026}"),
        fr: t("Nouvelle branche\u{2026}"),
    }
    FoldersGitNewBranchHint "folders.git.new_branch_hint" {
        en: t("Name of the new branch, from the current commit"),
        fr: t("Nom de la nouvelle branche, depuis le commit actuel"),
    }
    FoldersGitCreate "folders.git.create" {
        en: t("Create"),
        fr: t("Créer"),
    }
    FoldersGitPulled "folders.git.pulled" {
        en: p("Pulled {n} commit into {branch}.", "Pulled {n} commits into {branch}."),
        fr: p("{n} commit récupéré dans {branch}.", "{n} commits récupérés dans {branch}."),
    }
    FoldersGitUpToDate "folders.git.up_to_date" {
        en: t("{branch} is up to date."),
        fr: t("{branch} est à jour."),
    }
    FoldersGitErrNoGit "folders.git.err.no_git" {
        en: t("Git is not installed on this computer."),
        fr: t("Git n\u{2019}est pas installé sur cet ordinateur."),
    }
    FoldersGitErrTimedOut "folders.git.err.timed_out" {
        en: t("Git took too long, so Acta stopped it. Nothing was changed."),
        fr: t("Git a mis trop de temps\u{a0}: Acta l\u{2019}a arrêté. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrNoUpstream "folders.git.err.no_upstream" {
        en: t("{branch} has no upstream branch to pull from. Nothing was changed."),
        fr: t("{branch} n\u{2019}a pas de branche amont d\u{2019}où récupérer. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrDetached "folders.git.err.detached" {
        en: t("There is no current branch (detached HEAD). Nothing was changed."),
        fr: t("Il n\u{2019}y a pas de branche courante (HEAD détachée). Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrDiverged "folders.git.err.diverged" {
        en: t("{branch} and its upstream have diverged, so it cannot be fast-forwarded. Nothing was changed."),
        fr: t("{branch} et sa branche amont ont divergé\u{a0}: l\u{2019}avance rapide est impossible. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrLocalChanges "folders.git.err.local_changes" {
        en: t("Local changes in {folder} would be overwritten. Nothing was changed: commit them or set them aside first."),
        fr: t("Des modifications locales de {folder} seraient écrasées. Rien n\u{2019}a été modifié\u{a0}: commitez-les ou mettez-les de côté d\u{2019}abord."),
    }
    FoldersGitErrSignIn "folders.git.err.sign_in" {
        en: t("The remote asks for a sign-in, and Acta never asks for one. Nothing was changed."),
        fr: t("Le dépôt distant demande une connexion, et Acta n\u{2019}en demande jamais. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrNetwork "folders.git.err.network" {
        en: t("The remote could not be reached. Nothing was changed."),
        fr: t("Le dépôt distant est injoignable. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrExists "folders.git.err.exists" {
        en: t("A branch named {branch} already exists. Nothing was changed."),
        fr: t("Une branche nommée {branch} existe déjà. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrInvalidName "folders.git.err.invalid_name" {
        en: t("{branch} is not a valid branch name. Nothing was changed."),
        fr: t("{branch} n\u{2019}est pas un nom de branche valide. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrNoBranch "folders.git.err.no_branch" {
        en: t("There is no branch {branch}. Nothing was changed."),
        fr: t("Il n\u{2019}y a pas de branche {branch}. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrElsewhere "folders.git.err.elsewhere" {
        en: t("{branch} is checked out in another worktree. Nothing was changed."),
        fr: t("{branch} est extraite dans un autre worktree. Rien n\u{2019}a été modifié."),
    }
    FoldersGitErrOther "folders.git.err.other" {
        en: t("Git refused, and nothing was changed: {message}"),
        fr: t("Git a refusé, rien n\u{2019}a été modifié\u{a0}: {message}"),
    }
    FoldersGitBusyTitle "folders.git.busy.title" {
        en: t("Claude Code is working in {folder}"),
        fr: t("Claude Code travaille dans {folder}"),
    }
    FoldersGitBusySwitch "folders.git.busy.switch" {
        en: t("Switching to {branch} changes the files under it while it works. It is not told."),
        fr: t("Passer sur {branch} change ses fichiers pendant qu\u{2019}il travaille. Il n\u{2019}en est pas informé."),
    }
    FoldersGitBusyPull "folders.git.busy.pull" {
        en: t("Pulling changes the files under it while it works. It is not told."),
        fr: t("Le pull change ses fichiers pendant qu\u{2019}il travaille. Il n\u{2019}en est pas informé."),
    }
    FoldersGitBusyConfirmSwitch "folders.git.busy.confirm_switch" {
        en: t("Switch anyway"),
        fr: t("Changer quand même"),
    }
    FoldersGitBusyConfirmPull "folders.git.busy.confirm_pull" {
        en: t("Pull anyway"),
        fr: t("Pull quand même"),
    }
    TerminalTitle "terminal.title" {
        en: t("Terminal"),
        fr: t("Terminal"),
    }
    TerminalOnTopic "terminal.on_topic" {
        en: t("{id} {name}"),
        fr: t("{id} {name}"),
    }
    TerminalOnEpic "terminal.on_epic" {
        en: t("{id} {name}"),
        fr: t("{id} {name}"),
    }
    TerminalOnProject "terminal.on_project" {
        en: t("project {name}"),
        fr: t("projet {name}"),
    }
    TerminalEnded "terminal.ended" {
        en: t("Session ended. {keys} to restart."),
        fr: t("Session terminée. {keys} pour relancer."),
    }
    TerminalEndedOnce "terminal.ended_once" {
        en: t("Session ended."),
        fr: t("Session terminée."),
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
    TerminalMissingBodyTopic "terminal.missing.body.topic" {
        en: t("Claude Code was not started: the active folder of topic {name} is not on this computer."),
        fr: t("Claude Code n\u{2019}a pas été lancé\u{a0}: le dossier actif du sujet {name} n\u{2019}est pas sur cet ordinateur."),
    }
    TerminalMissingBodyEpic "terminal.missing.body.epic" {
        en: t("Claude Code was not started: the active folder of epic {name} is not on this computer."),
        fr: t("Claude Code n\u{2019}a pas été lancé\u{a0}: le dossier actif de l\u{2019}epic {name} n\u{2019}est pas sur cet ordinateur."),
    }
    TerminalMissingBodyProject "terminal.missing.body.project" {
        en: t("Claude Code was not started: the active folder of project {name} is not on this computer."),
        fr: t("Claude Code n\u{2019}a pas été lancé\u{a0}: le dossier actif du projet {name} n\u{2019}est pas sur cet ordinateur."),
    }
    TerminalMissingOther "terminal.missing.other" {
        en: t("Or start in another folder:"),
        fr: t("Ou démarrer dans un autre dossier\u{a0}:"),
    }
    TerminalRunningIn "terminal.running_in" {
        en: t("(running in {name})"),
        fr: t("(lancé dans {name})"),
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
    QuitInstalling "quit.installing" {
        en: t("{name} is still installing. It goes on after Acta quits, but its log is lost."),
        fr: t("{name} est encore en cours d\u{2019}installation. Elle continue après la fermeture d\u{2019}Acta, mais son journal est perdu."),
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
        en: t("Chat with Claude"),
        fr: t("Discuter avec Claude"),
    }
    ChatStartBody "chat.start.body" {
        en: t("claude.ai in a panel, signed in with your own account. Acta never reads the page."),
        fr: t("claude.ai dans un panneau, avec votre propre compte. Acta ne lit jamais la page."),
    }
    ChatStartMemory "chat.start.memory" {
        en: t("Uses about 400 MB of memory once opened."),
        fr: t("Utilise environ 400\u{202f}Mo de mémoire une fois ouvert."),
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
        en: t("Sign in by email: open the link, then type the code it shows here. Google or Apple sign-in may be refused."),
        fr: t("Connectez-vous par e-mail\u{a0}: ouvrez le lien, puis saisissez ici le code affiché. La connexion Google ou Apple peut être refusée."),
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
        en: t("WebView2 is not installed, so claude.ai cannot show here. Everything else works."),
        fr: t("WebView2 n\u{2019}est pas installé\u{a0}: claude.ai ne peut pas s\u{2019}afficher ici. Tout le reste fonctionne."),
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
        en: t("Signs you out of claude.ai here. Your conversations stay in your Claude account."),
        fr: t("Vous déconnecte de claude.ai ici. Vos conversations restent dans votre compte Claude."),
    }
    ChatSignoutConfirm "chat.signout.confirm" {
        en: t("Sign out and clear"),
        fr: t("Se déconnecter et effacer"),
    }
    ChatMore "chat.more" {
        en: t("More"),
        fr: t("Plus"),
    }
    ChatClearFailed "chat.clear_failed" {
        en: t("The chat data could not be deleted. Acta tries again at the next start."),
        fr: t("Les données du chat n\u{2019}ont pas pu être supprimées. Acta réessaiera au prochain démarrage."),
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
    ViewProjects "view.projects" {
        en: t("Projects"),
        fr: t("Projets"),
    }
    AreaLeft "area.left" {
        en: t("Left sidebar"),
        fr: t("Barre latérale gauche"),
    }
    AreaRight "area.right" {
        en: t("Right sidebar"),
        fr: t("Barre latérale droite"),
    }
    AreaBottom "area.bottom" {
        en: t("Bottom panel"),
        fr: t("Panneau du bas"),
    }
    AreaPage "area.page" {
        en: t("Page area"),
        fr: t("Zone des pages"),
    }
    AreaHideLeft "area.hide.left" {
        en: t("Hide left sidebar"),
        fr: t("Masquer la barre latérale gauche"),
    }
    AreaHideRight "area.hide.right" {
        en: t("Hide right sidebar"),
        fr: t("Masquer la barre latérale droite"),
    }
    AreaHideBottom "area.hide.bottom" {
        en: t("Hide bottom panel"),
        fr: t("Masquer le panneau du bas"),
    }
    AreaEmpty "area.empty" {
        en: t("Drag a view here, or move one from the palette."),
        fr: t("Faites glisser une vue ici, ou déplacez-en une depuis la palette."),
    }
    ViewActions "view.actions" {
        en: t("View actions"),
        fr: t("Actions de la vue"),
    }
    ViewMoveLeft "view.move_left" {
        en: t("Move to left sidebar"),
        fr: t("Déplacer dans la barre latérale gauche"),
    }
    ViewMoveRight "view.move_right" {
        en: t("Move to right sidebar"),
        fr: t("Déplacer dans la barre latérale droite"),
    }
    ViewMoveBottom "view.move_bottom" {
        en: t("Move to bottom panel"),
        fr: t("Déplacer dans le panneau du bas"),
    }
    ViewMovePage "view.move_page" {
        en: t("Move to page area"),
        fr: t("Déplacer dans la zone des pages"),
    }
    ViewSplitOff "view.split_off" {
        en: t("Split from group"),
        fr: t("Séparer du groupe"),
    }
    ViewHide "view.hide" {
        en: t("Hide"),
        fr: t("Masquer"),
    }
    LayoutMenu "layout.menu" {
        en: t("Layout"),
        fr: t("Disposition"),
    }
    LayoutGroup "layout.group" {
        en: t("Group {n}"),
        fr: t("Groupe {n}"),
    }
    LayoutWebviewHidden "layout.webview_hidden" {
        en: t("Hidden while you rearrange the layout."),
        fr: t("Masqué pendant la réorganisation de la disposition."),
    }
    LayoutMovedLeft "layout.moved.left" {
        en: t("View moved to the left sidebar: {view}"),
        fr: t("Vue déplacée dans la barre latérale gauche\u{a0}: {view}"),
    }
    LayoutMovedRight "layout.moved.right" {
        en: t("View moved to the right sidebar: {view}"),
        fr: t("Vue déplacée dans la barre latérale droite\u{a0}: {view}"),
    }
    LayoutMovedBottom "layout.moved.bottom" {
        en: t("View moved to the bottom panel: {view}"),
        fr: t("Vue déplacée dans le panneau du bas\u{a0}: {view}"),
    }
    LayoutMovedPage "layout.moved.page" {
        en: t("View moved to the page area: {view}"),
        fr: t("Vue déplacée dans la zone des pages\u{a0}: {view}"),
    }
    LayoutHidden "layout.hidden" {
        en: t("View hidden: {view}"),
        fr: t("Vue masquée\u{a0}: {view}"),
    }
    LayoutResetDone "layout.reset_done" {
        en: t("Layout reset."),
        fr: t("Disposition réinitialisée."),
    }
    LayoutNoRoom "layout.no_room" {
        en: t("Not enough room: {reason}"),
        fr: t("Pas assez de place\u{a0}: {reason}"),
    }
    LayoutNoRoomLeft "layout.no_room.left" {
        en: t("the window is too narrow for both sidebars."),
        fr: t("la fenêtre est trop étroite pour les deux barres latérales."),
    }
    LayoutNoRoomHideRight "layout.no_room.hide_right" {
        en: t("Hide the right sidebar and move"),
        fr: t("Masquer la barre latérale droite et déplacer"),
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
    PaletteEpics "palette.epics" {
        en: t("Epics"),
        fr: t("Epics"),
    }
    PaletteHintOpen "palette.hint.open" {
        en: t("{keys} open"),
        fr: t("{keys} ouvrir"),
    }
    PaletteHintTab "palette.hint.tab" {
        en: t("{keys} open in new tab"),
        fr: t("{keys} ouvrir dans un nouvel onglet"),
    }
    PaletteHintCreate "palette.hint.create" {
        en: t("{keys} create"),
        fr: t("{keys} créer"),
    }
    PaletteHintChoose "palette.hint.choose" {
        en: t("{keys} choose"),
        fr: t("{keys} choisir"),
    }
    PaletteHintClose "palette.hint.close" {
        en: t("{keys} close"),
        fr: t("{keys} fermer"),
    }
    PaletteNone "palette.none" {
        en: t("Nothing matches \"{query}\". {keys} creates a topic with this title."),
        fr: t("Aucun résultat pour «\u{202f}{query}\u{202f}». {keys} crée un sujet avec ce titre."),
    }
    PaletteNoChoice "palette.no_choice" {
        en: t("Nothing matches \"{query}\"."),
        fr: t("Aucun résultat pour «\u{202f}{query}\u{202f}»."),
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
    CmdGoInbox "cmd.go_inbox" {
        en: t("Go to inbox"),
        fr: t("Aller à la boîte de réception"),
    }
    CmdShowBlocked "cmd.show_blocked" {
        en: t("Show blocked topics"),
        fr: t("Afficher les sujets bloqués"),
    }
    CmdShowIdle "cmd.show_idle" {
        en: t("Show topics with no activity"),
        fr: t("Afficher les sujets sans activité"),
    }
    CmdShowDoneWeek "cmd.show_done_week" {
        en: t("Show topics done in the last 7 days"),
        fr: t("Afficher les sujets terminés ces 7 derniers jours"),
    }
    CmdShowDecidedWeek "cmd.show_decided_week" {
        en: t("Show topics with a decision in the last 7 days"),
        fr: t("Afficher les sujets avec une décision ces 7 derniers jours"),
    }
    CmdShowCreatedWeek "cmd.show_created_week" {
        en: t("Show topics created in the last 7 days"),
        fr: t("Afficher les sujets créés ces 7 derniers jours"),
    }
    CmdShowStaleMemories "cmd.show_stale_memories" {
        en: t("Show memories not updated for 90 days"),
        fr: t("Afficher les mémoires non mises à jour depuis 90 jours"),
    }
    CmdShowQuestionsLate "cmd.show_questions_late" {
        en: t("Show questions past their answer date"),
        fr: t("Afficher les questions dont la date de réponse est passée"),
    }
    CmdOverviewMore "cmd.overview_more" {
        en: t("Overview: Show more"),
        fr: t("Vue d\u{2019}ensemble\u{a0}: afficher plus"),
    }
    CmdOverviewLess "cmd.overview_less" {
        en: t("Overview: Show less"),
        fr: t("Vue d\u{2019}ensemble\u{a0}: afficher moins"),
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
        en: t("Maximize focused area"),
        fr: t("Agrandir au maximum la zone active"),
    }
    CmdSetStatus "cmd.set_status" {
        en: t("Set status: {status}"),
        fr: t("Définir le statut\u{a0}: {status}"),
    }
    CmdDueSet "cmd.due_set" {
        en: t("Set due date\u{2026}"),
        fr: t("Définir l\u{2019}échéance\u{2026}"),
    }
    CmdDueClear "cmd.due_clear" {
        en: t("Clear due date"),
        fr: t("Effacer l\u{2019}échéance"),
    }
    CmdQuestionNew "cmd.question_new" {
        en: t("New question"),
        fr: t("Nouvelle question"),
    }
    CmdQuestionsOpen "cmd.questions_open" {
        en: t("Go to questions"),
        fr: t("Aller aux questions"),
    }
    CmdReportSet "cmd.report_set" {
        en: t("Set Report to…"),
        fr: t("Définir « À rapporter à »…"),
    }
    CmdReportClear "cmd.report_clear" {
        en: t("Clear Report to"),
        fr: t("Effacer « À rapporter à »"),
    }
    CmdReported "cmd.reported" {
        en: t("Mark reported"),
        fr: t("Marquer comme rapporté"),
    }
    CmdUnreported "cmd.unreported" {
        en: t("Mark not reported"),
        fr: t("Marquer comme non rapporté"),
    }
    CmdQuestionAnswer "cmd.question_answer" {
        en: t("Mark question answered"),
        fr: t("Marquer la question comme répondue"),
    }
    CmdQuestionEdit "cmd.question_edit" {
        en: t("Edit question"),
        fr: t("Modifier la question"),
    }
    CmdQuestionDate "cmd.question_date" {
        en: t("Set answer date…"),
        fr: t("Définir la date de réponse…"),
    }
    CmdQuestionDateClear "cmd.question_date_clear" {
        en: t("Clear answer date"),
        fr: t("Effacer la date de réponse"),
    }
    CmdQuestionDrop "cmd.question_drop" {
        en: t("Drop question"),
        fr: t("Abandonner la question"),
    }
    CmdQuestionReopen "cmd.question_reopen" {
        en: t("Reopen question"),
        fr: t("Rouvrir la question"),
    }
    CmdQuestionMove "cmd.question_move" {
        en: t("Move question to…"),
        fr: t("Déplacer la question vers…"),
    }
    CmdQuestionDelete "cmd.question_delete" {
        en: t("Delete question"),
        fr: t("Supprimer la question"),
    }
    CmdContactsOpen "cmd.contacts_open" {
        en: t("Go to contacts"),
        fr: t("Aller aux contacts"),
    }
    CmdContactGo "cmd.contact_go" {
        en: t("Go to contact…"),
        fr: t("Aller à un contact…"),
    }
    CmdContactNew "cmd.contact_new" {
        en: t("New contact"),
        fr: t("Nouveau contact"),
    }
    CmdContactRename "cmd.contact_rename" {
        en: t("Rename contact"),
        fr: t("Renommer le contact"),
    }
    CmdContactJobTitle "cmd.contact_job_title" {
        en: t("Set job title…"),
        fr: t("Définir la fonction…"),
    }
    CmdContactCompany "cmd.contact_company" {
        en: t("Set company…"),
        fr: t("Définir l’entreprise…"),
    }
    CmdContactAddress "cmd.contact_address" {
        en: t("Edit how to address them"),
        fr: t("Modifier comment s’adresser à ce contact"),
    }
    CmdContactMove "cmd.contact_move" {
        en: t("Move contact to…"),
        fr: t("Déplacer le contact vers…"),
    }
    CmdContactMerge "cmd.contact_merge" {
        en: t("Merge contact into…"),
        fr: t("Fusionner le contact avec…"),
    }
    CmdContactDelete "cmd.contact_delete" {
        en: t("Delete contact"),
        fr: t("Supprimer le contact"),
    }
    CmdProfileName "cmd.profile_name" {
        en: t("Set your name"),
        fr: t("Définir votre nom"),
    }
    CmdProfileJobTitle "cmd.profile_job_title" {
        en: t("Set your job title"),
        fr: t("Définir votre fonction"),
    }
    CmdSettings "cmd.settings" {
        en: t("Open settings"),
        fr: t("Ouvrir les paramètres"),
    }
    CmdTheme "cmd.theme" {
        en: t("Theme: {value}"),
        fr: t("Thème\u{a0}: {value}"),
    }
    CmdTextSize "cmd.text_size" {
        en: t("Text size: {value}"),
        fr: t("Taille du texte\u{a0}: {value}"),
    }
    CmdReadingWidth "cmd.reading_width" {
        en: t("Reading width: {value}"),
        fr: t("Largeur de lecture\u{a0}: {value}"),
    }
    CmdLanguage "cmd.language" {
        en: t("Language: {value}"),
        fr: t("Langue\u{a0}: {value}"),
    }
    CmdNewProject "cmd.new_project" {
        en: t("New project\u{2026}"),
        fr: t("Nouveau projet\u{2026}"),
    }
    CmdTreeCollapseAll "cmd.tree_collapse_all" {
        en: t("Projects: Collapse all"),
        fr: t("Projets\u{a0}: tout replier"),
    }
    CmdNewEpic "cmd.new_epic" {
        en: t("New epic\u{2026}"),
        fr: t("Nouvel epic\u{2026}"),
    }
    CmdSwitchProject "cmd.switch_project" {
        en: t("Switch project\u{2026}"),
        fr: t("Changer de projet\u{2026}"),
    }
    CmdToggleSidebar "cmd.toggle_sidebar" {
        en: t("Toggle left sidebar"),
        fr: t("Afficher ou masquer la barre latérale gauche"),
    }
    CmdToggleTerminal "cmd.toggle_terminal" {
        en: t("Toggle terminal"),
        fr: t("Afficher ou masquer le terminal"),
    }
    CmdToggleChat "cmd.toggle_chat" {
        en: t("Toggle chat"),
        fr: t("Afficher ou masquer le chat"),
    }
    CmdToggleRight "cmd.toggle_right" {
        en: t("Toggle right sidebar"),
        fr: t("Afficher ou masquer la barre latérale droite"),
    }
    CmdToggleBottom "cmd.toggle_bottom" {
        en: t("Toggle bottom panel"),
        fr: t("Afficher ou masquer le panneau du bas"),
    }
    CmdViewShow "cmd.view_show" {
        en: t("{view}: Show"),
        fr: t("{view}\u{a0}: afficher"),
    }
    CmdViewHide "cmd.view_hide" {
        en: t("{view}: Hide"),
        fr: t("{view}\u{a0}: masquer"),
    }
    CmdViewLeft "cmd.view_left" {
        en: t("{view}: Move to left sidebar"),
        fr: t("{view}\u{a0}: déplacer dans la barre latérale gauche"),
    }
    CmdViewRight "cmd.view_right" {
        en: t("{view}: Move to right sidebar"),
        fr: t("{view}\u{a0}: déplacer dans la barre latérale droite"),
    }
    CmdViewBottom "cmd.view_bottom" {
        en: t("{view}: Move to bottom panel"),
        fr: t("{view}\u{a0}: déplacer dans le panneau du bas"),
    }
    CmdViewPage "cmd.view_page" {
        en: t("{view}: Move to page area"),
        fr: t("{view}\u{a0}: déplacer dans la zone des pages"),
    }
    CmdViewSplitOff "cmd.view_split_off" {
        en: t("{view}: Split from group"),
        fr: t("{view}\u{a0}: séparer du groupe"),
    }
    CmdSplitRight "cmd.split_right" {
        en: t("Split right"),
        fr: t("Diviser vers la droite"),
    }
    CmdSplitDown "cmd.split_down" {
        en: t("Split down"),
        fr: t("Diviser vers le bas"),
    }
    CmdTabNextGroup "cmd.tab_next_group" {
        en: t("Move tab to next group"),
        fr: t("Déplacer l’onglet dans le groupe suivant"),
    }
    CmdTabPrevGroup "cmd.tab_prev_group" {
        en: t("Move tab to previous group"),
        fr: t("Déplacer l’onglet dans le groupe précédent"),
    }
    CmdJoinGroups "cmd.join_groups" {
        en: t("Join groups"),
        fr: t("Réunir les groupes"),
    }
    CmdNextArea "cmd.next_area" {
        en: t("Focus next area"),
        fr: t("Aller à la zone suivante"),
    }
    CmdPrevArea "cmd.prev_area" {
        en: t("Focus previous area"),
        fr: t("Aller à la zone précédente"),
    }
    CmdGrowArea "cmd.grow_area" {
        en: t("Grow focused area"),
        fr: t("Agrandir la zone active"),
    }
    CmdShrinkArea "cmd.shrink_area" {
        en: t("Shrink focused area"),
        fr: t("Réduire la zone active"),
    }
    CmdLayoutUndo "cmd.layout_undo" {
        en: t("Undo layout change"),
        fr: t("Annuler le changement de disposition"),
    }
    CmdLayoutReset "cmd.layout_reset" {
        en: t("Reset layout"),
        fr: t("Réinitialiser la disposition"),
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
    CmdAttach "cmd.attach" {
        en: t("Attach files\u{2026}"),
        fr: t("Joindre des fichiers\u{2026}"),
    }
    CmdAttachmentSave "cmd.attachment_save" {
        en: t("Save file as\u{2026}"),
        fr: t("Enregistrer le fichier sous\u{2026}"),
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
    CmdFoldersBulk "cmd.folders_bulk" {
        en: p("Apply the same change to {n} folder", "Apply the same change to {n} folders"),
        fr: p("Appliquer le même changement à {n} dossier", "Appliquer le même changement à {n} dossiers"),
    }
    CmdFolderSwitch "cmd.folder_switch" {
        en: t("Switch active folder\u{2026}"),
        fr: t("Changer de dossier actif\u{2026}"),
    }
    CmdFolderActive "cmd.folder_active" {
        en: t("Make folder active"),
        fr: t("Rendre le dossier actif"),
    }
    CmdFolderSettings "cmd.folder_settings" {
        en: t("Folder settings\u{2026}"),
        fr: t("Réglages du dossier\u{2026}"),
    }
    CmdCommitMode "cmd.commit_mode" {
        en: t("Set commit mode\u{2026}"),
        fr: t("Définir le mode de commit\u{2026}"),
    }
    CmdCommitModeManual "cmd.commit_mode.manual" {
        en: t("Manual"),
        fr: t("Manuel"),
    }
    CmdCommitModeCommit "cmd.commit_mode.commit" {
        en: t("Commit automatically"),
        fr: t("Commiter automatiquement"),
    }
    CmdCommitModePush "cmd.commit_mode.push" {
        en: t("Commit and push automatically"),
        fr: t("Commiter et pousser automatiquement"),
    }
    CmdFolderPull "cmd.folder_pull" {
        en: t("Pull active folder"),
        fr: t("Pull du dossier actif"),
    }
    CmdFolderFetch "cmd.folder_fetch" {
        en: t("Fetch active folder"),
        fr: t("Fetch du dossier actif"),
    }
    CmdFolderBranch "cmd.folder_branch" {
        en: t("Switch branch of active folder\u{2026}"),
        fr: t("Changer de branche du dossier actif\u{2026}"),
    }
    CmdDictate "cmd.dictate" {
        en: t("Dictate"),
        fr: t("Dicter"),
    }
    CmdDictateStop "cmd.dictate_stop" {
        en: t("Stop dictating"),
        fr: t("Arrêter la dictée"),
    }
    CmdVoiceSettings "cmd.voice_settings" {
        en: t("Voice settings"),
        fr: t("Paramètres de la voix"),
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
    ReadyTitle "ready.title" {
        en: t("Ready for Claude Code"),
        fr: t("Prêt pour Claude Code"),
    }
    ReadyHint "ready.hint" {
        en: t("What Claude Code uses on this computer. Nothing is installed without your click."),
        fr: t("Ce que Claude Code utilise sur cet ordinateur. Rien n\u{2019}est installé sans votre clic."),
    }
    ReadyClaude "ready.claude" {
        en: t("Claude Code"),
        fr: t("Claude Code"),
    }
    ReadyClaudeWhy "ready.claude.why" {
        en: t("Runs in Acta\u{2019}s terminal and does the work. Required."),
        fr: t("S\u{2019}exécute dans le terminal d\u{2019}Acta et fait le travail. Requis."),
    }
    ReadySignin "ready.signin" {
        en: t("Signed in to Claude"),
        fr: t("Connecté à Claude"),
    }
    ReadySigninWhy "ready.signin.why" {
        en: t("Claude Code needs your Claude subscription. You sign in once, in Acta\u{2019}s terminal."),
        fr: t("Claude Code a besoin de votre abonnement Claude. Vous vous connectez une fois, dans le terminal d\u{2019}Acta."),
    }
    ReadyGit "ready.git" {
        en: t("Git"),
        fr: t("Git"),
    }
    ReadyGitWhy "ready.git.why" {
        en: t("Commits, branches and worktrees. Recommended."),
        fr: t("Commits, branches et worktrees. Recommandé."),
    }
    ReadyGitWhyWindows "ready.git.why_windows" {
        en: t("Commits and branches, and Git Bash, the shell Claude Code prefers on Windows. Recommended."),
        fr: t("Commits et branches, et Git Bash, le shell que Claude Code préfère sous Windows. Recommandé."),
    }
    ReadyGh "ready.gh" {
        en: t("GitHub CLI"),
        fr: t("GitHub CLI"),
    }
    ReadyGhWhy "ready.gh.why" {
        en: t("Lets Claude Code open pull requests and read issues on GitHub. Optional."),
        fr: t("Permet à Claude Code d\u{2019}ouvrir des pull requests et de lire les issues sur GitHub. Facultatif."),
    }
    ReadyStateFound "ready.state.found" {
        en: t("{version}"),
        fr: t("{version}"),
    }
    ReadyStateWithBash "ready.state.with_bash" {
        en: t("{version} · with Git Bash"),
        fr: t("{version} · avec Git Bash"),
    }
    ReadyStateWithoutBash "ready.state.without_bash" {
        en: t("{version} · without Git Bash: Claude Code will use PowerShell"),
        fr: t("{version} · sans Git Bash\u{a0}: Claude Code utilisera PowerShell"),
    }
    ReadyStateMissing "ready.state.missing" {
        en: t("Not installed"),
        fr: t("Non installé"),
    }
    ReadyStateMissingOptional "ready.state.missing_optional" {
        en: t("Not installed · optional"),
        fr: t("Non installé · facultatif"),
    }
    ReadyStateOutdated "ready.state.outdated" {
        en: t("{version} · {minimum} or later needed"),
        fr: t("{version} · {minimum} ou ultérieure requise"),
    }
    ReadyStateInstalling "ready.state.installing" {
        en: t("Installing\u{2026} {elapsed}"),
        fr: t("Installation\u{2026} {elapsed}"),
    }
    ReadyStateUpdating "ready.state.updating" {
        en: t("Updating\u{2026} {elapsed}"),
        fr: t("Mise à jour\u{2026} {elapsed}"),
    }
    ReadyStateTerminal "ready.state.terminal" {
        en: t("Finish in the terminal"),
        fr: t("Terminez dans le terminal"),
    }
    ReadyStateMacos "ready.state.macos" {
        en: t("Finish in the macOS window"),
        fr: t("Terminez dans la fenêtre de macOS"),
    }
    ReadyStateFailed "ready.state.failed" {
        en: t("Install failed"),
        fr: t("Échec de l\u{2019}installation"),
    }
    ReadyStateSignedIn "ready.state.signed_in" {
        en: t("Signed in"),
        fr: t("Connecté"),
    }
    ReadyStateApiKeyUsed "ready.state.api_key" {
        en: t("Using an API key"),
        fr: t("Clé API utilisée"),
    }
    ReadyStateApiKeyTip "ready.state.api_key.tip" {
        en: t("Acta works best with a Claude subscription."),
        fr: t("Acta fonctionne au mieux avec un abonnement Claude."),
    }
    ReadyStateSignedOut "ready.state.signed_out" {
        en: t("Not signed in"),
        fr: t("Non connecté"),
    }
    ReadyInstall "ready.install" {
        en: t("Install\u{2026}"),
        fr: t("Installer\u{2026}"),
    }
    ReadyUpdate "ready.update" {
        en: t("Update\u{2026}"),
        fr: t("Mettre à jour\u{2026}"),
    }
    ReadySigninButton "ready.signin_button" {
        en: t("Sign in\u{2026}"),
        fr: t("Se connecter\u{2026}"),
    }
    ReadySigninNeedsClaude "ready.signin.needs_claude" {
        en: t("Install Claude Code first"),
        fr: t("Installez d\u{2019}abord Claude Code"),
    }
    ReadyShowCommand "ready.show_command" {
        en: t("Show command"),
        fr: t("Afficher la commande"),
    }
    ReadyDownload "ready.download" {
        en: t("Open download page"),
        fr: t("Ouvrir la page de téléchargement"),
    }
    ReadyWingetInstead "ready.winget_instead" {
        en: t("Install with winget instead"),
        fr: t("Installer avec winget à la place"),
    }
    ReadyShowTerminal "ready.show_terminal" {
        en: t("Show terminal"),
        fr: t("Afficher le terminal"),
    }
    ReadyShowLog "ready.show_log" {
        en: t("Show log"),
        fr: t("Afficher le journal"),
    }
    ReadyHideLog "ready.hide_log" {
        en: t("Hide log"),
        fr: t("Masquer le journal"),
    }
    ReadyCopyLog "ready.copy_log" {
        en: t("Copy log"),
        fr: t("Copier le journal"),
    }
    ReadyCopyCommand "ready.copy_command" {
        en: t("Copy command"),
        fr: t("Copier la commande"),
    }
    ReadyCheckAgain "ready.check_again" {
        en: t("Check again"),
        fr: t("Vérifier à nouveau"),
    }
    ReadyOneAtATime "ready.one_at_a_time" {
        en: t("One install at a time"),
        fr: t("Une installation à la fois"),
    }
    ReadyFailedExit "ready.failed.exit" {
        en: t("The installer stopped with code {code}."),
        fr: t("Le programme d\u{2019}installation s\u{2019}est arrêté avec le code {code}."),
    }
    ReadyFailedNotFound "ready.failed.not_found" {
        en: t("Installed, but Acta cannot find {name} yet. Restart Acta."),
        fr: t("Installé, mais Acta ne trouve pas encore {name}. Redémarrez Acta."),
    }
    ReadyFailedNoStart "ready.failed.no_start" {
        en: t("Could not start the installer."),
        fr: t("Impossible de lancer le programme d\u{2019}installation."),
    }
    ReadyNoManager "ready.no_manager" {
        en: t("No package manager Acta can use was found here. Install {name} from its download page, then check again."),
        fr: t("Aucun gestionnaire de paquets utilisable par Acta n\u{2019}a été trouvé. Installez {name} depuis sa page de téléchargement, puis vérifiez à nouveau."),
    }
    ReadyDialogInstall "ready.dialog.install" {
        en: t("Install {name}?"),
        fr: t("Installer {name}\u{202f}?"),
    }
    ReadyDialogUpdate "ready.dialog.update" {
        en: t("Update {name}?"),
        fr: t("Mettre à jour {name}\u{202f}?"),
    }
    ReadyFromVendor "ready.from.vendor" {
        en: t("Acta runs {name}\u{2019}s official installer, exactly as its documentation gives it."),
        fr: t("Acta exécute le programme d\u{2019}installation officiel de {name}, tel que le donne sa documentation."),
    }
    ReadyFromWinget "ready.from.winget" {
        en: t("Acta runs Windows Package Manager (winget), which downloads {name} from its publisher and checks it against the hash in winget\u{2019}s catalogue."),
        fr: t("Acta exécute le gestionnaire de paquets de Windows (winget), qui télécharge {name} depuis son éditeur et le vérifie avec l\u{2019}empreinte du catalogue de winget."),
    }
    ReadyFromHomebrew "ready.from.homebrew" {
        en: t("Acta runs Homebrew, which downloads {name} and checks it."),
        fr: t("Acta exécute Homebrew, qui télécharge {name} et le vérifie."),
    }
    ReadyFromApple "ready.from.apple" {
        en: t("macOS installs Apple\u{2019}s Command Line Tools, which include Git, in its own window."),
        fr: t("macOS installe les Command Line Tools d\u{2019}Apple, qui contiennent Git, dans sa propre fenêtre."),
    }
    ReadyFromSystem "ready.from.system" {
        en: t("Your system\u{2019}s package manager installs it, in Acta\u{2019}s terminal. It asks for your password there; Acta never sees it."),
        fr: t("Le gestionnaire de paquets de votre système l\u{2019}installe, dans le terminal d\u{2019}Acta. Il y demande votre mot de passe\u{202f}; Acta ne le voit jamais."),
    }
    ReadyScopeUser "ready.scope.user" {
        en: t("For you only. No administrator rights needed."),
        fr: t("Pour vous seulement. Aucun droit d\u{2019}administrateur nécessaire."),
    }
    ReadyScopeUserUac "ready.scope.user_uac" {
        en: t("For you only. Windows may still ask for permission."),
        fr: t("Pour vous seulement. Windows peut tout de même demander une autorisation."),
    }
    ReadyScopeSystem "ready.scope.system" {
        en: t("For all users of this computer. Administrator rights needed."),
        fr: t("Pour tous les utilisateurs de cet ordinateur. Droits d\u{2019}administrateur nécessaires."),
    }
    ReadyAgreements "ready.agreements" {
        en: t("Installing accepts the publisher\u{2019}s licence and the terms of winget\u{2019}s catalogue."),
        fr: t("Installer vaut acceptation de la licence de l\u{2019}éditeur et des conditions du catalogue de winget."),
    }
    ReadyUpdatesSelf "ready.updates.self" {
        en: t("Claude Code then updates itself."),
        fr: t("Claude Code se met ensuite à jour tout seul."),
    }
    ReadyUpdatesWinget "ready.updates.winget" {
        en: t("Updates come from winget, not from Acta."),
        fr: t("Les mises à jour viennent de winget, pas d\u{2019}Acta."),
    }
    ReadyUpdatesHomebrew "ready.updates.homebrew" {
        en: t("Updates come from Homebrew, not from Acta."),
        fr: t("Les mises à jour viennent de Homebrew, pas d\u{2019}Acta."),
    }
    ReadyUpdatesApple "ready.updates.apple" {
        en: t("Updates come from macOS Software Update."),
        fr: t("Les mises à jour viennent de la mise à jour de logiciels de macOS."),
    }
    ReadyUpdatesSystem "ready.updates.system" {
        en: t("Updates come with your system\u{2019}s updates, not from Acta."),
        fr: t("Les mises à jour arrivent avec celles de votre système, pas d\u{2019}Acta."),
    }
    ReadyUninstall "ready.uninstall" {
        en: t("To remove it later, use {manager}; removing Acta leaves it installed."),
        fr: t("Pour le retirer plus tard, utilisez {manager}\u{202f}; désinstaller Acta le laisse en place."),
    }
    ReadyUninstallNative "ready.uninstall.native" {
        en: t("the steps on Claude Code\u{2019}s setup page"),
        fr: t("les étapes de la page d\u{2019}installation de Claude Code"),
    }
    ReadyDialogCommand "ready.dialog.command" {
        en: t("Command"),
        fr: t("Commande"),
    }
    ReadyDialogConfirmInstall "ready.dialog.confirm_install" {
        en: t("Install"),
        fr: t("Installer"),
    }
    ReadyDialogConfirmUpdate "ready.dialog.confirm_update" {
        en: t("Update"),
        fr: t("Mettre à jour"),
    }
    ReadyDialogConfirmTerminal "ready.dialog.confirm_terminal" {
        en: t("Open in terminal"),
        fr: t("Ouvrir dans le terminal"),
    }
    ReadySigninHint "ready.signin.hint" {
        en: t("A terminal opens with Claude Code. Follow its steps in your browser; Acta never sees your password or token."),
        fr: t("Un terminal s\u{2019}ouvre avec Claude Code. Suivez ses étapes dans votre navigateur\u{202f}; Acta ne voit jamais votre mot de passe ni votre jeton."),
    }
    SettingsAppearance "settings.appearance" {
        en: t("Appearance"),
        fr: t("Apparence"),
    }
    SettingsProfile "settings.profile" {
        en: t("Profile"),
        fr: t("Profil"),
    }
    SettingsProfileHint "settings.profile.hint" {
        en: t("Claude Code sees your name and job title."),
        fr: t("Claude Code voit votre nom et votre fonction."),
    }
    SettingsProfileName "settings.profile.name" {
        en: t("Name"),
        fr: t("Nom"),
    }
    SettingsProfileNameHint "settings.profile.name_hint" {
        en: t("Shown on what you write."),
        fr: t("Affiché sur ce que vous écrivez."),
    }
    SettingsProfileJobTitle "settings.profile.job_title" {
        en: t("Job title"),
        fr: t("Fonction"),
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
    SettingsLanguage "settings.language" {
        en: t("Language"),
        fr: t("Langue"),
    }
    SettingsLanguageHint "settings.language.hint" {
        en: t("What you and Claude Code write is never translated."),
        fr: t("Ce que vous et Claude Code écrivez n\u{2019}est jamais traduit."),
    }
    SettingsTextSize "settings.text_size" {
        en: t("Text size"),
        fr: t("Taille du texte"),
    }
    SettingsTextSizeHint "settings.text_size.hint" {
        en: t("Timelines, notes and documents."),
        fr: t("Journaux, notes et documents."),
    }
    SettingsTextSizeSmall "settings.text_size.small" {
        en: t("Small"),
        fr: t("Petit"),
    }
    SettingsTextSizeDefault "settings.text_size.default" {
        en: t("Default"),
        fr: t("Normal"),
    }
    SettingsTextSizeLarge "settings.text_size.large" {
        en: t("Large"),
        fr: t("Grand"),
    }
    SettingsReadingWidth "settings.reading_width" {
        en: t("Reading width"),
        fr: t("Largeur de lecture"),
    }
    SettingsReadingWidthNarrow "settings.reading_width.narrow" {
        en: t("Narrow"),
        fr: t("Étroite"),
    }
    SettingsReadingWidthDefault "settings.reading_width.default" {
        en: t("Default"),
        fr: t("Normale"),
    }
    SettingsReadingWidthWide "settings.reading_width.wide" {
        en: t("Wide"),
        fr: t("Large"),
    }
    SettingsLayout "settings.layout" {
        en: t("Layout"),
        fr: t("Disposition"),
    }
    SettingsLayoutDesc "settings.layout.desc" {
        en: t("Puts every view back in its default place."),
        fr: t("Remet chaque vue à sa place par défaut."),
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
    SettingsAutoCommit "settings.auto_commit" {
        en: t("Automatic commits"),
        fr: t("Commits automatiques"),
    }
    SettingsAutoCommitHint "settings.auto_commit.hint" {
        en: t("Default for folders that commit automatically."),
        fr: t("Par défaut pour les dossiers qui commitent automatiquement."),
    }
    SettingsClaude "settings.claude" {
        en: t("Claude Code"),
        fr: t("Claude Code"),
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
    SettingsCheckUnknown "settings.check.unknown" {
        en: t("Could not check"),
        fr: t("Vérification impossible"),
    }
    SettingsStatusReady "settings.status.ready" {
        en: t("Acta is set up in Claude Code"),
        fr: t("Acta est configuré dans Claude Code"),
    }
    SettingsStatusPartial "settings.status.partial" {
        en: p("Setup incomplete: {n} of 3 step done", "Setup incomplete: {n} of 3 steps done"),
        fr: p("Configuration incomplète\u{a0}: {n} étape sur 3 faite", "Configuration incomplète\u{a0}: {n} étapes sur 3 faites"),
    }
    SettingsStatusUnknown "settings.status.unknown" {
        en: t("Could not check the setup"),
        fr: t("Impossible de vérifier la configuration"),
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
        en: t("Lets Claude Code read and update your projects in Acta."),
        fr: t("Permet à Claude Code de lire et de mettre à jour vos projets dans Acta."),
    }
    SettingsSetupWhat "settings.setup.what" {
        en: t("What setup changes"),
        fr: t("Ce que change la configuration"),
    }
    SettingsSetupDetails "settings.setup.details" {
        en: t("Setup installs Acta\u{2019}s plugin in Claude Code, adds two permission lines (Acta and its browser) and Acta\u{2019}s status line to ~/.claude/settings.json (after your own status line, if you have one), and turns Claude Code\u{2019}s own memory off in the open project\u{2019}s folders. Remove undoes all of it."),
        fr: t("La configuration installe le plugin d\u{2019}Acta dans Claude Code, ajoute deux lignes d\u{2019}autorisation (Acta et son navigateur) et la ligne d\u{2019}état d\u{2019}Acta à ~/.claude/settings.json (après votre propre ligne d\u{2019}état, si vous en avez une), et désactive la mémoire propre à Claude Code dans les dossiers du projet ouvert. Retirer annule tout cela."),
    }
    SettingsRemoveTitle "settings.remove.title" {
        en: t("Remove Acta from Claude Code?"),
        fr: t("Retirer Acta de Claude Code\u{202f}?"),
    }
    SettingsRemoveBody "settings.remove.body" {
        en: t("Removes Acta\u{2019}s plugin, permissions and status line from Claude Code, and turns its own memory back on. Your projects stay in Acta."),
        fr: t("Retire de Claude Code le plugin, les autorisations et la ligne d\u{2019}état d\u{2019}Acta, et réactive sa propre mémoire. Vos projets restent dans Acta."),
    }
    SettingsRemoveConfirm "settings.remove.confirm" {
        en: t("Remove"),
        fr: t("Retirer"),
    }
    SettingsData "settings.data" {
        en: t("Data"),
        fr: t("Données"),
    }
    SettingsExportLast "settings.export.last" {
        en: t("Last export: {days}"),
        fr: t("Dernier export\u{a0}: {days}"),
    }
    SettingsExportNever "settings.export.never" {
        en: t("Never exported"),
        fr: t("Jamais exporté"),
    }
    SettingsSnapshotsToday "settings.snapshots.today" {
        en: p("{n} snapshot, from today, {time}", "{n} snapshots, newest today, {time}"),
        fr: p("{n} instantané, d\u{2019}aujourd\u{2019}hui à {time}", "{n} instantanés, le plus récent d\u{2019}aujourd\u{2019}hui à {time}"),
    }
    SettingsSnapshotsOlder "settings.snapshots.older" {
        en: p("{n} snapshot, from {datetime}", "{n} snapshots, newest {datetime}"),
        fr: p("{n} instantané, du {datetime}", "{n} instantanés, le plus récent du {datetime}"),
    }
    SettingsSnapshots "settings.snapshots" {
        en: t("Snapshots"),
        fr: t("Instantanés"),
    }
    SettingsRestore "settings.restore" {
        en: t("Restore\u{2026}"),
        fr: t("Restaurer\u{2026}"),
    }
    SettingsTrashCount "settings.trash_count" {
        en: p("{n} item", "{n} items"),
        fr: p("{n} élément", "{n} éléments"),
    }
    SettingsDatabase "settings.database" {
        en: t("Database"),
        fr: t("Base de données"),
    }
    SettingsAttachments "settings.attachments" {
        en: t("Attached files: {size} in {count}"),
        fr: t("Fichiers joints\u{a0}: {size} dans {count}"),
    }
    SettingsChat "settings.chat" {
        en: t("Chat"),
        fr: t("Chat"),
    }
    SettingsChatData "settings.chat_data" {
        en: t("Chat data"),
        fr: t("Données du chat"),
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
    SettingsLicencesHint "settings.licences.hint" {
        en: t("JetBrains Mono under the SIL Open Font License, Lucide under ISC."),
        fr: t("JetBrains Mono sous licence SIL Open Font License, Lucide sous licence ISC."),
    }
    SettingsLicencesHintLinux "settings.licences.hint_linux" {
        en: t("Inter and JetBrains Mono under the SIL Open Font License, Lucide under ISC."),
        fr: t("Inter et JetBrains Mono sous licence SIL Open Font License, Lucide sous licence ISC."),
    }
    StatusbarNotSetUp "statusbar.not_set_up" {
        en: t("Claude Code: not set up"),
        fr: t("Claude Code\u{a0}: non configuré"),
    }
    StatusbarNotFound "statusbar.not_found" {
        en: t("Claude Code: not found"),
        fr: t("Claude Code\u{a0}: introuvable"),
    }
    StatusbarNoFolder "statusbar.no_folder" {
        en: t("Claude Code: no folder"),
        fr: t("Claude Code\u{a0}: aucun dossier"),
    }
    StatusbarReady "statusbar.ready" {
        en: t("Claude Code: ready"),
        fr: t("Claude Code\u{a0}: prêt"),
    }
    StatusbarLastWrote "statusbar.last_wrote" {
        en: t("Last wrote {when} in {topic}"),
        fr: t("Dernière écriture {when} dans {topic}"),
    }
    StatusbarVersion "statusbar.version" {
        en: t("Acta {version}"),
        fr: t("Acta {version}"),
    }
    StatusClaudeMissing "status.claude.missing" {
        en: t("Claude Code: not installed"),
        fr: t("Claude Code\u{a0}: non installé"),
    }
    StatusClaudeSignedOut "status.claude.signed_out" {
        en: t("Claude Code: not signed in"),
        fr: t("Claude Code\u{a0}: non connecté"),
    }
    UpdateReady "update.ready" {
        en: t("Restart to update"),
        fr: t("Redémarrer pour mettre à jour"),
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
        en: t("The file is not encrypted: anyone who opens it can read these projects."),
        fr: t("Le fichier n\u{2019}est pas chiffré\u{a0}: quiconque l\u{2019}ouvre peut lire ces projets."),
    }
    ExportIncludeFiles "export.include_files" {
        en: t("Include attached files ({size})"),
        fr: t("Inclure les fichiers joints ({size})"),
    }
    ExportTooLarge "export.too_large" {
        en: t("With its files this export would be about {size}, more than an import can read ({limit}). Export without files: their names and sizes stay."),
        fr: t("Avec ses fichiers, cet export ferait environ {size}, plus qu\u{2019}un import ne peut en lire ({limit}). Exportez sans les fichiers\u{a0}: leurs noms et tailles sont conservés."),
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
        en: t("Everything is on this computer only. An export is your copy if the keychain is lost."),
        fr: t("Tout est sur cet ordinateur uniquement. Un export est votre copie si le trousseau est perdu."),
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
        en: t("Nothing will be deleted. Skipped records are the same as Acta\u{2019}s, or older."),
        fr: t("Rien ne sera supprimé. Les éléments ignorés sont identiques à ceux d\u{2019}Acta, ou plus anciens."),
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
        fr: t("Import terminé\u{a0}: {created}, {updated}. Un instantané d\u{2019}avant l\u{2019}import est conservé."),
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
    ImportNetworkFolders "import.network_folders" {
        en: p("{n} working folder on a network path was left out. Add it again from its project, epic or topic.", "{n} working folders on a network path were left out. Add each one again from its project, epic or topic."),
        fr: p("{n} dossier de travail sur un chemin réseau a été écarté. Ajoutez-le de nouveau depuis son projet, son epic ou son sujet.", "{n} dossiers de travail sur un chemin réseau ont été écartés. Ajoutez chacun de nouveau depuis son projet, son epic ou son sujet."),
    }
    ImportFilesMissing "import.files_missing" {
        en: p("{n} file was not in the export.", "{n} files were not in the export."),
        fr: p("{n} fichier n\u{2019}était pas dans l\u{2019}export.", "{n} fichiers n\u{2019}étaient pas dans l\u{2019}export."),
    }
    ImportRecoveredManual "import.recovered_manual" {
        en: t("Commit modes come back as Manual: set them again in each folder\u{2019}s settings."),
        fr: t("Les modes de commit reviennent en Manuel\u{a0}: redéfinissez-les dans les réglages de chaque dossier."),
    }
    ImportNewer "import.newer" {
        en: t("This file was made by a newer Acta (format {n}). Update Acta, then import it."),
        fr: t("Ce fichier a été créé par une version plus récente d\u{2019}Acta (format {n}). Mettez Acta à jour, puis importez-le."),
    }
    ImportNotActa "import.not_acta" {
        en: t("This is not an Acta export."),
        fr: t("Ce n\u{2019}est pas un export Acta."),
    }
    ImportTooLarge "import.too_large" {
        en: t("This file is too large to import ({size}). The limit is {limit}. Export one project at a time, then import each file."),
        fr: t("Ce fichier est trop volumineux pour être importé ({size}). La limite est de {limit}. Exportez un projet à la fois, puis importez chaque fichier."),
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
    TrashMergedInto "trash.merged_into" {
        en: t("Merged into {name}"),
        fr: t("Fusionné avec : {name}"),
    }
    TrashKeptNamed "trash.kept_named" {
        en: p("Kept: still named in {n} place", "Kept: still named in {n} places"),
        fr: p("Conservé : encore cité à {n} endroit", "Conservé : encore cité à {n} endroits"),
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
        en: t("Restore from snapshot"),
        fr: t("Restaurer depuis un instantané"),
    }
    SnapshotBody "snapshot.body" {
        en: t("Acta will restart with its data as of {datetime}. Everything as it is now is saved as a snapshot first, so you can come back."),
        fr: t("Acta redémarrera avec ses données du {datetime}. La situation actuelle est d\u{2019}abord enregistrée dans un instantané\u{a0}: vous pourrez y revenir."),
    }
    SnapshotRestored "snapshot.restored" {
        en: t("Restored the snapshot from {datetime}."),
        fr: t("Instantané du {datetime} restauré."),
    }
    SnapshotMigration "snapshot.migration" {
        en: t("The update to {version} could not convert the database. Acta put back the snapshot from just before; nothing was lost."),
        fr: t("La mise à jour vers {version} n\u{2019}a pas pu convertir la base de données. Acta a restauré l\u{2019}instantané pris juste avant\u{202f}; rien n\u{2019}a été perdu."),
    }
    SnapshotSize "snapshot.size" {
        en: t("{size} MB"),
        fr: t("{size} Mo"),
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
    KeylossBodyUnreadable "keyloss.body.unreadable" {
        en: t("Acta found its key, but the database cannot be read with it: the file may be damaged, or the key belongs to another database. This can follow a disk error or a copy of the file from another computer."),
        fr: t("Acta a trouvé sa clé, mais elle ne permet pas de lire la base de données\u{a0}: le fichier est peut-être endommagé, ou la clé appartient à une autre base. Cela peut suivre une erreur de disque ou une copie du fichier depuis un autre ordinateur."),
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
        fr: t("Acta n\u{2019}a pas modifié le fichier\u{a0}:"),
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
    OpenfailShowLog "openfail.show_log" {
        en: t("Show the log"),
        fr: t("Afficher le journal"),
    }
    FirstrunTitle "firstrun.title" {
        en: t("Start with a project"),
        fr: t("Commencez par un projet"),
    }
    FirstrunBody "firstrun.body" {
        en: t("A project is a folder you work on with Claude Code. Acta keeps its topics, decisions and notes."),
        fr: t("Un projet est un dossier sur lequel vous travaillez avec Claude Code. Acta en garde les sujets, les décisions et les notes."),
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
        en: t("Tabs, views and layout"),
        fr: t("Onglets, vues et disposition"),
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
    ShortcutsPalette "shortcuts.palette" {
        en: t("Command palette"),
        fr: t("Palette de commandes"),
    }
    ShortcutsNextTab "shortcuts.next_tab" {
        en: t("Next tab"),
        fr: t("Onglet suivant"),
    }
    ShortcutsPreviousTab "shortcuts.previous_tab" {
        en: t("Previous tab"),
        fr: t("Onglet précédent"),
    }
    ShortcutsFocusProjects "shortcuts.focus_projects" {
        en: t("Focus Projects"),
        fr: t("Aller aux projets"),
    }
    ShortcutsFocusPage "shortcuts.focus_page" {
        en: t("Focus the page area"),
        fr: t("Aller à la zone des pages"),
    }
    ShortcutsFocusTerminal "shortcuts.focus_terminal" {
        en: t("Focus the terminal"),
        fr: t("Aller au terminal"),
    }
    ShortcutsFocusChat "shortcuts.focus_chat" {
        en: t("Focus the chat"),
        fr: t("Aller au chat"),
    }
    ShortcutsNextArea "shortcuts.next_area" {
        en: t("Focus next area (reserved)"),
        fr: t("Aller à la zone suivante (réservé)"),
    }
    ShortcutsPreviousArea "shortcuts.previous_area" {
        en: t("Focus previous area (reserved)"),
        fr: t("Aller à la zone précédente (réservé)"),
    }
    ShortcutsMaximize "shortcuts.maximize" {
        en: t("Maximize focused area (reserved)"),
        fr: t("Agrandir au maximum la zone active (réservé)"),
    }
    ShortcutsListMove "shortcuts.list.move" {
        en: t("Move the cursor"),
        fr: t("Déplacer le curseur"),
    }
    ShortcutsListEnds "shortcuts.list.ends" {
        en: t("First or last row"),
        fr: t("Première ou dernière ligne"),
    }
    ShortcutsListFold "shortcuts.list.fold" {
        en: t("Collapse or expand"),
        fr: t("Replier ou déplier"),
    }
    ShortcutsListPreview "shortcuts.list.preview" {
        en: t("Preview, focus stays in the list"),
        fr: t("Aperçu, le focus reste dans la liste"),
    }
    ShortcutsListStatus "shortcuts.list.status" {
        en: t("Set status, then 1 to 6"),
        fr: t("Changer le statut, puis 1 à 6"),
    }
    ShortcutsListReorder "shortcuts.list.reorder" {
        en: t("Move up or down"),
        fr: t("Monter ou descendre"),
    }
    ShortcutsListMenu "shortcuts.list.menu" {
        en: t("Open the menu"),
        fr: t("Ouvrir le menu"),
    }
    ShortcutsEditorSave "shortcuts.editor.save" {
        en: t("Save the editor in focus"),
        fr: t("Enregistrer l\u{2019}éditeur actif"),
    }
    ShortcutsEditorCancel "shortcuts.editor.cancel" {
        en: t("Cancel the edit"),
        fr: t("Annuler la modification"),
    }
    ViewAgentBrowser "view.agent_browser" {
        en: t("Agent browser"),
        fr: t("Navigateur de l\u{2019}agent"),
    }
    BrowserAddress "browser.address" {
        en: t("Address"),
        fr: t("Adresse"),
    }
    BrowserIdleTitle "browser.idle.title" {
        en: t("Claude Code\u{2019}s browser"),
        fr: t("Le navigateur de Claude Code"),
    }
    BrowserIdleBody "browser.idle.body" {
        en: t("Claude Code opens pages here: your dev server, or sites you signed into here. It never opens claude.ai and never types passwords."),
        fr: t("Claude Code ouvre ses pages ici\u{a0}: votre serveur de développement, ou les sites auxquels vous vous êtes connecté ici. Il n\u{2019}ouvre jamais claude.ai et ne saisit jamais de mot de passe."),
    }
    BrowserStarting "browser.starting" {
        en: t("Starting the browser\u{2026}"),
        fr: t("Démarrage du navigateur\u{2026}"),
    }
    BrowserHidden "browser.hidden" {
        en: t("The browser is hidden while this is open."),
        fr: t("Le navigateur est masqué tant que ceci est ouvert."),
    }
    BrowserStop "browser.stop" {
        en: t("Stop Claude Code"),
        fr: t("Arrêter Claude Code"),
    }
    BrowserResume "browser.resume" {
        en: t("Resume"),
        fr: t("Reprendre"),
    }
    BrowserStopped "browser.stopped" {
        en: t("Stopped. Claude Code is told to ask you before it continues."),
        fr: t("Arrêté. Claude Code doit vous demander avant de continuer."),
    }
    BrowserHandoff "browser.handoff" {
        en: t("Claude Code asks you: {message}"),
        fr: t("Claude Code vous demande\u{a0}: {message}"),
    }
    BrowserDone "browser.done" {
        en: t("Done"),
        fr: t("Terminé"),
    }
    BrowserDriver "browser.driver" {
        en: t("driven by {session}"),
        fr: t("piloté par {session}"),
    }
    BrowserDriverUnnamed "browser.driver.unnamed" {
        en: t("driven by Claude Code"),
        fr: t("piloté par Claude Code"),
    }
    BrowserActOpened "browser.act.opened" {
        en: t("Claude Code opened {target}"),
        fr: t("Claude Code a ouvert {target}"),
    }
    BrowserActReloaded "browser.act.reloaded" {
        en: t("Claude Code reloaded the page"),
        fr: t("Claude Code a rechargé la page"),
    }
    BrowserActBack "browser.act.back" {
        en: t("Claude Code went back"),
        fr: t("Claude Code est revenu en arrière"),
    }
    BrowserActForward "browser.act.forward" {
        en: t("Claude Code went forward"),
        fr: t("Claude Code est allé en avant"),
    }
    BrowserActClicked "browser.act.clicked" {
        en: t("Claude Code clicked {target}"),
        fr: t("Claude Code a cliqué sur {target}"),
    }
    BrowserActTyped "browser.act.typed" {
        en: t("Claude Code typed into {target}"),
        fr: t("Claude Code a saisi du texte dans {target}"),
    }
    BrowserActScreenshot "browser.act.screenshot" {
        en: t("Claude Code took a screenshot"),
        fr: t("Claude Code a pris une capture d\u{2019}écran"),
    }
    BrowserActWaiting "browser.act.waiting" {
        en: t("Claude Code is waiting for you"),
        fr: t("Claude Code vous attend"),
    }
    BrowserClear "browser.clear" {
        en: t("Clear browser data\u{2026}"),
        fr: t("Effacer les données du navigateur\u{2026}"),
    }
    BrowserClearTitle "browser.clear.title" {
        en: t("Clear the browser\u{2019}s data?"),
        fr: t("Effacer les données du navigateur\u{202f}?"),
    }
    BrowserClearBody "browser.clear.body" {
        en: t("This deletes the cookies and site data of Acta\u{2019}s browser from this computer: you are signed out of every site you signed into there."),
        fr: t("Cela supprime de cet ordinateur les cookies et les données de site du navigateur d\u{2019}Acta\u{a0}: vous êtes déconnecté de tous les sites auxquels vous vous y étiez connecté."),
    }
    BrowserClearConfirm "browser.clear.confirm" {
        en: t("Clear data"),
        fr: t("Effacer les données"),
    }
    BrowserClearFailed "browser.clear_failed" {
        en: t("The browser data could not be deleted. Close what may use it and try again."),
        fr: t("Les données du navigateur n\u{2019}ont pas pu être supprimées. Fermez ce qui peut les utiliser et réessayez."),
    }
    BrowserRefusedTitle "browser.refused.title" {
        en: t("Acta did not start its browser"),
        fr: t("Acta n\u{2019}a pas démarré son navigateur"),
    }
    BrowserRefusedBody "browser.refused.body" {
        en: t("A debugging port or another WebView2 override is configured for Acta (a WebView2 policy, an environment variable or a DevToolsActivePort file). It could let another program control or read the browser. Remove it, then try again."),
        fr: t("Un port de débogage ou un autre réglage WebView2 est configuré pour Acta (une stratégie WebView2, une variable d\u{2019}environnement ou un fichier DevToolsActivePort). Il pourrait permettre à un autre programme de piloter ou de lire le navigateur. Supprimez-le, puis réessayez."),
    }
    BrowserRuntimeTitle "browser.runtime.title" {
        en: t("The agent browser needs WebView2"),
        fr: t("Le navigateur de l\u{2019}agent a besoin de WebView2"),
    }
    BrowserRuntimeBody "browser.runtime.body" {
        en: t("WebView2 is not installed, so the browser cannot start. Everything else works."),
        fr: t("WebView2 n\u{2019}est pas installé\u{a0}: le navigateur ne peut pas démarrer. Tout le reste fonctionne."),
    }
    BrowserFailedTitle "browser.failed.title" {
        en: t("The browser could not be started"),
        fr: t("Le navigateur n\u{2019}a pas pu être démarré"),
    }
    BrowserPipeTaken "browser.pipe.taken" {
        en: t("Claude Code cannot reach this browser: another copy of Acta, or another program, already serves it. Close it and restart Acta."),
        fr: t("Claude Code ne peut pas joindre ce navigateur\u{a0}: une autre copie d\u{2019}Acta, ou un autre programme, le sert déjà. Fermez-le et redémarrez Acta."),
    }
    BrowserPipeFailed "browser.pipe.failed" {
        en: t("Claude Code cannot reach this browser. Restart Acta; the log says why."),
        fr: t("Claude Code ne peut pas joindre ce navigateur. Redémarrez Acta\u{202f}; le journal indique pourquoi."),
    }
    BrowserDevtools "browser.devtools" {
        en: t("Open developer tools"),
        fr: t("Ouvrir les outils de développement"),
    }
    CmdBrowserDevtools "cmd.browser_devtools" {
        en: t("Agent browser: Open developer tools"),
        fr: t("Navigateur de l\u{2019}agent\u{a0}: ouvrir les outils de développement"),
    }
    ToolOwner "tool.owner" {
        en: t("{level} {id} {title}"),
        fr: t("{level} {id} {title}"),
    }
    ToolClaudeN "tool.claude_n" {
        en: t("Claude {n}"),
        fr: t("Claude {n}"),
    }
    ToolChatN "tool.chat_n" {
        en: t("Chat {n}"),
        fr: t("Chat {n}"),
    }
    ToolBrowserN "tool.browser_n" {
        en: t("Browser {n}"),
        fr: t("Navigateur {n}"),
    }
    ToolBrowserFor "tool.browser_for" {
        en: t("Browser for Claude {n}"),
        fr: t("Navigateur de Claude {n}"),
    }
    ToolDrivenBy "tool.driven_by" {
        en: t("driven by Claude {n}"),
        fr: t("piloté par Claude {n}"),
    }
    ToolClaudeStart "tool.claude_start" {
        en: t("Start Claude Code here"),
        fr: t("Démarrer Claude Code ici"),
    }
    ToolClaudeNew "tool.claude_new" {
        en: t("New Claude Code session here"),
        fr: t("Nouvelle session Claude Code ici"),
    }
    ToolClaudeNone "tool.claude_none" {
        en: t("No Claude Code on this {level_lower}"),
        fr: p_by("level_lower", "Pas de Claude Code sur ce {level_lower}", "Pas de Claude Code sur cet {level_lower}"),
    }
    ToolClaudeNoneBody "tool.claude_none_body" {
        en: t("Starts in {folder} with what this {level_lower} knows."),
        fr: t("Démarre dans {folder} avec ce que sait ce {level_lower}."),
    }
    ToolNoOwner "tool.no_owner" {
        en: t("Claude Code belongs to a project, an epic or a topic. Open one to start it or see its sessions."),
        fr: t("Claude Code appartient à un projet, un epic ou un sujet. Ouvrez-en un pour le démarrer ou voir ses sessions."),
    }
    ToolChatNew "tool.chat_new" {
        en: t("New chat here"),
        fr: t("Nouveau chat ici"),
    }
    ToolBrowserNew "tool.browser_new" {
        en: t("New browser here"),
        fr: t("Nouveau navigateur ici"),
    }
    ToolRestartIn "tool.restart_in" {
        en: t("Restart in {folder}"),
        fr: t("Redémarrer dans {folder}"),
    }
    ToolLimit "tool.limit" {
        en: t("{n} is the most on one page."),
        fr: t("{n} au maximum sur une même page."),
    }
    ToolFree "tool.free" {
        en: t("Claude Code (no project)"),
        fr: t("Claude Code (sans projet)"),
    }
    ToolFreeContext "tool.free_context" {
        en: t("In your home folder, without Acta\u{2019}s context"),
        fr: t("Dans votre dossier personnel, sans le contexte d\u{2019}Acta"),
    }
    ToolFreeTitle "tool.free_title" {
        en: t("Claude Code without a project"),
        fr: t("Claude Code sans projet"),
    }
    ToolFreeBody "tool.free_body" {
        en: t("For work that needs no project: it starts in your home folder and knows nothing about Acta."),
        fr: t("Pour un travail sans projet\u{a0}: il démarre dans votre dossier personnel et ne sait rien d\u{2019}Acta."),
    }
    ToolFreeStart "tool.free_start" {
        en: t("Start"),
        fr: t("Démarrer"),
    }
    ToolFreeNew "tool.free_new" {
        en: t("New Claude Code (no project)"),
        fr: t("Nouveau Claude Code (sans projet)"),
    }
    CmdClaudeRestart "cmd.claude_restart" {
        en: t("Restart Claude Code"),
        fr: t("Redémarrer Claude Code"),
    }
    ChatClearAllBody "chat.clear_all_body" {
        en: t("This deletes the cookies and site data of every chat from this computer. Your conversations stay in your Claude account."),
        fr: t("Cela supprime de cet ordinateur les cookies et données de site de tous les chats. Vos conversations restent dans votre compte Claude."),
    }
    TerminalClaude "terminal.claude" {
        en: t("Claude"),
        fr: t("Claude"),
    }
    TerminalWaiting "terminal.waiting" {
        en: t("waiting for you"),
        fr: t("vous attend"),
    }
    ViewSessions "view.sessions" {
        en: t("Claude Code sessions"),
        fr: t("Sessions Claude Code"),
    }
    SessionsWorking "sessions.working" {
        en: t("{n} working"),
        fr: t("{n} au travail"),
    }
    SessionsWaiting "sessions.waiting" {
        en: p("{n} waiting for you", "{n} waiting for you"),
        fr: p("{n} vous attend", "{n} vous attendent"),
    }
    SessionsIdle "sessions.idle" {
        en: p("{n} idle", "{n} idle"),
        fr: p("{n} inactive", "{n} inactives"),
    }
    SessionsRunning "sessions.running" {
        en: p("{n} running", "{n} running"),
        fr: p("{n} en cours", "{n} en cours"),
    }
    StatusbarSessions "statusbar.sessions" {
        en: t("Claude Code: {list}"),
        fr: t("Claude Code\u{a0}: {list}"),
    }
    SessionsStateWorking "sessions.state.working" {
        en: t("Working"),
        fr: t("Au travail"),
    }
    SessionsStateWaiting "sessions.state.waiting" {
        en: t("Waiting for you"),
        fr: t("Vous attend"),
    }
    SessionsStateIdle "sessions.state.idle" {
        en: t("Idle"),
        fr: t("Inactive"),
    }
    SessionsSince "sessions.since" {
        en: t("since {time}"),
        fr: t("depuis {time}"),
    }
    SessionsLast "sessions.last" {
        en: t("last: {activity}"),
        fr: t("dernière action\u{a0}: {activity}"),
    }
    SessionsLastTool "sessions.last_tool" {
        en: t("last tool: {tool}"),
        fr: t("dernier outil\u{a0}: {tool}"),
    }
    SessionsInActa "sessions.in_acta" {
        en: t("in Acta\u{2019}s terminal"),
        fr: t("dans le terminal d\u{2019}Acta"),
    }
    SessionsOutside "sessions.outside" {
        en: t("in another terminal"),
        fr: t("dans un autre terminal"),
    }
    SessionsEmpty "sessions.empty" {
        en: t("No Claude Code session is running."),
        fr: t("Aucune session Claude Code en cours."),
    }
    SessionsShowTerminal "sessions.show_terminal" {
        en: t("Show terminal"),
        fr: t("Afficher le terminal"),
    }
    SessionsEnd "sessions.end" {
        en: t("End session"),
        fr: t("Terminer la session"),
    }
    SessionsHereWorking "sessions.here.working" {
        en: t("Claude Code is working here"),
        fr: t("Claude Code travaille ici"),
    }
    SessionsHereWaiting "sessions.here.waiting" {
        en: t("Claude Code is waiting for you here"),
        fr: t("Claude Code vous attend ici"),
    }
    SessionsHereIdle "sessions.here.idle" {
        en: t("Claude Code is open here"),
        fr: t("Claude Code est ouvert ici"),
    }
    SessionsHereMany "sessions.here.many" {
        en: p("{n} Claude Code session here", "{n} Claude Code sessions here"),
        fr: p("{n} session Claude Code ici", "{n} sessions Claude Code ici"),
    }
    SettingsResume "settings.resume" {
        en: t("Continue the last conversation of each page"),
        fr: t("Reprendre la dernière conversation de chaque page"),
    }
    SettingsFlashWaiting "settings.flash_waiting" {
        en: t("Flash Acta in the taskbar when a session waits for you"),
        fr: t("Faire clignoter Acta dans la barre des tâches quand une session vous attend"),
    }
    SettingsFlashWaitingHint "settings.flash_waiting.hint" {
        en: t("Only while Acta is in the background."),
        fr: t("Seulement quand Acta est en arrière-plan."),
    }
    CmdResumeOn "cmd.resume_on" {
        en: t("Continue the last conversation of each page"),
        fr: t("Reprendre la dernière conversation de chaque page"),
    }
    CmdResumeOff "cmd.resume_off" {
        en: t("Start a new Claude Code conversation each time"),
        fr: t("Commencer une nouvelle conversation Claude Code à chaque fois"),
    }
    CmdFlashOn "cmd.flash_on" {
        en: t("Flash Acta in the taskbar when a session waits for you"),
        fr: t("Faire clignoter Acta dans la barre des tâches quand une session vous attend"),
    }
    CmdFlashOff "cmd.flash_off" {
        en: t("Do not flash Acta when a session waits"),
        fr: t("Ne pas faire clignoter Acta quand une session attend"),
    }
    UsageFiveHour "usage.five_hour" {
        en: t("5h {percent}"),
        fr: t("5 h {percent}"),
    }
    UsageSevenDay "usage.seven_day" {
        en: t("7d {percent}"),
        fr: t("7 j {percent}"),
    }
    UsageFiveHourTip "usage.five_hour.tip" {
        en: t("5-hour limit: {percent} used"),
        fr: t("Limite sur 5 heures\u{a0}: {percent} utilisés"),
    }
    UsageSevenDayTip "usage.seven_day.tip" {
        en: t("7-day limit: {percent} used"),
        fr: t("Limite sur 7 jours\u{a0}: {percent} utilisés"),
    }
    UsageResetsAt "usage.resets_at" {
        en: t("resets at {time}"),
        fr: t("réinitialisée à {time}"),
    }
    UsageResetsOn "usage.resets_on" {
        en: t("resets {day} {time}"),
        fr: t("réinitialisée {day} à {time}"),
    }
    UsageStale "usage.stale" {
        en: t("This window has reset since Claude Code last reported it."),
        fr: t("Cette fenêtre a été réinitialisée depuis le dernier relevé de Claude Code."),
    }
    UsageNoDataTip "usage.no_data.tip" {
        en: t("Claude Code reports your plan\u{2019}s limits after the first answer of a session (Pro, Max and Ultra plans), through Acta\u{2019}s status line."),
        fr: t("Claude Code signale les limites de votre forfait après la première réponse d\u{2019}une session (forfaits Pro, Max et Ultra), par la ligne d\u{2019}état d\u{2019}Acta."),
    }
    SettingsUsage "settings.usage" {
        en: t("Show plan usage in the status bar"),
        fr: t("Afficher l\u{2019}utilisation du forfait dans la barre d\u{2019}état"),
    }
    CmdUsageOn "cmd.usage_on" {
        en: t("Show plan usage in the status bar"),
        fr: t("Afficher l\u{2019}utilisation du forfait dans la barre d\u{2019}état"),
    }
    CmdUsageOff "cmd.usage_off" {
        en: t("Hide plan usage"),
        fr: t("Masquer l\u{2019}utilisation du forfait"),
    }
    SettingsSetupTitle "settings.setup.title" {
        en: t("Set up Claude Code?"),
        fr: t("Configurer Claude Code\u{202f}?"),
    }
    SettingsSetupBody "settings.setup.body" {
        en: t("Acta adds its plugin, two permissions and its status line to ~/.claude/settings.json. Your own status line still shows first."),
        fr: t("Acta ajoute son plugin, deux autorisations et sa ligne d\u{2019}état à ~/.claude/settings.json. Votre propre ligne d\u{2019}état reste affichée en premier."),
    }
    SettingsSetupFolders "settings.setup.folders" {
        en: p("It also turns Claude Code\u{2019}s own memory off in {n} folder of {project}.", "It also turns Claude Code\u{2019}s own memory off in {n} folders of {project}."),
        fr: p("Il désactive aussi la mémoire propre à Claude Code dans {n} dossier de {project}.", "Il désactive aussi la mémoire propre à Claude Code dans {n} dossiers de {project}."),
    }
    SettingsSetupConfirm "settings.setup.confirm" {
        en: t("Set up"),
        fr: t("Configurer"),
    }
    SettingsPluginHint "settings.plugin.hint" {
        en: t("Sessions started in Acta\u{2019}s terminal always load Acta\u{2019}s plugin. To stop that, set \"acta@inline\": false under enabledPlugins."),
        fr: t("Les sessions lancées dans le terminal d\u{2019}Acta chargent toujours le plugin d\u{2019}Acta. Pour l\u{2019}éviter, mettez \"acta@inline\": false sous enabledPlugins."),
    }
}
