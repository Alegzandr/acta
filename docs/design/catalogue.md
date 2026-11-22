# Acta design: copy catalogue

Part of the Acta design system. [DESIGN.md](../../DESIGN.md) is the index: direction, principles and the never-list; the review checklist is [checklist.md](checklist.md). Section numbers are shared across all parts.

## 10. Languages (continued)

### 10.4 Copy catalogue

The authoritative text of every string the design system defines. Sections 4 to 8 quote the English; code refers to the key. `{name}` is a placeholder; `{when}`, `{when_short}`, `{date}` and `{datetime}` are the time forms of 10.2, and the French long form brings its own preposition ("Modifié le 12 nov. 2026", "Modifié mardi à 14:05"). A cell with two forms gives "one · other", chosen by `{n}` unless the cell says otherwise. Keys ending in `.document` and `.memory` are the per-kind variants French agreement needs. Rows marked (W), (M), (L) apply to Windows, macOS, Linux.

| Key | English | French |
|---|---|---|
| `common.cancel` | Cancel | Annuler |
| `common.close` | Close | Fermer |
| `common.undo` | Undo | Annuler |
| `common.retry` | Retry | Réessayer |
| `common.try_again` | Try again | Réessayer |
| `common.edit` | Edit | Modifier |
| `common.delete` | Delete | Supprimer |
| `common.restore` | Restore | Restaurer |
| `common.dismiss` | Dismiss | Ignorer |
| `common.got_it` | Got it | Compris |
| `common.copied` | Copied | Copié |
| `common.save` | Save | Enregistrer |
| `common.open` | Open | Ouvrir |
| `common.move` | Move | Déplacer |
| `common.history` | History | Historique |
| `common.details` | Details | Détails |
| `common.none` | None | Aucun |
| `common.search` | Search | Rechercher |
| `common.clear` | Clear | Effacer |
| `common.opening` | Opening... | Ouverture... |
| `common.checking` | Checking... | Vérification... |
| `common.show_all` | Show all | Tout afficher |
| `common.show_all_n` | Show all {n} | Tout afficher ({n}) |
| `common.by` | {when} by {author} | {when} par {author} |
| `time.now` | just now | à l'instant |
| `time.minutes` | {n} min ago | il y a {n} min |
| `time.hours` | {n} h ago | il y a {n} h |
| `time.yesterday` | yesterday {time} | hier à {time} |
| `time.weekday` | {weekday} {time} | {weekday} à {time} (full weekday name) |
| `time.date` | {date} | le {date} |
| `time.datetime` | {date}, {time} | {date} à {time} |
| `time.today` | today, {time} | aujourd'hui à {time} |
| `time.today_word` | today | aujourd'hui |
| `time.days` | {n} day ago · {n} days ago | il y a {n} jour · il y a {n} jours |
| `time.short.yesterday` | Yesterday | Hier |
| `count.projects` | {n} project · {n} projects | {n} projet · {n} projets |
| `count.topics` | {n} topic · {n} topics | {n} sujet · {n} sujets |
| `count.notes` | {n} note · {n} notes | {n} note · {n} notes |
| `count.documents` | {n} document · {n} documents | {n} document · {n} documents |
| `count.memories` | {n} memory · {n} memories | {n} mémoire · {n} mémoires |
| `tree.inbox` | Inbox | Boîte de réception |
| `tree.knowledge` | Knowledge | Connaissances |
| `tree.trash` | Trash | Corbeille |
| `tree.settings` | Settings | Paramètres |
| `tree.closed` | {n} closed | {n} clos |
| `tree.no_epic` | No epic | Sans epic |
| `tree.rename` | Rename | Renommer |
| `tree.set_status` | Set status | Changer le statut |
| `tree.move_epic` | Move to epic | Déplacer vers un epic |
| `tree.unread` | New changes by Claude Code | Nouvelles modifications de Claude Code |
| `tree.empty` | No projects yet. | Aucun projet pour l'instant. |
| `tree.collapse_all` | Collapse all | Tout replier |
| `status.draft` | Draft | Brouillon |
| `status.planned` | Planned | Prévu |
| `status.active` | Active | En cours |
| `status.blocked` | Blocked | Bloqué |
| `status.done` | Done | Terminé |
| `status.dropped` | Dropped | Abandonné |
| `event.status` | {author} set status to {status} | {author} a passé le statut à « {status} » |
| `event.due_set` | {author} set the due date to {date} | {author} a fixé l'échéance au {date} |
| `event.due_cleared` | {author} cleared the due date | {author} a supprimé l'échéance |
| `event.report_set` | {author} noted to report to {name} | {author} a noté de rapporter à : {name} |
| `event.reported` | {author} marked it reported to {name} | {author} l'a marqué comme rapporté à : {name} |
| `event.report_unmarked` | {author} marked it not reported | {author} l'a marqué comme non rapporté |
| `event.report_cleared` | {author} removed Report to | {author} a retiré « À rapporter à » |
| `prop.status` | Status | Statut |
| `prop.epic` | Epic | Epic |
| `prop.folders` | Folders | Dossiers |
| `prop.progress` | Progress | Avancement |
| `prop.kind` | Kind | Nature |
| `prop.level` | Level | Niveau |
| `prop.due` | Due | Échéance |
| `prop.report_to` | Report to | À rapporter à |
| `prop.add_due` | Due date | Échéance |
| `prop.add_report_to` | Report to | À rapporter à |
| `meta.record` | {id} · Updated {when} by {author} | {id} · Modifié {when} par {author} |
| `meta.updated` | Updated {when} by {author} | Modifié {when} par {author} |
| `meta.copy_id` | Copy id | Copier l'identifiant |
| `due.none` | None | Aucune |
| `due.today` | Today | Aujourd'hui |
| `due.tomorrow` | Tomorrow | Demain |
| `due.in_days` | In {n} day · In {n} days | Dans {n} jour · Dans {n} jours |
| `due.overdue` | Overdue by {n} day · Overdue by {n} days | En retard de {n} jour · En retard de {n} jours |
| `due.tip.today` | Due today | Échéance aujourd'hui |
| `due.tip.tomorrow` | Due tomorrow | Échéance demain |
| `due.tip.in_days` | Due {date}, in {n} day · Due {date}, in {n} days | Échéance le {date}, dans {n} jour · Échéance le {date}, dans {n} jours |
| `due.tip.overdue` | Overdue by {n} day, due {date} · Overdue by {n} days, due {date} | En retard de {n} jour, échéance le {date} · En retard de {n} jours, échéance le {date} |
| `due.tip.date` | Due {date} | Échéance le {date} |
| `due.picker.placeholder` | fri, in 3d, 15/12, 2026-12-15 | ven., dans 3 j, 15/12, 2026-12-15 |
| `due.picker.invalid` | Not a date. Try fri, in 3d, 15/12 or 2026-12-15. | Date non reconnue. Essayez ven., dans 3 j, 15/12 ou 2026-12-15. |
| `due.picker.range` | Choose a date between 2000 and 2099. | Choisissez une date entre 2000 et 2099. |
| `due.picker.none` | No due date | Aucune échéance |
| `due.picker.prev` | Previous month | Mois précédent |
| `due.picker.next` | Next month | Mois suivant |
| `prop.progress.value` | {done} of {total} topic done · {done} of {total} topics done (form chosen by {total}) | {done} sujet terminé sur {total} · {done} sujets terminés sur {total} (form chosen by {done}) |
| `progress.none` | No topics yet | Aucun sujet |
| `kind.document` | Document | Document |
| `kind.memory` | Memory | Mémoire |
| `level.project` | Project {name} | Projet {name} |
| `level.epic` | Epic {name} | Epic {name} |
| `level.topic` | Topic {name} | Sujet {name} |
| `section.state` | Current state | État actuel |
| `section.description` | Description | Description |
| `section.questions` | Questions | Questions |
| `section.knowledge` | Knowledge | Connaissances |
| `overview.title` | Overview | Vue d'ensemble |
| `overview.clear` | Nothing needs attention. | Rien ne demande votre attention. |
| `overview.more` | More | Plus |
| `overview.less` | Less | Moins |
| `overview.attention` | Needs attention | À traiter |
| `overview.blocked` | {n} blocked topic · {n} blocked topics | {n} sujet bloqué · {n} sujets bloqués |
| `overview.overdue` | {n} overdue topic · {n} overdue topics | {n} sujet en retard · {n} sujets en retard |
| `overview.idle` | {n} active topic idle for 7 days · {n} active topics idle for 7 days | {n} sujet en cours inactif depuis 7 jours · {n} sujets en cours inactifs depuis 7 jours |
| `overview.inbox` | {n} note in the inbox for {days} days · {n} notes in the inbox, oldest {days} days | {n} note dans la boîte de réception depuis {days} jours · {n} notes dans la boîte de réception, la plus ancienne depuis {days} jours |
| `overview.week` | Last 7 days | 7 derniers jours |
| `overview.week.done` | {n} topic done · {n} topics done | {n} sujet terminé · {n} sujets terminés |
| `overview.week.decisions` | {n} decision · {n} decisions | {n} décision · {n} décisions |
| `overview.week.new` | {n} new topic · {n} new topics | {n} nouveau sujet · {n} nouveaux sujets |
| `overview.week.none` | Nothing done, decided or opened this week. Last activity {when}. | Rien de terminé, décidé ni ouvert cette semaine. Dernière activité {when}. |
| `overview.epics` | Epics | Epics |
| `overview.epics.more` | {n} more epic · {n} more epics | {n} autre epic · {n} autres epics |
| `overview.epics.complete` | {n} complete epic · {n} complete epics | {n} epic terminé · {n} epics terminés |
| `overview.memories` | {n} memory not updated for 90 days · {n} memories not updated for 90 days | {n} mémoire non mise à jour depuis 90 jours · {n} mémoires non mises à jour depuis 90 jours |
| `overview.questions_late` | {n} question past its answer date · {n} questions past their answer date | {n} question dont la date de réponse est passée · {n} questions dont la date de réponse est passée |
| `section.timeline` | Timeline | Journal |
| `state.updated` | Updated {when} by {author} | Modifié {when} par {author} |
| `state.placeholder` | Where does this stand? One or two sentences. | Où en est-on ? Une ou deux phrases. |
| `description.placeholder` | What is this topic about, and what does done look like? | De quoi s'agit-il, et à quoi verra-t-on que c'est terminé ? |
| `description.placeholder.epic` | What is this epic about? | De quoi parle cet epic ? |
| `description.placeholder.project` | What is this project about? | De quoi parle ce projet ? |
| `timeline.all` | All | Tout |
| `entry.progress` | Progress | Avancement |
| `entry.decision` | Decision | Décision |
| `entry.decisions` | Decisions | Décisions |
| `entry.discussion` | Discussion | Discussion |
| `composer.hint` | {keys} to save | {keys} pour enregistrer |
| `timeline.new` | {n} new entry · {n} new entries | {n} nouvelle entrée · {n} nouvelles entrées |
| `timeline.empty` | No entries yet. Claude Code logs progress here as it works. | Aucune entrée pour l'instant. Claude Code consigne ici l'avancement au fil du travail. |
| `entry.copy_markdown` | Copy as Markdown | Copier en Markdown |
| `entry.change_type` | Change type | Changer le type |
| `entry.edited` | edited | modifiée |
| `entry.edited_by` | edited by {name} | modifiée par {name} |
| `conflict.notice` | Claude Code changed this while you were editing. | Claude Code a modifié ceci pendant que vous écriviez. |
| `conflict.theirs` | Show theirs | Voir sa version |
| `conflict.mine` | Keep mine | Garder la mienne |
| `deleted.notice` | This topic was deleted by Claude Code. | Ce sujet a été supprimé par Claude Code. |
| `deleted.restore` | Restore | Restaurer |
| `topic.new` | New topic | Nouveau sujet |
| `topic.needs_title` | A topic needs a title. | Un sujet doit avoir un titre. |
| `topic.deleted` | Topic deleted. | Sujet supprimé. |
| `author.you` | You | Vous |
| `kb.deleted` | Deleted from knowledge. | Supprimé des connaissances. |
| `note.deleted` | Note deleted. | Note supprimée. |
| `entry.deleted` | Entry deleted. | Entrée supprimée. |
| `epic.deleted` | Epic deleted. | Epic supprimé. |
| `project.deleted` | Project deleted. | Projet supprimé. |
| `topic.restored` | Topic restored. | Sujet restauré. |
| `topic.empty_project` | Capture a note, or create a topic. | Saisissez une note ou créez un sujet. |
| `topic.empty.heading` | No topics yet | Aucun sujet pour l'instant |
| `error.save_entry` | Could not save the entry. The database is locked by another process. | Impossible d'enregistrer l'entrée. La base de données est verrouillée par un autre processus. |
| `error.save` | Could not save the change. Try again. | Impossible d'enregistrer la modification. Réessayez. |
| `undo.changed` | Not undone: it was changed since. | Non annulé : cela a été modifié depuis. |
| `redo.changed` | Not redone: it was changed since. | Non rétabli : cela a été modifié depuis. |
| `table.count` | {n} topic · {n} topics | {n} sujet · {n} sujets |
| `table.filter.open` | Open | Ouverts |
| `table.filter.all` | All | Tous |
| `table.filter.closed` | Closed | Clos |
| `table.title` | Title | Titre |
| `table.entries` | Entries | Entrées |
| `table.updated` | Updated | Modifié |
| `table.reset` | Reset order | Rétablir l'ordre |
| `table.due` | Due | Échéance |
| `table.questions` | Questions | Questions |
| `table.questions_tip` | {n} open question · {n} open questions | {n} question en suspens · {n} questions en suspens |
| `table.questions_tip_overdue` | 1 open question, overdue · {n} open questions, {late} overdue | 1 question en suspens, en retard · {n} questions en suspens, dont {late} en retard |
| `table.report_to` | Report to | À rapporter à |
| `table.filter.overdue` | Overdue | En retard |
| `table.filter.week` | Next 7 days | 7 prochains jours |
| `filter.blocked` | Blocked | Bloqués |
| `filter.idle` | No activity for 7 days | Sans activité depuis 7 jours |
| `filter.done_week` | Done in the last 7 days | Terminés ces 7 derniers jours |
| `filter.decided_week` | Decided in the last 7 days | Décidés ces 7 derniers jours |
| `filter.created_week` | Created in the last 7 days | Créés ces 7 derniers jours |
| `filter.stale_memories` | Not updated for 90 days | Non mises à jour depuis 90 jours |
| `filter.clear` | Clear filter | Effacer le filtre |
| `table.group_progress` | {done} of {total} done | {done} sur {total} terminé · {done} sur {total} terminés (form chosen by {done}) |
| `inbox.count` | {n} note · {n} notes | {n} note · {n} notes |
| `inbox.placeholder` | Note for {project} | Note pour {project} |
| `inbox.add` | Add | Ajouter |
| `inbox.filter.open` | Open | À traiter |
| `inbox.filter.filed` | Filed | Classées |
| `inbox.file` | File with Claude Code | Classer avec Claude Code |
| `inbox.filed_as` | Filed as | Classée dans |
| `inbox.dismissed` | Dismissed | Écartée |
| `inbox.by_claude` | by Claude Code | par Claude Code |
| `inbox.make_topic` | Make topic | Créer un sujet |
| `inbox.file_to` | File... | Classer... |
| `inbox.pick.new_epic` | New epic | Nouvel epic |
| `inbox.pick.in_epic` | New topic in {path} | Nouveau sujet dans {path} |
| `inbox.pick.as_entry` | Entry in {path} | Entrée dans {path} |
| `inbox.empty` | Inbox is empty. Captured notes wait here until they are filed. | La boîte de réception est vide. Les notes saisies attendent ici d'être classées. |
| `inbox.filed_empty` | No filed notes yet. | Aucune note classée pour l'instant. |
| `attach.button` | Attach files... | Joindre des fichiers... |
| `attach.drop` | Drop to attach | Déposer pour joindre |
| `attach.adding` | Adding... {percent} | Ajout... {percent} |
| `attach.pasted_name` | Pasted image {date} {time}.png | Image collée {date} {time}.png |
| `attach.too_large` | {name} is {size}; the limit is 25 MB per file. | {name} pèse {size} ; la limite est de 25 Mo par fichier. |
| `attach.too_many` | 20 files is the most a note, an entry or a description can hold. | Une note, une entrée ou une description contient 20 fichiers au plus. |
| `attach.read_failed` | Could not read {name}. It may have moved or be in use. | Impossible de lire {name}. Il a peut-être été déplacé, ou il est utilisé. |
| `attach.large_notice` | Files over 10 MB are also kept in every snapshot. | Les fichiers de plus de 10 Mo sont aussi conservés dans chaque instantané. |
| `attach.count` | {n} file · {n} files | {n} fichier · {n} fichiers |
| `attach.view` | View | Afficher |
| `attach.save_as` | Save as... | Enregistrer sous... |
| `attach.saved` | Saved to {folder}. | Enregistré dans {folder}. |
| `attach.show_in_folder` | Show in folder | Afficher dans le dossier |
| `attach.save_failed` | Could not save {name}. {reason} | Impossible d'enregistrer {name}. {reason} |
| `attach.remove` | Remove | Retirer |
| `attach.removed` | File removed. | Fichier retiré. |
| `attach.restored` | File restored. | Fichier restauré. |
| `attach.moved_to` | Moved to {id} | Déplacés vers {id} |
| `attach.position` | {i} of {n} | {i} sur {n} |
| `attach.prev` | Previous file | Fichier précédent |
| `attach.next` | Next file | Fichier suivant |
| `attach.fit` | Fit | Ajuster |
| `attach.actual` | Actual size | Taille réelle |
| `attach.truncated` | First 1 MB shown. Save the file to see all of it. | Premier Mo affiché. Enregistrez le fichier pour le voir en entier. |
| `attach.gone` | The file was removed while the editor was open; add it again. | Le fichier a été retiré pendant que l'éditeur était ouvert ; ajoutez-le de nouveau. |
| `voice.button` | Dictate | Dicter |
| `voice.button_setup` | Dictate... | Dicter... |
| `voice.button_downloading` | Dictation is downloading its model: {percent} | La dictée télécharge son modèle : {percent} |
| `voice.stop` | Stop dictating | Arrêter la dictée |
| `voice.starting` | Starting... | Démarrage... |
| `voice.listening` | Listening | À l'écoute |
| `voice.writing` | Writing text... | Écriture du texte... |
| `voice.stopped_silence` | Dictation stopped after a minute of silence. | Dictée arrêtée après une minute de silence. |
| `voice.stopped_limit` | Dictation stops after 10 minutes. Start again to go on. | La dictée s'arrête après 10 minutes. Recommencez pour continuer. |
| `voice.nothing_heard` | Nothing was heard. Check the microphone in Settings > Voice. | Rien n'a été entendu. Vérifiez le micro dans Paramètres > Voix. |
| `voice.settings_link` | Voice settings | Paramètres de la voix |
| `voice.slow` | Dictation is slow on this computer. A smaller model is faster. | La dictée est lente sur cet ordinateur. Un modèle plus petit est plus rapide. |
| `voice.error.blocked` | Acta is not allowed to use the microphone. | Acta n'est pas autorisé à utiliser le micro. |
| `voice.error.open_privacy` | Open privacy settings (W) | Ouvrir les paramètres de confidentialité (W) |
| `voice.error.open_system_settings` | Open System Settings (M) | Ouvrir Réglages Système (M) |
| `voice.error.no_device` | No microphone was found. | Aucun micro n'a été trouvé. |
| `voice.error.device_lost` | The microphone stopped. What was heard is kept. | Le micro s'est arrêté. Ce qui a été entendu est conservé. |
| `voice.error.model` | The dictation model is missing or damaged. | Le modèle de dictée est absent ou endommagé. |
| `voice.error.download_again` | Download again | Télécharger à nouveau |
| `voice.error.engine` | Dictation stopped unexpectedly. Your text is safe. | La dictée s'est arrêtée de façon inattendue. Votre texte est intact. |
| `voice.setup.title` | Dictate with your voice | Dicter avec votre voix |
| `voice.setup.body` | Acta turns what you say into text on this computer. Your voice never leaves it and is never saved. | Acta transforme ce que vous dites en texte sur cet ordinateur. Votre voix n'en sort jamais et n'est jamais enregistrée. |
| `voice.setup.cpu` | {size} download · works on this computer's processor | Téléchargement de {size} · fonctionne avec le processeur de cet ordinateur |
| `voice.setup.gpu` | {size} download · uses this computer's graphics | Téléchargement de {size} · utilise la carte graphique de cet ordinateur |
| `voice.setup.free` | Free on this disk: {free}. | Libre sur ce disque : {free}. |
| `voice.setup.no_space` | Not enough free space: {size} needed, {free} free. | Pas assez d'espace libre : {size} nécessaires, {free} libres. |
| `voice.setup.system` | Use the macOS recogniser instead: nothing to download (M) | Utiliser plutôt la reconnaissance de macOS : rien à télécharger (M) |
| `voice.setup.system_primary` | Use the macOS recogniser (M) | Utiliser la reconnaissance de macOS (M) |
| `voice.setup.unavailable` | Dictation needs at least 2 GB of memory; this computer has {memory}. | La dictée demande au moins 2 Go de mémoire ; cet ordinateur en a {memory}. |
| `voice.setup.unsupported_cpu` | Dictation needs a processor from 2013 or later; this one is older. | La dictée demande un processeur de 2013 ou plus récent ; celui-ci est plus ancien. |
| `voice.setup.more` | Other models are in Settings > Voice. | Les autres modèles sont dans Paramètres > Voix. |
| `voice.setup.download` | Download {size} | Télécharger {size} |
| `voice.download.progress_title` | Downloading Whisper {model}... | Téléchargement de Whisper {model}... |
| `voice.download.bytes` | {done} of {size} | {done} sur {size} |
| `voice.download.checking` | Checking... | Vérification... |
| `voice.download.ready` | Ready. Press {shortcut} in a text, or the microphone, to dictate. | Prêt. Appuyez sur {shortcut} dans un texte, ou sur le micro, pour dicter. |
| `voice.download.hide` | Hide | Masquer |
| `voice.download.cancel` | Cancel download | Annuler le téléchargement |
| `voice.download.failed` | The download stopped. {reason} | Le téléchargement s'est arrêté. {reason} |
| `voice.download.mismatch` | The file did not match. Try again later. | Le fichier ne correspond pas. Réessayez plus tard. |
| `voice.download.offline` | There is no connection. | Il n'y a pas de connexion. |
| `voice.settings.title` | Voice | Voix |
| `voice.settings.engine` | Dictation | Dictée |
| `voice.settings.engine_hint` | Dictation in notes, descriptions and documents. Your voice never leaves this computer. | Dictée dans les notes, les descriptions et les documents. Votre voix ne quitte jamais cet ordinateur. |
| `voice.engine.whisper` | Whisper, on this computer | Whisper, sur cet ordinateur |
| `voice.engine.macos` | macOS recogniser, on this computer (M) | Reconnaissance de macOS, sur cet ordinateur (M) |
| `voice.engine.off` | Off | Désactivée |
| `voice.settings.model` | Model | Modèle |
| `voice.settings.model_hint` | Recommended for this computer: {model}. Larger models are more accurate and slower. | Recommandé pour cet ordinateur : {model}. Les modèles plus grands sont plus précis et plus lents. |
| `voice.settings.model_hint_gpu` | This computer's graphics can run the large model quickly. | La carte graphique de cet ordinateur fait tourner le grand modèle rapidement. |
| `voice.settings.gpu` | Use the graphics card | Utiliser la carte graphique |
| `voice.settings.gpu_hint` | Faster with large models. Acta uses the processor when the graphics card cannot be used. | Plus rapide avec les grands modèles. Acta utilise le processeur quand la carte graphique n'est pas utilisable. |
| `voice.settings.language` | Language | Langue |
| `voice.settings.language_hint` | Pick one if dictation mistakes one language for the other. | Choisissez-en une si la dictée confond les deux langues. |
| `voice.language.auto` | English or French, detected | Anglais ou français, détecté |
| `voice.language.en` | English | Anglais |
| `voice.language.fr` | French | Français |
| `voice.settings.microphone` | Microphone | Micro |
| `voice.settings.microphone_default` | System default | Celui du système |
| `voice.settings.shortcut` | Shortcut | Raccourci |
| `voice.settings.shortcut_hint` | Press once to start and again to stop, or hold it while you speak. | Appuyez une fois pour commencer et une autre pour arrêter, ou maintenez-le pendant que vous parlez. |
| `voice.model.tiny_hint` | {size} · for older computers; rough in French | {size} · pour les ordinateurs anciens ; approximatif en français |
| `voice.model.base_hint` | {size} · fast; fair accuracy | {size} · rapide ; précision correcte |
| `voice.model.small_hint` | {size} · good for English and French | {size} · bon en anglais et en français |
| `voice.model.turbo_hint` | {size} · most accurate; best with a graphics card | {size} · le plus précis ; idéal avec une carte graphique |
| `voice.model.recommended` | Recommended | Recommandé |
| `voice.model.download` | Download | Télécharger |
| `voice.model.in_use` | In use | Utilisé |
| `voice.model.use` | Use | Utiliser |
| `voice.model.delete` | Delete model | Supprimer le modèle |
| `voice.model.delete_title` | Delete Whisper {model}? | Supprimer Whisper {model} ? |
| `voice.model.delete_body` | It will be downloaded again ({size}) if you choose it later. | Il sera téléchargé à nouveau ({size}) si vous le choisissez plus tard. |
| `voice.model.busy` | Stop dictating before deleting the model in use. | Arrêtez la dictée avant de supprimer le modèle utilisé. |
| `voice.models.where` | Models are kept in {path}. Free on this disk: {free}. | Les modèles sont conservés dans {path}. Libre sur ce disque : {free}. |
| `voice.models.used` | Downloaded models: {size}. | Modèles téléchargés : {size}. |
| `kb.new_document` | New document | Nouveau document |
| `kb.new_memory` | New memory | Nouvelle mémoire |
| `kb.written.document` | Written by {author} {when} | Écrit par {author} {when} |
| `kb.written.memory` | Written by {author} {when} | Écrite par {author} {when} |
| `kb.edited.document` | edited by {author} {when} | modifié par {author} {when} |
| `kb.edited.memory` | edited by {author} {when} | modifiée par {author} {when} |
| `kb.from_epic` | From epic {name} | De l'epic {name} |
| `kb.from_project` | From project {name} | Du projet {name} |
| `kb.more_inherited` | {n} more from epic and project | {n} autre hérité de l'epic et du projet · {n} autres hérités de l'epic et du projet |
| `kb.more_inherited.project` | {n} more from project {name} | {n} autre hérité du projet {name} · {n} autres hérités du projet {name} |
| `kb.more_epic` | {n} more in the topics of this epic | {n} de plus dans les sujets de cet epic |
| `kb.more_project` | {n} more in this project's epics and topics | {n} de plus dans les epics et sujets de ce projet |
| `kb.empty` | No documents or memories yet. | Aucun document ni mémoire pour l'instant. |
| `kb.summary.memory` | One line: the fact, in short | Une ligne : le fait, en bref |
| `kb.summary.document` | One line: what this is and when to read it | Une ligne : ce que c'est et quand le lire |
| `kb.show_history` | Show history | Afficher l'historique |
| `kb.move_epic` | Move to epic "{name}" | Déplacer vers l'epic « {name} » |
| `kb.move_project` | Move to project "{name}" | Déplacer vers le projet « {name} » |
| `kb.move_to` | Move to... | Déplacer vers... |
| `kb.change_kind` | Change kind | Changer de nature |
| `kb.save_file` | Save as file | Enregistrer comme fichier |
| `kb.saved_file` | Saved {name}. The file is not encrypted. | {name} enregistré. Le fichier n'est pas chiffré. |
| `kb.moved.project.document` | Moved to project {name}. | Déplacé vers le projet {name}. |
| `kb.moved.project.memory` | Moved to project {name}. | Déplacée vers le projet {name}. |
| `kb.moved.epic.document` | Moved to epic {name}. | Déplacé vers l'epic {name}. |
| `kb.moved.epic.memory` | Moved to epic {name}. | Déplacée vers l'epic {name}. |
| `kb.moved.topic.document` | Moved to topic {name}. | Déplacé vers le sujet {name}. |
| `kb.moved.topic.memory` | Moved to topic {name}. | Déplacée vers le sujet {name}. |
| `kb.scope` | {project} · {documents}, {memories} (the two counts are `count.documents` and `count.memories`) | {project} · {documents}, {memories} |
| `kb.search` | Search knowledge | Rechercher dans les connaissances |
| `kb.filter.level` | Level: {value} | Niveau : {value} |
| `kb.filter.kind` | Kind: {value} | Nature : {value} |
| `kb.filter.author` | Author: {value} | Auteur : {value} |
| `kb.value.all` | All | Tous |
| `kb.value.all.kind` | All (for the Kind filter) | Toutes |
| `kb.value.project` | Project | Projet |
| `kb.value.epics` | Epics | Epics |
| `kb.value.topics` | Topics | Sujets |
| `kb.value.documents` | Documents | Documents |
| `kb.value.memories` | Memories | Mémoires |
| `kb.results` | {n} result for "{query}" · {n} results for "{query}" | {n} résultat pour « {query} » · {n} résultats pour « {query} » |
| `kb.no_results` | Nothing in this project mentions "{query}". | Rien dans ce projet ne mentionne « {query} ». |
| `questions.new` | New question | Nouvelle question |
| `questions.closed` | {n} closed | {n} close · {n} closes |
| `questions.more_epic` | {n} more open in the topics of this epic | {n} autre en suspens dans les sujets de cet epic · {n} autres en suspens dans les sujets de cet epic |
| `questions.more_epic_overdue` | {n} more open in the topics of this epic, {late} overdue | {n} autre en suspens dans les sujets de cet epic, en retard · {n} autres en suspens dans les sujets de cet epic, dont {late} en retard |
| `questions.more_project` | {n} more open in this project's epics and topics | {n} autre en suspens dans les epics et sujets de ce projet · {n} autres en suspens dans les epics et sujets de ce projet |
| `questions.more_project_overdue` | {n} more open in this project's epics and topics, {late} overdue | {n} autre en suspens dans les epics et sujets de ce projet, en retard · {n} autres en suspens dans les epics et sujets de ce projet, dont {late} en retard |
| `questions.empty` | No questions yet. Note what you are waiting to hear. | Aucune question pour l'instant. Notez ce que vous attendez d'apprendre. |
| `questions.scope` | {scope} · {counts} | {scope} · {counts} |
| `count.questions_open` | {n} open | {n} en suspens |
| `count.questions_closed` | {n} closed | {n} close · {n} closes |
| `questions.whole_project` | Whole project | Tout le projet |
| `questions.none_open` | No open questions in {name}. | Aucune question en suspens dans {name}. |
| `questions.none_overdue` | No overdue questions. | Aucune question en retard. |
| `questions.none_closed` | No closed questions yet. | Aucune question close pour l'instant. |
| `question.state.open` | Open question | Question en suspens |
| `question.state.answered` | Answered | Répondue |
| `question.state.dropped` | Dropped | Abandonnée |
| `question.asked` | Asked {name} | Posée à : {name} |
| `question.by` | Answer by {date} | Réponse attendue le {date} |
| `question.answered_by` | Answer recorded {date} by {author} | Réponse notée le {date} par {author} |
| `question.dropped_by` | Dropped {date} by {author} | Abandonnée le {date} par {author} |
| `question.logged` | Logged as a decision | Consignée comme décision |
| `question.in` | in {level} | dans : {level} (`{level}` is a `level.*` string) |
| `question.placeholder` | What do you need to know? | Que faut-il savoir ? |
| `question.field.asked` | Asked | Posée à |
| `question.field.by` | Answer by | Réponse attendue |
| `question.mark_answered` | Mark answered | Marquer comme répondue |
| `question.answer_placeholder` | What was the answer? | Quelle a été la réponse ? |
| `question.log_decision` | Also log it as a decision | La consigner aussi comme décision |
| `question.save_answer` | Save answer | Enregistrer la réponse |
| `question.edit_answer` | Edit answer | Modifier la réponse |
| `question.set_date` | Set answer date... | Définir la date de réponse... |
| `question.clear_date` | Clear answer date | Effacer la date de réponse |
| `question.drop` | Drop question | Abandonner la question |
| `question.reopen` | Reopen | Rouvrir |
| `question.move` | Move to... | Déplacer vers... |
| `question.deleted` | Question deleted. | Question supprimée. |
| `question.restored` | Question restored. | Question restaurée. |
| `question.counter` | {n} / 300 | {n} / 300 |
| `person.none` | None | Personne |
| `count.person_asked` | {n} question to ask · {n} questions to ask | {n} question à poser · {n} questions à poser |
| `count.person_report` | {n} thing to report · {n} things to report | {n} élément à rapporter · {n} éléments à rapporter |
| `person.section.asked` | Questions asked | Questions posées |
| `person.section.report` | To report | À rapporter |
| `person.empty` | Nothing open with {name}. | Rien en cours avec : {name}. |
| `report.to` | Report to {name} | À rapporter à : {name} |
| `report.pending` | Not reported yet | Pas encore rapporté |
| `report.done` | Reported {date} | Rapporté le {date} |
| `report.mark` | Mark reported | Marquer comme rapporté |
| `report.unmark` | Mark not reported | Marquer comme non rapporté |
| `report.change` | Change person... | Changer de personne... |
| `report.reported_count` | {n} reported | {n} rapporté · {n} rapportés |
| `report.filter` | Report to: {name} | À rapporter à : {name} |
| `report.filter.all` | Report to: anyone | À rapporter à : tout le monde |
| `report.filter.anyone` | Anyone | Tout le monde |
| `contact.placeholder` | Search contacts | Rechercher un contact |
| `contact.new_named` | New contact "{name}" | Nouveau contact : {name} |
| `contact.new_in` | in {level} | dans : {level} |
| `contact.new_tag` | new | nouveau |
| `contact.out_of_scope` | {name} is no longer visible here. Choose again. | Ce contact n'est plus visible ici : {name}. Choisissez à nouveau. |
| `contact.level.all` | All projects | Tous les projets |
| `contact.level.current` | Current | Actuel |
| `contact.move.blocked` | Used outside | Utilisé ailleurs |
| `contact.move.refused` | Named outside {level}: {places}. Change those first. | Cité hors de ce niveau ({level}) : {places}. Modifiez-les d'abord. |
| `contact.move.place` | {id} {title} ({field}) | {id} {title} ({field}) |
| `contact.move.more` | +{n} more | +{n} autre · +{n} autres |
| `contact.lifted` | {name} is now a contact of all projects, so {record} still sees them. | Contact étendu à tous les projets pour rester visible depuis {record} : {name}. |
| `contact.lifted.project` | {name} is now a contact of project {project}, so {record} still sees them. | Contact étendu au projet {project} pour rester visible depuis {record} : {name}. |
| `contacts.title` | Contacts | Contacts |
| `contacts.count` | {n} contact · {n} contacts | {n} contact · {n} contacts |
| `contacts.filter.project` | This project | Ce projet |
| `contacts.filter.all` | All | Tous |
| `contacts.new` | New contact | Nouveau contact |
| `contacts.new_here` | New contact here | Nouveau contact ici |
| `contacts.empty` | No contacts yet. A contact is added when you choose who a question is asked of or who to report to. | Aucun contact pour l'instant. Un contact s'ajoute quand vous choisissez à qui poser une question ou à qui rapporter. |
| `contacts.no_match` | No contact matches "{query}". | Aucun contact ne correspond à « {query} ». |
| `contacts.none_here` | No contacts in {project}. | Aucun contact dans {project}. |
| `contact.name_placeholder` | Name | Nom |
| `contact.name_required` | A contact needs a name. | Un contact doit avoir un nom. |
| `contact.open_count` | {n} open | {n} en cours |
| `contact.prop.job_title` | Job title | Fonction |
| `contact.prop.company` | Company | Entreprise |
| `contact.prop.level` | Level | Niveau |
| `contact.add.job_title` | Job title | Fonction |
| `contact.add.company` | Company | Entreprise |
| `contact.add.address` | How to address them | Comment s'adresser à ce contact |
| `contact.section.address` | How to address them | Comment s'adresser à ce contact |
| `contact.address_placeholder` | Formal or first name, tone, what to avoid | Vouvoiement ou tutoiement, prénom ou nom, ton, ce qu'il faut éviter |
| `contact.address_counter` | {n} / 500 | {n} / 500 |
| `contact.meta` | {id} · Added {date} by {author} | {id} · Ajouté le {date} par {author} |
| `contact.meta_updated` | {id} · Added {date} by {author} · Updated {when} | {id} · Ajouté le {date} par {author} · Modifié {when} |
| `contact.scope` | {asked}, {report} | {asked}, {report} |
| `contact.scope_projects` | {asked}, {report} in {n} projects | {asked}, {report} dans {n} projets |
| `contact.duplicate` | Another contact is also named {name}, in {level}. | Un autre contact porte aussi ce nom, dans {level} : {name}. |
| `contact.merge_into_it` | Merge into it... | Fusionner avec lui... |
| `contact.keep_both` | Keep both | Garder les deux |
| `contact.go` | Go to {name} | Ouvrir : {name} |
| `contact.rename` | Rename | Renommer |
| `contact.move` | Move to... | Déplacer vers... |
| `contact.merge` | Merge into... | Fusionner avec... |
| `contact.delete` | Delete contact | Supprimer le contact |
| `contact.merged` | Merged into {name}. | Fusionné avec : {name}. |
| `contact.restored_merged` | Contact restored. Its uses stayed with {name}. | Contact restauré. Ses usages restent à : {name}. |
| `contact.deleted` | Contact deleted. | Contact supprimé. |
| `contact.restored` | Contact restored. | Contact restauré. |
| `contact.in_trash` | In the trash | Dans la corbeille |
| `doc.editing` | Editing | Modification en cours |
| `doc.needs` | A document needs a title and a summary. | Un document doit avoir un titre et un résumé. |
| `doc.go_heading` | Go to heading | Aller au titre |
| `doc.section_lines` | {n} line · {n} lines | {n} ligne · {n} lignes |
| `history.version` | Version from {datetime} by {author} | Version du {datetime} par {author} |
| `history.text` | Text | Texte |
| `history.changes` | Changes | Modifications |
| `history.restore` | Restore this version | Restaurer cette version |
| `history.restored` | Restored the version from {datetime}. | Version du {datetime} restaurée. |
| `history.replaced` | Replaced section {name} | Section {name} remplacée |
| `history.edited.document` | Edited | Modifié |
| `history.edited.memory` | Edited | Modifiée |
| `history.from_file.document` | Created from file | Créé depuis un fichier |
| `history.from_file.memory` | Created from file | Créée depuis un fichier |
| `history.created.document` | Created | Créé |
| `history.created.memory` | Created | Créée |
| `folders.title` | Working folders | Dossiers de travail |
| `folders.from_epic` | from epic {name} | de l'epic {name} |
| `folders.from_project` | from project {name} | du projet {name} |
| `folders.add` | Add folder... | Ajouter un dossier... |
| `folders.hint` | Claude Code starts in the active folder and can also use the others. | Claude Code démarre dans le dossier actif et peut aussi utiliser les autres. |
| `folders.none` | No working folder. The terminal will ask where to start. | Aucun dossier de travail. Le terminal demandera où démarrer. |
| `folders.set` | Set working folder... | Définir le dossier de travail... |
| `folders.not_found` | Not found on this computer | Introuvable sur cet ordinateur |
| `folders.locate` | Locate... | Localiser... |
| `folders.n_not_found` | {n} not found | {n} introuvable · {n} introuvables |
| `folders.also_topic` | Also attached to topic {name}. | Également rattaché au sujet {name}. |
| `folders.also_epic` | Also attached to epic {name}. | Également rattaché à l'epic {name}. |
| `folders.also_more` | Also attached to topics {a}, {b} and {n} more. | Également rattaché aux sujets {a}, {b} et {n} autre. · Également rattaché aux sujets {a}, {b} et {n} autres. |
| `folders.also.tip` | A session started in this folder outside Acta opens on {level} and is told about both. | Une session lancée dans ce dossier hors d'Acta s'ouvre sur : {level}, et est informée des deux. |
| `folders.show.w` | Show in Explorer (W) | Afficher dans l'Explorateur |
| `folders.show.m` | Reveal in Finder (M) | Afficher dans le Finder |
| `folders.show.l` | Show in file manager (L) | Afficher dans le gestionnaire de fichiers |
| `folders.copy_path` | Copy path | Copier le chemin |
| `folders.move_up` | Move up | Monter |
| `folders.move_down` | Move down | Descendre |
| `folders.choose_other` | Choose another folder... | Choisir un autre dossier... |
| `folders.remove.topic` | Remove from this topic | Retirer de ce sujet |
| `folders.remove.epic` | Remove from this epic | Retirer de cet epic |
| `folders.remove.project` | Remove from this project | Retirer de ce projet |
| `folders.edit_in` | Edit in epic {name} | Modifier dans l'epic {name} |
| `folders.removed.topic` | {folder} removed from this topic. | {folder} retiré de ce sujet. |
| `folders.removed.epic` | {folder} removed from this epic. | {folder} retiré de cet epic. |
| `folders.removed.project` | {folder} removed from this project. | {folder} retiré de ce projet. |
| `folders.located_for` | {folder} located for epic {name} and everything under it. | {folder} localisé pour l'epic {name} et tout ce qu'il contient. |
| `folders.bulk` | {n} other folder under {root} was not found either. Apply the same change to it? · {n} other folders under {root} were not found either. Apply the same change to them? | {n} autre dossier sous {root} est également introuvable. Lui appliquer le même changement ? · {n} autres dossiers sous {root} sont également introuvables. Leur appliquer le même changement ? |
| `folders.bulk_apply` | Apply to {n} folder · Apply to {n} folders | Appliquer à {n} dossier · Appliquer à {n} dossiers |
| `folders.bulk_done` | {n} folder located. · {n} folders located. | {n} dossier localisé. · {n} dossiers localisés. |
| `folders.picker` | Choose a working folder | Choisir un dossier de travail |
| `folders.active` | Active | Actif |
| `folders.set_active` | Set as active here | Définir comme actif ici |
| `folders.edit` | Edit folders... | Modifier les dossiers... |
| `folders.settings` | Folder settings... | Réglages du dossier... |
| `folders.group.topic` | This topic | Ce sujet |
| `folders.group.epic` | This epic | Cet epic |
| `folders.group.project` | This project | Ce projet |
| `folders.group.from_epic` | From epic {name} | De l'epic {name} |
| `folders.group.from_project` | From project {name} | Du projet {name} |
| `folders.no_own` | No folders of its own. | Aucun dossier propre. |
| `folders.not_found_short` | Not found | Introuvable |
| `folders.other_checkout` | Another checkout of {name}'s repository. Not passed to Claude Code here. | Autre copie du dépôt de {name}. Non transmise à Claude Code ici. |
| `folders.git.detached` | detached at {commit} | détaché sur {commit} |
| `folders.git.changed` | {n} changed | {n} modifié · {n} modifiés |
| `folders.git.auto_commit` | Claude Code commits automatically here | Claude Code commite automatiquement ici |
| `folders.git.auto_push` | Claude Code commits and pushes automatically here | Claude Code commite et pousse automatiquement ici |
| `folders.name` | Name | Nom |
| `folders.icon` | Icon | Icône |
| `folders.icon.auto` | Automatic | Automatique |
| `folders.icon.auto_hint` | Chosen from the folder: worktree, repository or plain folder | Choisie d'après le dossier : worktree, dépôt ou dossier simple |
| `folders.icon.folder` | Folder | Dossier |
| `folders.icon.repo` | Git repository | Dépôt git |
| `folders.icon.worktree` | Git worktree | Worktree git |
| `folders.icon.code` | Code | Code |
| `folders.icon.docs` | Documentation | Documentation |
| `folders.icon.package` | Package | Paquet |
| `folders.icon.server` | Server | Serveur |
| `folders.icon.web` | Web | Web |
| `folders.icon.mobile` | Mobile | Mobile |
| `folders.icon.data` | Data | Données |
| `folders.icon.lab` | Experiments | Expériences |
| `folders.icon.design` | Design | Design |
| `folders.commits` | Commits by Claude Code | Commits par Claude Code |
| `folders.commit.manual` | Manual | Manuel |
| `folders.commit.commit` | Commit | Commit |
| `folders.commit.push` | Commit and push | Commit et push |
| `folders.commit.manual_hint` | Claude Code commits only when asked. | Claude Code ne commite que sur demande. |
| `folders.commit.commit_hint` | Claude Code commits each finished, tested change. | Claude Code commite chaque modification terminée et testée. |
| `folders.commit.push_hint` | Then pushes to the upstream branch, never forced. It stops if a push fails. | Puis pousse vers la branche amont, jamais en force. Il s'arrête si un push échoue. |
| `folders.commit.as_user` | Commit in my name, without Co-Authored-By Claude | Commiter en mon nom, sans Co-Authored-By Claude |
| `folders.commit.on_default` | Also on {branch} | Aussi sur {branch} |
| `folders.commit.identity` | Commits as {name}. | Commits au nom de : {name}. |
| `folders.commit.no_identity` | Git has no user name here, so commits will fail. | Git n'a pas de nom d'utilisateur ici : les commits échoueront. |
| `folders.commit.file` | Saved in .claude/settings.local.json, kept out of git. | Enregistré dans .claude/settings.local.json, exclu de git. |
| `folders.commit.tracked` | .claude/settings.local.json is tracked by git here, so Acta leaves it alone. Claude Code will ask before each commit. | .claude/settings.local.json est suivi par git ici, Acta n'y touche donc pas. Claude Code demandera avant chaque commit. |
| `folders.commit.user_key` | Your own {key} setting is kept. | Votre propre réglage {key} est conservé. |
| `folders.commit.root_wins` | The repository's own attribution setting wins. | Le réglage attribution du dépôt l'emporte. |
| `folders.git.behind` | behind {n} | {n} en retard |
| `folders.git.pull` | Pull | Pull |
| `folders.git.fetch` | Fetch | Fetch |
| `folders.git.branch_tip` | Switch branch | Changer de branche |
| `folders.git.local` | Branches | Branches |
| `folders.git.remote` | Remote branches | Branches distantes |
| `folders.git.track` | track | suivre |
| `folders.git.new_branch` | New branch... | Nouvelle branche... |
| `folders.git.new_branch_hint` | Name of the new branch, from the current commit | Nom de la nouvelle branche, depuis le commit actuel |
| `folders.git.create` | Create | Créer |
| `folders.git.pulled` | Pulled {n} commit into {branch}. · Pulled {n} commits into {branch}. | {n} commit récupéré dans {branch}. · {n} commits récupérés dans {branch}. |
| `folders.git.up_to_date` | {branch} is up to date. | {branch} est à jour. |
| `folders.git.err.no_git` | Git is not installed on this computer. | Git n'est pas installé sur cet ordinateur. |
| `folders.git.err.timed_out` | Git took too long, so Acta stopped it. Nothing was changed. | Git a mis trop de temps : Acta l'a arrêté. Rien n'a été modifié. |
| `folders.git.err.no_upstream` | {branch} has no upstream branch to pull from. Nothing was changed. | {branch} n'a pas de branche amont d'où récupérer. Rien n'a été modifié. |
| `folders.git.err.detached` | There is no current branch (detached HEAD). Nothing was changed. | Il n'y a pas de branche courante (HEAD détachée). Rien n'a été modifié. |
| `folders.git.err.diverged` | {branch} and its upstream have diverged, so it cannot be fast-forwarded. Nothing was changed. | {branch} et sa branche amont ont divergé : l'avance rapide est impossible. Rien n'a été modifié. |
| `folders.git.err.local_changes` | Local changes in {folder} would be overwritten. Nothing was changed: commit them or set them aside first. | Des modifications locales de {folder} seraient écrasées. Rien n'a été modifié : commitez-les ou mettez-les de côté d'abord. |
| `folders.git.err.sign_in` | The remote asks for a sign-in, and Acta never asks for one. Nothing was changed. | Le dépôt distant demande une connexion, et Acta n'en demande jamais. Rien n'a été modifié. |
| `folders.git.err.network` | The remote could not be reached. Nothing was changed. | Le dépôt distant est injoignable. Rien n'a été modifié. |
| `folders.git.err.exists` | A branch named {branch} already exists. Nothing was changed. | Une branche nommée {branch} existe déjà. Rien n'a été modifié. |
| `folders.git.err.invalid_name` | {branch} is not a valid branch name. Nothing was changed. | {branch} n'est pas un nom de branche valide. Rien n'a été modifié. |
| `folders.git.err.no_branch` | There is no branch {branch}. Nothing was changed. | Il n'y a pas de branche {branch}. Rien n'a été modifié. |
| `folders.git.err.elsewhere` | {branch} is checked out in another worktree. Nothing was changed. | {branch} est extraite dans un autre worktree. Rien n'a été modifié. |
| `folders.git.err.other` | Git refused, and nothing was changed: {message} | Git a refusé, rien n'a été modifié : {message} |
| `folders.git.busy.title` | Claude Code is working in {folder} | Claude Code travaille dans {folder} |
| `folders.git.busy.switch` | Switching to {branch} changes the files under it while it works. It is not told. | Passer sur {branch} change ses fichiers pendant qu'il travaille. Il n'en est pas informé. |
| `folders.git.busy.pull` | Pulling changes the files under it while it works. It is not told. | Le pull change ses fichiers pendant qu'il travaille. Il n'en est pas informé. |
| `folders.git.busy.confirm_switch` | Switch anyway | Changer quand même |
| `folders.git.busy.confirm_pull` | Pull anyway | Pull quand même |
| `terminal.title` | Terminal | Terminal |
| `terminal.on_topic` | {id} {name} | {id} {name} |
| `terminal.on_epic` | {id} {name} | {id} {name} |
| `terminal.on_project` | project {name} | projet {name} |
| `terminal.restart_here` | Restart here | Redémarrer ici |
| `terminal.ended` | Session ended. {keys} to restart. | Session terminée. {keys} pour relancer. |
| `terminal.ended_once` | Session ended. | Session terminée. |
| `terminal.ask.title` | Where should Claude Code start? | Où Claude Code doit-il démarrer ? |
| `terminal.ask.topic` | {name} has no working folder, and neither has its epic or its project. | {name} n'a pas de dossier de travail, son epic et son projet non plus. |
| `terminal.ask.epic` | {name} has no working folder, and neither has its project. | {name} n'a pas de dossier de travail, son projet non plus. |
| `terminal.ask.project` | {name} has no working folder yet. | {name} n'a pas encore de dossier de travail. |
| `terminal.ask.save` | Save as the working folder of project {name} | Enregistrer comme dossier de travail du projet {name} |
| `terminal.ask.unsaved` | If not saved, it is used until Acta quits. | S'il n'est pas enregistré, il est utilisé jusqu'à la fermeture d'Acta. |
| `terminal.ask.choose` | Choose a folder... | Choisir un dossier... |
| `terminal.missing.title` | The working folder was not found | Le dossier de travail est introuvable |
| `terminal.missing.locate` | Locate folder... | Localiser le dossier... |
| `terminal.left_out` | {folder} was not found and was left out of this session. | {folder} est introuvable et n'a pas été inclus dans cette session. |
| `terminal.located` | {folder} located. Restart the terminal to include it. | {folder} localisé. Redémarrez le terminal pour l'inclure. |
| `terminal.restart.title` | Restart the terminal? | Redémarrer le terminal ? |
| `terminal.restart.body` | Claude Code is still running in it. | Claude Code y est encore en cours d'exécution. |
| `terminal.restart.confirm` | Restart | Redémarrer |
| `terminal.missing.body.topic` | Claude Code was not started: the active folder of topic {name} is not on this computer. | Claude Code n'a pas été lancé : le dossier actif du sujet {name} n'est pas sur cet ordinateur. |
| `terminal.missing.body.epic` | Claude Code was not started: the active folder of epic {name} is not on this computer. | Claude Code n'a pas été lancé : le dossier actif de l'epic {name} n'est pas sur cet ordinateur. |
| `terminal.missing.body.project` | Claude Code was not started: the active folder of project {name} is not on this computer. | Claude Code n'a pas été lancé : le dossier actif du projet {name} n'est pas sur cet ordinateur. |
| `terminal.missing.other` | Or start in another folder: | Ou démarrer dans un autre dossier : |
| `terminal.running_in` | (running in {name}) | (lancé dans {name}) |
| `terminal.restart_in` | Restart in {name} | Redémarrer dans {name} |
| `terminal.close.title` | Close the terminal? | Fermer le terminal ? |
| `terminal.close.body` | Claude Code is still running in it. | Claude Code y est encore en cours d'exécution. |
| `terminal.close.confirm` | Close terminal | Fermer le terminal |
| `quit.title` | Quit Acta? | Quitter Acta ? |
| `quit.body` | Claude Code is still running in the terminal of {project}. | Claude Code est encore en cours d'exécution dans le terminal de {project}. |
| `quit.installing` | {name} is still installing. It goes on after Acta quits, but its log is lost. | {name} est encore en cours d'installation. Elle continue après la fermeture d'Acta, mais son journal est perdu. |
| `quit.confirm` | Quit | Quitter |
| `chat.title` | Chat | Chat |
| `chat.open_browser` | Open in browser | Ouvrir dans le navigateur |
| `chat.reload` | Reload | Recharger |
| `chat.help` | Sign-in help | Aide à la connexion |
| `chat.start.title` | Chat with Claude | Discuter avec Claude |
| `chat.start.body` | claude.ai in a panel, signed in with your own account. Acta never reads the page. | claude.ai dans un panneau, avec votre propre compte. Acta ne lit jamais la page. |
| `chat.start.memory` | Uses about 400 MB of memory once opened. | Utilise environ 400 Mo de mémoire une fois ouvert. |
| `chat.start.here` | Open claude.ai here | Ouvrir claude.ai ici |
| `chat.start.browser` | Open in your browser | Ouvrir dans votre navigateur |
| `chat.starting` | Opening claude.ai... | Ouverture de claude.ai... |
| `chat.signin` | Sign in by email: open the link, then type the code it shows here. Google or Apple sign-in may be refused. | Connectez-vous par e-mail : ouvrez le lien, puis saisissez ici le code affiché. La connexion Google ou Apple peut être refusée. |
| `chat.hidden` | The chat is hidden while this is open. | Le chat est masqué tant que ceci est ouvert. |
| `chat.runtime.title` | The chat panel needs WebView2 | Le panneau de chat a besoin de WebView2 |
| `chat.runtime.body` | WebView2 is not installed, so claude.ai cannot show here. Everything else works. | WebView2 n'est pas installé : claude.ai ne peut pas s'afficher ici. Tout le reste fonctionne. |
| `chat.runtime.get` | Get WebView2 | Obtenir WebView2 |
| `chat.browser` | Open claude.ai in your browser | Ouvrir claude.ai dans votre navigateur |
| `chat.failed.title` | claude.ai could not be opened here | claude.ai n'a pas pu être ouvert ici |
| `chat.failed.offline` | There is no connection. | Il n'y a pas de connexion. |
| `chat.signout` | Sign out and clear chat data... | Se déconnecter et effacer les données du chat... |
| `chat.signout.title` | Sign out and clear chat data? | Se déconnecter et effacer les données du chat ? |
| `chat.signout.body` | Signs you out of claude.ai here. Your conversations stay in your Claude account. | Vous déconnecte de claude.ai ici. Vos conversations restent dans votre compte Claude. |
| `chat.signout.confirm` | Sign out and clear | Se déconnecter et effacer |
| `chat.more` | More | Plus |
| `chat.clear_failed` | The chat data could not be deleted. Acta tries again at the next start. | Les données du chat n'ont pas pu être supprimées. Acta réessaiera au prochain démarrage. |
| `tab.open_new` | Open in new tab | Ouvrir dans un nouvel onglet |
| `tab.close` | Close tab | Fermer l'onglet |
| `tab.close_others` | Close other tabs | Fermer les autres onglets |
| `tab.reopen` | Reopen closed tab | Rouvrir l'onglet fermé |
| `view.projects` | Projects | Projets |
| `area.left` | Left sidebar | Barre latérale gauche |
| `area.right` | Right sidebar | Barre latérale droite |
| `area.bottom` | Bottom panel | Panneau du bas |
| `area.page` | Page area | Zone des pages |
| `area.hide.left` | Hide left sidebar | Masquer la barre latérale gauche |
| `area.hide.right` | Hide right sidebar | Masquer la barre latérale droite |
| `area.hide.bottom` | Hide bottom panel | Masquer le panneau du bas |
| `area.empty` | Drag a view here, or move one from the palette. | Faites glisser une vue ici, ou déplacez-en une depuis la palette. |
| `view.actions` | View actions | Actions de la vue |
| `view.move_left` | Move to left sidebar | Déplacer dans la barre latérale gauche |
| `view.move_right` | Move to right sidebar | Déplacer dans la barre latérale droite |
| `view.move_bottom` | Move to bottom panel | Déplacer dans le panneau du bas |
| `view.move_page` | Move to page area | Déplacer dans la zone des pages |
| `view.split_off` | Split from group | Séparer du groupe |
| `view.hide` | Hide | Masquer |
| `layout.menu` | Layout | Disposition |
| `layout.group` | Group {n} | Groupe {n} |
| `layout.webview_hidden` | Hidden while you rearrange the layout. | Masqué pendant la réorganisation de la disposition. |
| `layout.moved.left` | View moved to the left sidebar: {view} | Vue déplacée dans la barre latérale gauche : {view} |
| `layout.moved.right` | View moved to the right sidebar: {view} | Vue déplacée dans la barre latérale droite : {view} |
| `layout.moved.bottom` | View moved to the bottom panel: {view} | Vue déplacée dans le panneau du bas : {view} |
| `layout.moved.page` | View moved to the page area: {view} | Vue déplacée dans la zone des pages : {view} |
| `layout.hidden` | View hidden: {view} | Vue masquée : {view} |
| `layout.reset_done` | Layout reset. | Disposition réinitialisée. |
| `layout.no_room` | Not enough room: {reason} | Pas assez de place : {reason} |
| `layout.no_room.left` | the window is too narrow for both sidebars. | la fenêtre est trop étroite pour les deux barres latérales. |
| `layout.no_room.hide_right` | Hide the right sidebar and move | Masquer la barre latérale droite et déplacer |
| `palette.recent` | Recent | Récents |
| `palette.topics` | Topics | Sujets |
| `palette.commands` | Commands | Commandes |
| `palette.projects` | Projects | Projets |
| `palette.epics` | Epics | Epics |
| `palette.hint.open` | {keys} open | {keys} ouvrir |
| `palette.hint.tab` | {keys} open in new tab | {keys} ouvrir dans un nouvel onglet |
| `palette.hint.create` | {keys} create | {keys} créer |
| `palette.hint.choose` | {keys} choose | {keys} choisir |
| `palette.hint.close` | {keys} close | {keys} fermer |
| `palette.none` | Nothing matches "{query}". {keys} creates a topic with this title. | Aucun résultat pour « {query} ». {keys} crée un sujet avec ce titre. |
| `palette.no_choice` | Nothing matches "{query}". | Aucun résultat pour « {query} ». |
| `cmd.go_topic` | Go to topic... | Aller au sujet... |
| `cmd.capture` | Capture a note | Saisir une note |
| `cmd.new_topic` | New topic | Nouveau sujet |
| `cmd.go_inbox` | Go to inbox | Aller à la boîte de réception |
| `cmd.show_blocked` | Show blocked topics | Afficher les sujets bloqués |
| `cmd.show_idle` | Show topics with no activity | Afficher les sujets sans activité |
| `cmd.show_done_week` | Show topics done in the last 7 days | Afficher les sujets terminés ces 7 derniers jours |
| `cmd.show_decided_week` | Show topics with a decision in the last 7 days | Afficher les sujets avec une décision ces 7 derniers jours |
| `cmd.show_created_week` | Show topics created in the last 7 days | Afficher les sujets créés ces 7 derniers jours |
| `cmd.show_stale_memories` | Show memories not updated for 90 days | Afficher les mémoires non mises à jour depuis 90 jours |
| `cmd.show_questions_late` | Show questions past their answer date | Afficher les questions dont la date de réponse est passée |
| `cmd.overview_more` | Overview: Show more | Vue d'ensemble : afficher plus |
| `cmd.overview_less` | Overview: Show less | Vue d'ensemble : afficher moins |
| `cmd.back` | Back | Précédent |
| `cmd.forward` | Forward | Suivant |
| `cmd.undo` | Undo | Annuler |
| `cmd.redo` | Redo | Rétablir |
| `cmd.zoom` | Maximize focused area | Agrandir au maximum la zone active |
| `cmd.set_status` | Set status: {status} | Définir le statut : {status} |
| `cmd.due_set` | Set due date... | Définir l'échéance... |
| `cmd.due_clear` | Clear due date | Effacer l'échéance |
| `cmd.question_new` | New question | Nouvelle question |
| `cmd.questions_open` | Go to questions | Aller aux questions |
| `cmd.report_set` | Set Report to... | Définir « À rapporter à »... |
| `cmd.report_clear` | Clear Report to | Effacer « À rapporter à » |
| `cmd.reported` | Mark reported | Marquer comme rapporté |
| `cmd.unreported` | Mark not reported | Marquer comme non rapporté |
| `cmd.question_answer` | Mark question answered | Marquer la question comme répondue |
| `cmd.question_edit` | Edit question | Modifier la question |
| `cmd.question_date` | Set answer date... | Définir la date de réponse... |
| `cmd.question_date_clear` | Clear answer date | Effacer la date de réponse |
| `cmd.question_drop` | Drop question | Abandonner la question |
| `cmd.question_reopen` | Reopen question | Rouvrir la question |
| `cmd.question_move` | Move question to... | Déplacer la question vers... |
| `cmd.question_delete` | Delete question | Supprimer la question |
| `cmd.contacts_open` | Go to contacts | Aller aux contacts |
| `cmd.contact_go` | Go to contact... | Aller à un contact... |
| `cmd.contact_new` | New contact | Nouveau contact |
| `cmd.contact_rename` | Rename contact | Renommer le contact |
| `cmd.contact_job_title` | Set job title... | Définir la fonction... |
| `cmd.contact_company` | Set company... | Définir l'entreprise... |
| `cmd.contact_address` | Edit how to address them | Modifier comment s'adresser à ce contact |
| `cmd.contact_move` | Move contact to... | Déplacer le contact vers... |
| `cmd.contact_merge` | Merge contact into... | Fusionner le contact avec... |
| `cmd.contact_delete` | Delete contact | Supprimer le contact |
| `cmd.profile_name` | Set your name | Définir votre nom |
| `cmd.profile_job_title` | Set your job title | Définir votre fonction |
| `cmd.settings` | Open settings | Ouvrir les paramètres |
| `cmd.theme` | Theme: {value} | Thème : {value} |
| `cmd.text_size` | Text size: {value} | Taille du texte : {value} |
| `cmd.reading_width` | Reading width: {value} | Largeur de lecture : {value} |
| `cmd.language` | Language: {value} | Langue : {value} |
| `cmd.new_project` | New project... | Nouveau projet... |
| `cmd.tree_collapse_all` | Projects: Collapse all | Projets : tout replier |
| `cmd.new_epic` | New epic... | Nouvel epic... |
| `cmd.switch_project` | Switch project... | Changer de projet... |
| `cmd.toggle_sidebar` | Toggle left sidebar | Afficher ou masquer la barre latérale gauche |
| `cmd.toggle_terminal` | Toggle terminal | Afficher ou masquer le terminal |
| `cmd.toggle_chat` | Toggle chat | Afficher ou masquer le chat |
| `cmd.toggle_right` | Toggle right sidebar | Afficher ou masquer la barre latérale droite |
| `cmd.toggle_bottom` | Toggle bottom panel | Afficher ou masquer le panneau du bas |
| `cmd.restart_terminal` | Restart terminal here | Redémarrer le terminal ici |
| `cmd.view_show` | {view}: Show | {view} : afficher |
| `cmd.view_hide` | {view}: Hide | {view} : masquer |
| `cmd.view_left` | {view}: Move to left sidebar | {view} : déplacer dans la barre latérale gauche |
| `cmd.view_right` | {view}: Move to right sidebar | {view} : déplacer dans la barre latérale droite |
| `cmd.view_bottom` | {view}: Move to bottom panel | {view} : déplacer dans le panneau du bas |
| `cmd.view_page` | {view}: Move to page area | {view} : déplacer dans la zone des pages |
| `cmd.view_split_off` | {view}: Split from group | {view} : séparer du groupe |
| `cmd.split_right` | Split right | Diviser vers la droite |
| `cmd.split_down` | Split down | Diviser vers le bas |
| `cmd.tab_next_group` | Move tab to next group | Déplacer l'onglet dans le groupe suivant |
| `cmd.tab_prev_group` | Move tab to previous group | Déplacer l'onglet dans le groupe précédent |
| `cmd.join_groups` | Join groups | Réunir les groupes |
| `cmd.next_area` | Focus next area | Aller à la zone suivante |
| `cmd.prev_area` | Focus previous area | Aller à la zone précédente |
| `cmd.grow_area` | Grow focused area | Agrandir la zone active |
| `cmd.shrink_area` | Shrink focused area | Réduire la zone active |
| `cmd.layout_undo` | Undo layout change | Annuler le changement de disposition |
| `cmd.layout_reset` | Reset layout | Réinitialiser la disposition |
| `cmd.setup` | Set up Claude Code | Configurer Claude Code |
| `cmd.restart_update` | Restart to update | Redémarrer pour mettre à jour |
| `cmd.terminal_larger` | Increase terminal text size | Augmenter la taille du texte du terminal |
| `cmd.terminal_smaller` | Decrease terminal text size | Réduire la taille du texte du terminal |
| `cmd.auto_memory_off` | Turn off Claude Code's own memory in attached folders | Désactiver la mémoire propre à Claude Code dans les dossiers rattachés |
| `cmd.auto_memory_on` | Leave Claude Code's own memory on in attached folders | Laisser active la mémoire propre à Claude Code dans les dossiers rattachés |
| `cmd.shortcuts` | Keyboard shortcuts | Raccourcis clavier |
| `cmd.kb_open` | Open document or memory... | Ouvrir un document ou une mémoire... |
| `cmd.kb_go` | Go to knowledge base | Aller aux connaissances |
| `cmd.kb_search` | Search knowledge... | Rechercher dans les connaissances... |
| `cmd.kb_review` | Review memories written by Claude Code | Revoir les mémoires écrites par Claude Code |
| `cmd.kb_from_file` | New document from file... | Nouveau document depuis un fichier... |
| `cmd.kb_to_memory` | Change kind to memory | Convertir en mémoire |
| `cmd.kb_to_document` | Change kind to document | Convertir en document |
| `cmd.go_heading` | Go to heading... | Aller au titre... |
| `cmd.save_file` | Save as file... | Enregistrer comme fichier... |
| `cmd.attach` | Attach files... | Joindre des fichiers... |
| `cmd.attachment_save` | Save file as... | Enregistrer le fichier sous... |
| `cmd.export` | Export... | Exporter... |
| `cmd.import` | Import... | Importer... |
| `cmd.trash` | Open trash | Ouvrir la corbeille |
| `cmd.snapshot` | Restore from snapshot... | Restaurer depuis un instantané... |
| `cmd.folders_add` | Add working folder... | Ajouter un dossier de travail... |
| `cmd.folders_show` | Show working folders | Afficher les dossiers de travail |
| `cmd.folder_up` | Move folder up | Monter le dossier |
| `cmd.folder_down` | Move folder down | Descendre le dossier |
| `cmd.folder_locate` | Locate folder... | Localiser le dossier... |
| `cmd.folder_copy` | Copy folder path | Copier le chemin du dossier |
| `cmd.folder_show.w` | Show folder in Explorer (W) | Afficher le dossier dans l'Explorateur |
| `cmd.folder_show.m` | Reveal folder in Finder (M) | Afficher le dossier dans le Finder |
| `cmd.folder_show.l` | Show folder in file manager (L) | Afficher le dossier dans le gestionnaire de fichiers |
| `cmd.folder_remove.topic` | Remove folder from this topic | Retirer le dossier de ce sujet |
| `cmd.folder_remove.epic` | Remove folder from this epic | Retirer le dossier de cet epic |
| `cmd.folder_remove.project` | Remove folder from this project | Retirer le dossier de ce projet |
| `cmd.folders_bulk` | Apply the same change to {n} folder · Apply the same change to {n} folders | Appliquer le même changement à {n} dossier · Appliquer le même changement à {n} dossiers |
| `cmd.folder_switch` | Switch active folder... | Changer de dossier actif... |
| `cmd.folder_active` | Make folder active | Rendre le dossier actif |
| `cmd.folder_settings` | Folder settings... | Réglages du dossier... |
| `cmd.commit_mode` | Set commit mode... | Définir le mode de commit... |
| `cmd.commit_mode.manual` | Manual | Manuel |
| `cmd.commit_mode.commit` | Commit automatically | Commiter automatiquement |
| `cmd.commit_mode.push` | Commit and push automatically | Commiter et pousser automatiquement |
| `cmd.folder_pull` | Pull active folder | Pull du dossier actif |
| `cmd.folder_fetch` | Fetch active folder | Fetch du dossier actif |
| `cmd.folder_branch` | Switch branch of active folder... | Changer de branche du dossier actif... |
| `cmd.dictate` | Dictate | Dicter |
| `cmd.dictate_stop` | Stop dictating | Arrêter la dictée |
| `cmd.voice_settings` | Voice settings | Paramètres de la voix |
| `slash.h1` | Heading 1 | Titre 1 |
| `slash.h2` | Heading 2 | Titre 2 |
| `slash.h3` | Heading 3 | Titre 3 |
| `slash.bullets` | Bulleted list | Liste à puces |
| `slash.numbers` | Numbered list | Liste numérotée |
| `slash.tasks` | Task list | Liste de tâches |
| `slash.quote` | Quote | Citation |
| `slash.code` | Code block | Bloc de code |
| `slash.table` | Table | Tableau |
| `slash.divider` | Divider | Séparateur |
| `ready.title` | Ready for Claude Code | Prêt pour Claude Code |
| `ready.hint` | What Claude Code uses on this computer. Nothing is installed without your click. | Ce que Claude Code utilise sur cet ordinateur. Rien n'est installé sans votre clic. |
| `ready.claude` | Claude Code | Claude Code |
| `ready.claude.why` | Runs in Acta's terminal and does the work. Required. | S'exécute dans le terminal d'Acta et fait le travail. Requis. |
| `ready.signin` | Signed in to Claude | Connecté à Claude |
| `ready.signin.why` | Claude Code needs your Claude subscription. You sign in once, in Acta's terminal. | Claude Code a besoin de votre abonnement Claude. Vous vous connectez une fois, dans le terminal d'Acta. |
| `ready.git` | Git | Git |
| `ready.git.why` | Commits, branches and worktrees. Recommended. | Commits, branches et worktrees. Recommandé. |
| `ready.git.why_windows` | Commits and branches, and Git Bash, the shell Claude Code prefers on Windows. Recommended. | Commits et branches, et Git Bash, le shell que Claude Code préfère sous Windows. Recommandé. |
| `ready.gh` | GitHub CLI | GitHub CLI |
| `ready.gh.why` | Lets Claude Code open pull requests and read issues on GitHub. Optional. | Permet à Claude Code d'ouvrir des pull requests et de lire les issues sur GitHub. Facultatif. |
| `ready.state.found` | {version} | {version} |
| `ready.state.with_bash` | {version} · with Git Bash | {version} · avec Git Bash |
| `ready.state.without_bash` | {version} · without Git Bash: Claude Code will use PowerShell | {version} · sans Git Bash : Claude Code utilisera PowerShell |
| `ready.state.missing` | Not installed | Non installé |
| `ready.state.missing_optional` | Not installed · optional | Non installé · facultatif |
| `ready.state.outdated` | {version} · {minimum} or later needed | {version} · {minimum} ou ultérieure requise |
| `ready.state.installing` | Installing... {elapsed} | Installation... {elapsed} |
| `ready.state.updating` | Updating... {elapsed} | Mise à jour... {elapsed} |
| `ready.state.terminal` | Finish in the terminal | Terminez dans le terminal |
| `ready.state.macos` | Finish in the macOS window | Terminez dans la fenêtre de macOS |
| `ready.state.failed` | Install failed | Échec de l'installation |
| `ready.state.signed_in` | Signed in | Connecté |
| `ready.state.api_key` | Using an API key | Clé API utilisée |
| `ready.state.api_key.tip` | Acta works best with a Claude subscription. | Acta fonctionne au mieux avec un abonnement Claude. |
| `ready.state.signed_out` | Not signed in | Non connecté |
| `ready.install` | Install... | Installer... |
| `ready.update` | Update... | Mettre à jour... |
| `ready.signin_button` | Sign in... | Se connecter... |
| `ready.signin.needs_claude` | Install Claude Code first | Installez d'abord Claude Code |
| `ready.show_command` | Show command | Afficher la commande |
| `ready.download` | Open download page | Ouvrir la page de téléchargement |
| `ready.winget_instead` | Install with winget instead | Installer avec winget à la place |
| `ready.show_terminal` | Show terminal | Afficher le terminal |
| `ready.show_log` | Show log | Afficher le journal |
| `ready.hide_log` | Hide log | Masquer le journal |
| `ready.copy_log` | Copy log | Copier le journal |
| `ready.copy_command` | Copy command | Copier la commande |
| `ready.check_again` | Check again | Vérifier à nouveau |
| `ready.one_at_a_time` | One install at a time | Une installation à la fois |
| `ready.failed.exit` | The installer stopped with code {code}. | Le programme d'installation s'est arrêté avec le code {code}. |
| `ready.failed.not_found` | Installed, but Acta cannot find {name} yet. Restart Acta. | Installé, mais Acta ne trouve pas encore {name}. Redémarrez Acta. |
| `ready.failed.no_start` | Could not start the installer. | Impossible de lancer le programme d'installation. |
| `ready.no_manager` | No package manager Acta can use was found here. Install {name} from its download page, then check again. | Aucun gestionnaire de paquets utilisable par Acta n'a été trouvé. Installez {name} depuis sa page de téléchargement, puis vérifiez à nouveau. |
| `ready.dialog.install` | Install {name}? | Installer {name} ? |
| `ready.dialog.update` | Update {name}? | Mettre à jour {name} ? |
| `ready.from.vendor` | Acta runs {name}'s official installer, exactly as its documentation gives it. | Acta exécute le programme d'installation officiel de {name}, tel que le donne sa documentation. |
| `ready.from.winget` | Acta runs Windows Package Manager (winget), which downloads {name} from its publisher and checks it against the hash in winget's catalogue. | Acta exécute le gestionnaire de paquets de Windows (winget), qui télécharge {name} depuis son éditeur et le vérifie avec l'empreinte du catalogue de winget. |
| `ready.from.homebrew` | Acta runs Homebrew, which downloads {name} and checks it. | Acta exécute Homebrew, qui télécharge {name} et le vérifie. |
| `ready.from.apple` | macOS installs Apple's Command Line Tools, which include Git, in its own window. | macOS installe les Command Line Tools d'Apple, qui contiennent Git, dans sa propre fenêtre. |
| `ready.from.system` | Your system's package manager installs it, in Acta's terminal. It asks for your password there; Acta never sees it. | Le gestionnaire de paquets de votre système l'installe, dans le terminal d'Acta. Il y demande votre mot de passe ; Acta ne le voit jamais. |
| `ready.scope.user` | For you only. No administrator rights needed. | Pour vous seulement. Aucun droit d'administrateur nécessaire. |
| `ready.scope.user_uac` | For you only. Windows may still ask for permission. | Pour vous seulement. Windows peut tout de même demander une autorisation. |
| `ready.scope.system` | For all users of this computer. Administrator rights needed. | Pour tous les utilisateurs de cet ordinateur. Droits d'administrateur nécessaires. |
| `ready.agreements` | Installing accepts the publisher's licence and the terms of winget's catalogue. | Installer vaut acceptation de la licence de l'éditeur et des conditions du catalogue de winget. |
| `ready.updates.self` | Claude Code then updates itself. | Claude Code se met ensuite à jour tout seul. |
| `ready.updates.winget` | Updates come from winget, not from Acta. | Les mises à jour viennent de winget, pas d'Acta. |
| `ready.updates.homebrew` | Updates come from Homebrew, not from Acta. | Les mises à jour viennent de Homebrew, pas d'Acta. |
| `ready.updates.apple` | Updates come from macOS Software Update. | Les mises à jour viennent de la mise à jour de logiciels de macOS. |
| `ready.updates.system` | Updates come with your system's updates, not from Acta. | Les mises à jour arrivent avec celles de votre système, pas d'Acta. |
| `ready.uninstall` | To remove it later, use {manager}; removing Acta leaves it installed. | Pour le retirer plus tard, utilisez {manager} ; désinstaller Acta le laisse en place. |
| `ready.uninstall.native` | the steps on Claude Code's setup page | les étapes de la page d'installation de Claude Code |
| `ready.dialog.command` | Command | Commande |
| `ready.dialog.confirm_install` | Install | Installer |
| `ready.dialog.confirm_update` | Update | Mettre à jour |
| `ready.dialog.confirm_terminal` | Open in terminal | Ouvrir dans le terminal |
| `ready.signin.hint` | A terminal opens with Claude Code. Follow its steps in your browser; Acta never sees your password or token. | Un terminal s'ouvre avec Claude Code. Suivez ses étapes dans votre navigateur ; Acta ne voit jamais votre mot de passe ni votre jeton. |
| `settings.appearance` | Appearance | Apparence |
| `settings.profile` | Profile | Profil |
| `settings.profile.hint` | Claude Code sees your name and job title. | Claude Code voit votre nom et votre fonction. |
| `settings.profile.name` | Name | Nom |
| `settings.profile.name_hint` | Shown on what you write. | Affiché sur ce que vous écrivez. |
| `settings.profile.job_title` | Job title | Fonction |
| `settings.theme` | Theme | Thème |
| `settings.theme.system` | System | Système |
| `settings.theme.light` | Light | Clair |
| `settings.theme.dark` | Dark | Sombre |
| `settings.language` | Language | Langue |
| `settings.language.hint` | What you and Claude Code write is never translated. | Ce que vous et Claude Code écrivez n'est jamais traduit. |
| `settings.text_size` | Text size | Taille du texte |
| `settings.text_size.hint` | Timelines, notes and documents. | Journaux, notes et documents. |
| `settings.text_size.small` | Small | Petit |
| `settings.text_size.default` | Default | Normal |
| `settings.text_size.large` | Large | Grand |
| `settings.reading_width` | Reading width | Largeur de lecture |
| `settings.reading_width.narrow` | Narrow | Étroite |
| `settings.reading_width.default` | Default | Normale |
| `settings.reading_width.wide` | Wide | Large |
| `settings.layout` | Layout | Disposition |
| `settings.layout.desc` | Puts every view back in its default place. | Remet chaque vue à sa place par défaut. |
| `settings.terminal_size` | Terminal text size | Taille du texte du terminal |
| `settings.folders` | Working folders | Dossiers de travail |
| `settings.auto_memory` | Turn off Claude Code's own memory in attached folders | Désactiver la mémoire propre à Claude Code dans les dossiers rattachés |
| `settings.auto_memory.hint` | When a folder is attached, Acta writes autoMemoryEnabled: false to its .claude/settings.local.json, so that what Claude Code learns is kept in Acta, where you can read it. | Quand un dossier est rattaché, Acta écrit autoMemoryEnabled: false dans son .claude/settings.local.json, afin que ce que Claude Code apprend soit conservé dans Acta, où vous pouvez le lire. |
| `settings.not_found` | Folders not found | Dossiers introuvables |
| `settings.auto_commit` | Automatic commits | Commits automatiques |
| `settings.auto_commit.hint` | Default for folders that commit automatically. | Par défaut pour les dossiers qui commitent automatiquement. |
| `settings.claude` | Claude Code | Claude Code |
| `settings.check.files` | Plugin files written | Fichiers du plugin écrits |
| `settings.check.plugin` | Plugin installed | Plugin installé |
| `settings.check.rule` | Permission rule added | Règle d'autorisation ajoutée |
| `settings.check.done` | Done | Fait |
| `settings.check.missing` | Missing | À faire |
| `settings.check.unknown` | Could not check | Vérification impossible |
| `settings.status.ready` | Acta is set up in Claude Code | Acta est configuré dans Claude Code |
| `settings.status.partial` | Setup incomplete: {n} of 3 step done · Setup incomplete: {n} of 3 steps done | Configuration incomplète : {n} étape sur 3 faite · Configuration incomplète : {n} étapes sur 3 faites |
| `settings.status.unknown` | Could not check the setup | Impossible de vérifier la configuration |
| `settings.setup` | Set up Claude Code | Configurer Claude Code |
| `settings.setup_again` | Run setup again | Relancer la configuration |
| `settings.setup_remove` | Remove from Claude Code | Retirer de Claude Code |
| `settings.setup.hint` | Lets Claude Code read and update your projects in Acta. | Permet à Claude Code de lire et de mettre à jour vos projets dans Acta. |
| `settings.setup.what` | What setup changes | Ce que change la configuration |
| `settings.setup.details` | Setup installs Acta's plugin in Claude Code, adds two permission lines (Acta and its browser) and Acta's status line to ~/.claude/settings.json (after your own status line, if you have one), and turns Claude Code's own memory off in the open project's folders. Remove undoes all of it. | La configuration installe le plugin d'Acta dans Claude Code, ajoute deux lignes d'autorisation (Acta et son navigateur) et la ligne d'état d'Acta à ~/.claude/settings.json (après votre propre ligne d'état, si vous en avez une), et désactive la mémoire propre à Claude Code dans les dossiers du projet ouvert. Retirer annule tout cela. |
| `settings.remove.title` | Remove Acta from Claude Code? | Retirer Acta de Claude Code ? |
| `settings.remove.body` | Removes Acta's plugin, permissions and status line from Claude Code, and turns its own memory back on. Your projects stay in Acta. | Retire de Claude Code le plugin, les autorisations et la ligne d'état d'Acta, et réactive sa propre mémoire. Vos projets restent dans Acta. |
| `settings.remove.confirm` | Remove | Retirer |
| `settings.data` | Data | Données |
| `settings.export.last` | Last export: {days} (`time.days` or `time.today_word`) | Dernier export : {days} |
| `settings.export.never` | Never exported | Jamais exporté |
| `settings.snapshots.today` | {n} snapshot, from today, {time} · {n} snapshots, newest today, {time} | {n} instantané, d'aujourd'hui à {time} · {n} instantanés, le plus récent d'aujourd'hui à {time} |
| `settings.snapshots.older` | {n} snapshot, from {datetime} · {n} snapshots, newest {datetime} | {n} instantané, du {datetime} · {n} instantanés, le plus récent du {datetime} |
| `settings.snapshots` | Snapshots | Instantanés |
| `settings.restore` | Restore... | Restaurer... |
| `settings.trash_count` | {n} item · {n} items | {n} élément · {n} éléments |
| `settings.database` | Database | Base de données |
| `settings.attachments` | Attached files: {size} in {count} | Fichiers joints : {size} dans {count} (`{count}` is `attach.count`) |
| `settings.chat` | Chat | Chat |
| `settings.chat_data` | Chat data | Données du chat |
| `settings.about` | About | À propos |
| `settings.release_notes` | Release notes | Notes de version |
| `settings.licences` | Licences | Licences |
| `settings.licences.hint` | JetBrains Mono under the SIL Open Font License, Lucide under ISC. | JetBrains Mono sous licence SIL Open Font License, Lucide sous licence ISC. |
| `settings.licences.hint_linux` | Inter and JetBrains Mono under the SIL Open Font License, Lucide under ISC. | Inter et JetBrains Mono sous licence SIL Open Font License, Lucide sous licence ISC. |
| `statusbar.not_set_up` | Claude Code: not set up | Claude Code : non configuré |
| `statusbar.not_found` | Claude Code: not found | Claude Code : introuvable |
| `statusbar.no_folder` | Claude Code: no folder | Claude Code : aucun dossier |
| `statusbar.ready` | Claude Code: ready | Claude Code : prêt |
| `statusbar.last_wrote` | Last wrote {when} in {topic} | Dernière écriture {when} dans {topic} |
| `statusbar.version` | Acta {version} | Acta {version} |
| `status.claude.missing` | Claude Code: not installed | Claude Code : non installé |
| `status.claude.signed_out` | Claude Code: not signed in | Claude Code : non connecté |
| `update.ready` | Restart to update | Redémarrer pour mettre à jour |
| `update.done` | Updated to {version} | Mise à jour {version} installée |
| `update.failed` | Update failed | Échec de la mise à jour |
| `update.manual` | Download manually | Télécharger manuellement |
| `updater.checking` | Checking for updates... | Recherche de mises à jour... |
| `updater.downloading` | Downloading update... | Téléchargement de la mise à jour... |
| `updater.restarting` | Restarting... | Redémarrage... |
| `export.title` | Export | Exporter |
| `export.scope.all` | Everything | Tout |
| `export.scope.project` | This project | Ce projet |
| `export.warning` | The file is not encrypted: anyone who opens it can read these projects. | Le fichier n'est pas chiffré : quiconque l'ouvre peut lire ces projets. |
| `export.include_files` | Include attached files ({size}) | Inclure les fichiers joints ({size}) |
| `export.too_large` | With its files this export would be about {size}, more than an import can read ({limit}). Export without files: their names and sizes stay. | Avec ses fichiers, cet export ferait environ {size}, plus qu'un import ne peut en lire ({limit}). Exportez sans les fichiers : leurs noms et tailles sont conservés. |
| `export.passphrase` | Protect with a passphrase | Protéger par une phrase secrète |
| `export.passphrase.lost` | Acta cannot recover a lost passphrase. | Acta ne peut pas retrouver une phrase secrète perdue. |
| `export.choose` | Choose location... | Choisir l'emplacement... |
| `export.done` | Exported {n} project to {file}. · Exported {n} projects to {file}. | {n} projet exporté vers {file}. · {n} projets exportés vers {file}. |
| `export.show` | Show in folder | Afficher dans le dossier |
| `export.suggest` | Everything is on this computer only. An export is your copy if the keychain is lost. | Tout est sur cet ordinateur uniquement. Un export est votre copie si le trousseau est perdu. |
| `import.title` | Import "{file}" | Importer « {file} » |
| `import.meta` | Exported {date} · format {n} · {projects} (`count.projects`) | Exporté le {date} · format {n} · {projects} |
| `import.created` | Created | Créés |
| `import.updated` | Updated | Mis à jour |
| `import.skipped` | Skipped | Ignorés |
| `import.kind.projects` | Projects | Projets |
| `import.kind.epics` | Epics | Epics |
| `import.kind.topics` | Topics | Sujets |
| `import.kind.entries` | Entries | Entrées |
| `import.kind.notes` | Notes | Notes |
| `import.kind.documents` | Documents | Documents |
| `import.kind.memories` | Memories | Mémoires |
| `import.safe` | Nothing will be deleted. Skipped records are the same as Acta's, or older. | Rien ne sera supprimé. Les éléments ignorés sont identiques à ceux d'Acta, ou plus anciens. |
| `import.show_changes` | Show what changes | Afficher ce qui change |
| `import.confirm` | Import | Importer |
| `import.done` | Imported: {created}, {updated}. A snapshot from before the import is kept. (`import.count.*`) | Import terminé : {created}, {updated}. Un instantané d'avant l'import est conservé. |
| `import.count.created` | {n} created | {n} créé · {n} créés |
| `import.count.updated` | {n} updated | {n} mis à jour |
| `import.folders` | {n} working folder was not found on this computer. Locate it from its project, epic or topic. · {n} working folders were not found on this computer. Locate each one from its project, epic or topic. | {n} dossier de travail est introuvable sur cet ordinateur. Localisez-le depuis son projet, son epic ou son sujet. · {n} dossiers de travail sont introuvables sur cet ordinateur. Localisez chacun depuis son projet, son epic ou son sujet. |
| `import.network_folders` | {n} working folder on a network path was left out. Add it again from its project, epic or topic. · {n} working folders on a network path were left out. Add each one again from its project, epic or topic. | {n} dossier de travail sur un chemin réseau a été écarté. Ajoutez-le de nouveau depuis son projet, son epic ou son sujet. · {n} dossiers de travail sur un chemin réseau ont été écartés. Ajoutez chacun de nouveau depuis son projet, son epic ou son sujet. |
| `import.files_missing` | {n} file was not in the export. · {n} files were not in the export. | {n} fichier n'était pas dans l'export. · {n} fichiers n'étaient pas dans l'export. |
| `import.recovered_manual` | Commit modes come back as Manual: set them again in each folder's settings. | Les modes de commit reviennent en Manuel : redéfinissez-les dans les réglages de chaque dossier. |
| `import.newer` | This file was made by a newer Acta (format {n}). Update Acta, then import it. | Ce fichier a été créé par une version plus récente d'Acta (format {n}). Mettez Acta à jour, puis importez-le. |
| `import.not_acta` | This is not an Acta export. | Ce n'est pas un export Acta. |
| `import.too_large` | This file is too large to import ({size}). The limit is {limit}. Export one project at a time, then import each file. | Ce fichier est trop volumineux pour être importé ({size}). La limite est de {limit}. Exportez un projet à la fois, puis importez chaque fichier. |
| `import.wrong_passphrase` | Wrong passphrase. | Phrase secrète incorrecte. |
| `trash.hint` | Deleted things stay here for 30 days. | Les éléments supprimés restent ici 30 jours. |
| `trash.by` | by {author} | par {author} |
| `trash.project_content` | {topics}, {notes} (`count.topics`, `count.notes`) | {topics}, {notes} |
| `trash.restored_with_epic` | Topic and its epic restored. | Sujet et son epic restaurés. |
| `trash.restored` | Item restored. | Élément restauré. |
| `trash.days` | {n} day left · {n} days left | {n} jour restant · {n} jours restants |
| `trash.empty` | Trash is empty. | La corbeille est vide. |
| `trash.merged_into` | Merged into {name} | Fusionné avec : {name} |
| `trash.kept_named` | Kept: still named in {n} place · Kept: still named in {n} places | Conservé : encore cité à {n} endroit · Conservé : encore cité à {n} endroits |
| `snapshot.daily` | Daily | Quotidien |
| `snapshot.weekly` | Weekly | Hebdomadaire |
| `snapshot.before_update` | Before update to {version} | Avant la mise à jour vers {version} |
| `snapshot.before_import` | Before import | Avant l'import |
| `snapshot.before_restore` | Before restore | Avant la restauration |
| `snapshot.confirm` | Restore and restart | Restaurer et redémarrer |
| `snapshot.title` | Restore from snapshot | Restaurer depuis un instantané |
| `snapshot.body` | Acta will restart with its data as of {datetime}. Everything as it is now is saved as a snapshot first, so you can come back. | Acta redémarrera avec ses données du {datetime}. La situation actuelle est d'abord enregistrée dans un instantané : vous pourrez y revenir. |
| `snapshot.restored` | Restored the snapshot from {datetime}. | Instantané du {datetime} restauré. |
| `snapshot.migration` | The update to {version} could not convert the database. Acta put back the snapshot from just before; nothing was lost. | La mise à jour vers {version} n'a pas pu convertir la base de données. Acta a restauré l'instantané pris juste avant ; rien n'a été perdu. |
| `snapshot.size` | {size} MB | {size} Mo |
| `keyloss.title` | Acta cannot open its database | Acta ne peut pas ouvrir sa base de données |
| `keyloss.body` | The database is on this computer and intact, but its key is missing from {keychain}. (`keychain.*`) This usually follows a system reinstall, a new user profile or a move to another machine. | La base de données est sur cet ordinateur et intacte, mais sa clé est absente {keychain}. Cela arrive généralement après une réinstallation du système, la création d'un nouveau profil utilisateur ou un changement de machine. |
| `keyloss.body.m` | The database is on this computer and intact, but its key file is missing from Acta's data folder. This usually follows a restore from a backup that left it out, a new user profile or a move to another machine. | La base de données est sur cet ordinateur et intacte, mais son fichier de clé manque dans le dossier de données d'Acta. Cela suit en général une restauration de sauvegarde qui l'a omis, un nouveau profil utilisateur ou un changement de machine. |
| `keyloss.body.unreadable` | Acta found its key, but the database cannot be read with it: the file may be damaged, or the key belongs to another database. This can follow a disk error or a copy of the file from another computer. | Acta a trouvé sa clé, mais elle ne permet pas de lire la base de données : le fichier est peut-être endommagé, ou la clé appartient à une autre base. Cela peut suivre une erreur de disque ou une copie du fichier depuis un autre ordinateur. |
| `keychain.w` | Windows Credential Manager | du Gestionnaire d'identification Windows |
| `keychain.m` | the macOS Keychain | du trousseau macOS |
| `keychain.l` | the Secret Service keyring | du service Secret Service |
| `keyloss.untouched` | Acta has not changed the file: | Acta n'a pas modifié le fichier : |
| `keyloss.import` | Import an export... | Importer un export... |
| `keyloss.limit` | Without the key or an export, the content cannot be read. You can start empty; the file is kept as {file}. | Sans la clé ni export, le contenu ne peut pas être lu. Vous pouvez repartir de zéro ; le fichier est conservé sous le nom {file}. |
| `keyloss.start_empty` | Start empty | Repartir de zéro |
| `openfail.title` | Acta could not open its database | Acta n'a pas pu ouvrir sa base de données |
| `openfail.body` | Something on this computer kept Acta from reading its data. Nothing was changed. Try again; if it keeps failing, the log in the data folder says what failed. | Quelque chose sur cet ordinateur empêche Acta de lire ses données. Rien n'a été modifié. Réessayez ; si l'échec se répète, le journal du dossier de données indique la cause. |
| `openfail.nodir` | Acta could not find where to keep its data on this computer. | Acta ne trouve pas où conserver ses données sur cet ordinateur. |
| `openfail.show_log` | Show the log | Afficher le journal |
| `firstrun.title` | Start with a project | Commencez par un projet |
| `firstrun.body` | A project is a folder you work on with Claude Code. Acta keeps its topics, decisions and notes. | Un projet est un dossier sur lequel vous travaillez avec Claude Code. Acta en garde les sujets, les décisions et les notes. |
| `firstrun.choose` | Choose a folder | Choisir un dossier |
| `firstrun.then` | Then | Ensuite |
| `firstrun.step1` | Connect Claude Code | Connecter Claude Code |
| `firstrun.step2` | Capture a first note | Saisir une première note |
| `firstrun.step3` | Open the terminal | Ouvrir le terminal |
| `shortcuts.filter` | Filter shortcuts | Filtrer les raccourcis |
| `shortcuts.none` | No shortcut matches "{query}". | Aucun raccourci ne correspond à « {query} ». |
| `shortcuts.group.general` | General | Général |
| `shortcuts.group.tabs` | Tabs, views and layout | Onglets, vues et disposition |
| `shortcuts.group.lists` | Lists and tree | Listes et arborescence |
| `shortcuts.group.editors` | Editors | Éditeurs |
| `shortcuts.group.terminal` | Terminal and chat | Terminal et chat |
| `shortcuts.palette` | Command palette | Palette de commandes |
| `shortcuts.next_tab` | Next tab | Onglet suivant |
| `shortcuts.previous_tab` | Previous tab | Onglet précédent |
| `shortcuts.focus_projects` | Focus Projects | Aller aux projets |
| `shortcuts.focus_page` | Focus the page area | Aller à la zone des pages |
| `shortcuts.focus_terminal` | Focus the terminal | Aller au terminal |
| `shortcuts.focus_chat` | Focus the chat | Aller au chat |
| `shortcuts.next_area` | Focus next area (reserved) | Aller à la zone suivante (réservé) |
| `shortcuts.previous_area` | Focus previous area (reserved) | Aller à la zone précédente (réservé) |
| `shortcuts.maximize` | Maximize focused area (reserved) | Agrandir au maximum la zone active (réservé) |
| `shortcuts.list.move` | Move the cursor | Déplacer le curseur |
| `shortcuts.list.ends` | First or last row | Première ou dernière ligne |
| `shortcuts.list.fold` | Collapse or expand | Replier ou déplier |
| `shortcuts.list.preview` | Preview, focus stays in the list | Aperçu, le focus reste dans la liste |
| `shortcuts.list.status` | Set status, then 1 to 6 | Changer le statut, puis 1 à 6 |
| `shortcuts.list.reorder` | Move up or down | Monter ou descendre |
| `shortcuts.list.menu` | Open the menu | Ouvrir le menu |
| `shortcuts.editor.save` | Save the editor in focus | Enregistrer l'éditeur actif |
| `shortcuts.editor.cancel` | Cancel the edit | Annuler la modification |
| `view.agent_browser` | Agent browser | Navigateur de l'agent |
| `browser.address` | Address | Adresse |
| `browser.idle.title` | Claude Code's browser | Le navigateur de Claude Code |
| `browser.idle.body` | Claude Code opens pages here: your dev server, or sites you signed into here. It never opens claude.ai and never types passwords. | Claude Code ouvre ses pages ici : votre serveur de développement, ou les sites auxquels vous vous êtes connecté ici. Il n'ouvre jamais claude.ai et ne saisit jamais de mot de passe. |
| `browser.starting` | Starting the browser... | Démarrage du navigateur... |
| `browser.hidden` | The browser is hidden while this is open. | Le navigateur est masqué tant que ceci est ouvert. |
| `browser.stop` | Stop Claude Code | Arrêter Claude Code |
| `browser.resume` | Resume | Reprendre |
| `browser.stopped` | Stopped. Claude Code is told to ask you before it continues. | Arrêté. Claude Code doit vous demander avant de continuer. |
| `browser.handoff` | Claude Code asks you: {message} | Claude Code vous demande : {message} |
| `browser.done` | Done | Terminé |
| `browser.driver` | driven by {session} | piloté par {session} |
| `browser.driver.unnamed` | driven by Claude Code | piloté par Claude Code |
| `browser.act.opened` | Claude Code opened {target} | Claude Code a ouvert {target} |
| `browser.act.reloaded` | Claude Code reloaded the page | Claude Code a rechargé la page |
| `browser.act.back` | Claude Code went back | Claude Code est revenu en arrière |
| `browser.act.forward` | Claude Code went forward | Claude Code est allé en avant |
| `browser.act.clicked` | Claude Code clicked {target} | Claude Code a cliqué sur {target} |
| `browser.act.typed` | Claude Code typed into {target} | Claude Code a saisi du texte dans {target} |
| `browser.act.screenshot` | Claude Code took a screenshot | Claude Code a pris une capture d'écran |
| `browser.act.waiting` | Claude Code is waiting for you | Claude Code vous attend |
| `browser.clear` | Clear browser data... | Effacer les données du navigateur... |
| `browser.clear.title` | Clear the browser's data? | Effacer les données du navigateur ? |
| `browser.clear.body` | This deletes the cookies and site data of Acta's browser from this computer: you are signed out of every site you signed into there. | Cela supprime de cet ordinateur les cookies et les données de site du navigateur d'Acta : vous êtes déconnecté de tous les sites auxquels vous vous y étiez connecté. |
| `browser.clear.confirm` | Clear data | Effacer les données |
| `browser.clear_failed` | The browser data could not be deleted. Close what may use it and try again. | Les données du navigateur n'ont pas pu être supprimées. Fermez ce qui peut les utiliser et réessayez. |
| `browser.refused.title` | Acta did not start its browser | Acta n'a pas démarré son navigateur |
| `browser.refused.body` | A debugging port or another WebView2 override is configured for Acta (a WebView2 policy, an environment variable or a DevToolsActivePort file). It could let another program control or read the browser. Remove it, then try again. | Un port de débogage ou un autre réglage WebView2 est configuré pour Acta (une stratégie WebView2, une variable d'environnement ou un fichier DevToolsActivePort). Il pourrait permettre à un autre programme de piloter ou de lire le navigateur. Supprimez-le, puis réessayez. |
| `browser.runtime.title` | The agent browser needs WebView2 | Le navigateur de l'agent a besoin de WebView2 |
| `browser.runtime.body` | WebView2 is not installed, so the browser cannot start. Everything else works. | WebView2 n'est pas installé : le navigateur ne peut pas démarrer. Tout le reste fonctionne. |
| `browser.failed.title` | The browser could not be started | Le navigateur n'a pas pu être démarré |
| `browser.pipe.taken` | Claude Code cannot reach this browser: another copy of Acta, or another program, already serves it. Close it and restart Acta. | Claude Code ne peut pas joindre ce navigateur : une autre copie d'Acta, ou un autre programme, le sert déjà. Fermez-le et redémarrez Acta. |
| `browser.pipe.failed` | Claude Code cannot reach this browser. Restart Acta; the log says why. | Claude Code ne peut pas joindre ce navigateur. Redémarrez Acta ; le journal indique pourquoi. |
| `terminal.claude` | Claude | Claude |
| `terminal.waiting` | waiting for you | vous attend |
| `view.sessions` | Claude Code sessions | Sessions Claude Code |
| `sessions.working` | {n} working | {n} au travail |
| `sessions.waiting` | {n} waiting for you · {n} waiting for you | {n} vous attend · {n} vous attendent |
| `sessions.idle` | {n} idle · {n} idle | {n} inactive · {n} inactives |
| `sessions.running` | {n} running · {n} running | {n} en cours · {n} en cours |
| `statusbar.sessions` | Claude Code: {list} | Claude Code : {list} |
| `sessions.state.working` | Working | Au travail |
| `sessions.state.waiting` | Waiting for you | Vous attend |
| `sessions.state.idle` | Idle | Inactive |
| `sessions.since` | since {time} | depuis {time} |
| `sessions.last` | last: {activity} | dernière action : {activity} |
| `sessions.last_tool` | last tool: {tool} | dernier outil : {tool} |
| `sessions.in_acta` | in Acta's terminal | dans le terminal d'Acta |
| `sessions.outside` | in another terminal | dans un autre terminal |
| `sessions.empty` | No Claude Code session is running. | Aucune session Claude Code en cours. |
| `sessions.show_terminal` | Show terminal | Afficher le terminal |
| `sessions.end` | End session | Terminer la session |
| `sessions.here.working` | Claude Code is working here | Claude Code travaille ici |
| `sessions.here.waiting` | Claude Code is waiting for you here | Claude Code vous attend ici |
| `sessions.here.idle` | Claude Code is open here | Claude Code est ouvert ici |
| `sessions.here.many` | {n} Claude Code session here · {n} Claude Code sessions here | {n} session Claude Code ici · {n} sessions Claude Code ici |
| `settings.resume` | Continue the last conversation of each page | Reprendre la dernière conversation de chaque page |
| `settings.flash_waiting` | Flash Acta in the taskbar when a session waits for you | Faire clignoter Acta dans la barre des tâches quand une session vous attend |
| `settings.flash_waiting.hint` | Only while Acta is in the background. | Seulement quand Acta est en arrière-plan. |
| `cmd.resume_on` | Continue the last conversation of each page | Reprendre la dernière conversation de chaque page |
| `cmd.resume_off` | Start a new Claude Code conversation each time | Commencer une nouvelle conversation Claude Code à chaque fois |
| `cmd.flash_on` | Flash Acta in the taskbar when a session waits for you | Faire clignoter Acta dans la barre des tâches quand une session vous attend |
| `cmd.flash_off` | Do not flash Acta when a session waits | Ne pas faire clignoter Acta quand une session attend |
| `usage.five_hour` | 5h {percent} | 5 h {percent} |
| `usage.seven_day` | 7d {percent} | 7 j {percent} |
| `usage.five_hour.tip` | 5-hour limit: {percent} used | Limite sur 5 heures : {percent} utilisés |
| `usage.seven_day.tip` | 7-day limit: {percent} used | Limite sur 7 jours : {percent} utilisés |
| `usage.resets_at` | resets at {time} | réinitialisée à {time} |
| `usage.resets_on` | resets {day} {time} | réinitialisée {day} à {time} |
| `usage.stale` | This window has reset since Claude Code last reported it. | Cette fenêtre a été réinitialisée depuis le dernier relevé de Claude Code. |
| `usage.no_data.tip` | Claude Code reports your plan's limits after the first answer of a session (Pro, Max and Ultra plans), through Acta's status line. | Claude Code signale les limites de votre forfait après la première réponse d'une session (forfaits Pro, Max et Ultra), par la ligne d'état d'Acta. |
| `settings.usage` | Show plan usage in the status bar | Afficher l'utilisation du forfait dans la barre d'état |
| `cmd.usage_on` | Show plan usage in the status bar | Afficher l'utilisation du forfait dans la barre d'état |
| `cmd.usage_off` | Hide plan usage | Masquer l'utilisation du forfait |
| `settings.setup.title` | Set up Claude Code? | Configurer Claude Code ? |
| `settings.setup.body` | Acta adds its plugin, two permissions and its status line to ~/.claude/settings.json. Your own status line still shows first. | Acta ajoute son plugin, deux autorisations et sa ligne d'état à ~/.claude/settings.json. Votre propre ligne d'état reste affichée en premier. |
| `settings.setup.folders` | It also turns Claude Code's own memory off in {n} folder of {project}. · It also turns Claude Code's own memory off in {n} folders of {project}. | Il désactive aussi la mémoire propre à Claude Code dans {n} dossier de {project}. · Il désactive aussi la mémoire propre à Claude Code dans {n} dossiers de {project}. |
| `settings.setup.confirm` | Set up | Configurer |
| `settings.plugin.hint` | Sessions started in Acta's terminal always load Acta's plugin. To stop that, set "acta@inline": false under enabledPlugins. | Les sessions lancées dans le terminal d'Acta chargent toujours le plugin d'Acta. Pour l'éviter, mettez "acta@inline": false sous enabledPlugins. |

Strings that exist in the behaviour sections but are not in this table are a defect of the table: add the row in the same change that adds the string.

### 10.4.1 Keys of the workbench layout (4.8)

Moved into 10.4 when 4.8 was implemented (M8), with the texts that replaced "Toggle sidebar", "Zoom tool window" and "Tabs and tool windows"; the commands that moved a tool window to a dock and widened or narrowed a pane left with them. `{view}` is always a view's name key (`view.projects`, `tree.inbox`, `tree.knowledge`, `terminal.title`, `chat.title`), never translated twice or inflected: the colon form keeps every sentence whole in French, where a view's gender would otherwise change the participle. `view.agent_browser` ("Agent browser", "Navigateur de l'agent") joined 10.4 with ADR 0010, with the `browser.*` keys of the Agent browser view (components-app.md 5).

### 10.4.2 Keys of the tree (4.2.1)

Moved into 10.4 when 4.2.1 was implemented: `tree.unread`, `tree.empty` and `tree.collapse_all` right after `tree.move_epic`, `cmd.tree_collapse_all` after `cmd.new_project`. The ghost row reuses `topic.new` and the empty tree's button `cmd.new_project`. The Knowledge row's count is gone from the tree, so no key changes for it.

### 10.4.3 Keys of folder sets and commit modes (ADR 0011)

Moved into 10.4 when the folders UI of ADR 0011 was implemented: removed `folders.primary`, `folders.include`, `folders.not_used`, `folders.set_here`, `folders.uses`, `folders.make_primary`, `cmd.folder_primary`, `cmd.folders_include`, `cmd.folders_exclude` and `terminal.missing.body` (replaced by its three level keys); `folders.hint` changed; the new `folders.*` keys after `folders.picker`, `terminal.missing.body.*`, `terminal.missing.other`, `terminal.running_in` and `terminal.restart_in` after `terminal.restart.confirm`, `cmd.folder_*` and `cmd.commit_mode*` after `cmd.folders_bulk`, `settings.auto_commit*` after `settings.not_found`, `import.auto_commit` after `import.wrong_passphrase` (retired after the security audit of 2026-11-22: an import never brings commit modes, ADR 0011 amended; removed from the table and the code). The checkbox of `settings.auto_commit` reuses `folders.commit.as_user`. `{branch}` is the branch's name as git has it, never translated; `{key}` is a settings key (`attribution`, `env`), never translated. The `folders.git.*` keys of the pull and branch actions (ADR 0011 addendum, components-folders.md 5, Pull and branch) and `cmd.folder_pull`, `cmd.folder_fetch`, `cmd.folder_branch` were added with them. *Pull* and *Fetch* stay in English in French, as git's own verbs; `{message}` is git's last line, untranslated.

### 10.4.4 Keys of due dates (10.2.1)

Moved into 10.4 when due dates were implemented: `prop.due` after `prop.level`, the `due.*` keys right after it, `event.due_*` after `event.status`, `table.due` and `table.filter.overdue`, `table.filter.week` after `table.reset`, `cmd.due_*` after `cmd.set_status`. Month and weekday names and the compact form are formats of 10.2.1, not keys. In French, *échéance* is feminine ("Aucune"), and `{date}` takes "le" or "au" from the string (10.2). `due.in_days` and `due.tip.in_days` are used from 2 days ahead (today and tomorrow have their own keys), so their singular forms only serve the plural rule. `due.tip.date` is the tooltip of a date on a closed record, which is never "overdue". The picker's Clear button reuses `common.clear`. Agent-facing text (`due 11-27 (6d)`, `overdue 3d`) is protocol, English only, and not in this catalogue (10).

### 10.4.5 Keys of the Overview (components-records.md 5)

Moved into 10.4 when the Overview was implemented: `overview.*` after `section.knowledge`, `filter.*` after `table.filter.week`, `cmd.show_*` and `cmd.overview_*` after `cmd.go_inbox`. Reused, not duplicated: "No epic" is `tree.no_epic`, the Knowledge block label is `section.knowledge`. Every `{n}` and `{days}` takes `tnum`; `{days}` is always 7 or more, so its sentences are written in the plural; `{when}` is the long form of 10.2 and the French brings its own preposition ("Dernière activité mardi à 14:05", "Dernière activité le 12 nov. 2026"). Memories are feminine in French (*mémoire*), topics masculine (*sujet*), epics masculine (*epic*). `overview.inbox` is chosen by `{n}`; with one note, "the oldest" would be wrong, so the singular form names that note's age directly. `filter.decided_week` lists topics, so "Décidés" agrees with *sujets*. Agent-facing text (`Attention: blocked T-12 ...`, `· no activity 12d`) is protocol, English only, and not in this catalogue (10).

### 10.4.6 Keys of questions and people

Amended 2026-11-22: people are contacts (components-contacts.md 5); the `person.*` keys that 10.4.11 lists as replaced are not moved into 10.4.

New with questions and Report to (2026-11-21; components-questions.md 5, data.md 11 requests 18 and 19). Moved into 10.4 when questions and Report to were implemented (branch `gpui-engineer/questions-ui`): `section.questions` after `section.description`, `prop.report_to` after `prop.due`, `event.report_*` after `event.due_*`, `table.questions*` and `table.report_to` after `table.due`, the `cmd.*` keys after `cmd.due_*` (`cmd.show_questions_late` after `cmd.show_stale_memories`, `overview.questions_late` after `overview.memories`), the other families together after `kb.*`. In French, *question* is feminine ("répondue", "abandonnée", "close"); the person is user text and always follows a colon in French ("Posée à : Marie", "À rapporter à : le service RH"), so no article or preposition ever meets it (10.3). `{name}` is a person or a record name, never translated; `{date}` is the absolute date of 10.2.

`questions.more_*_overdue` replace `questions.more_*` when `{late}` is above 0, and `table.questions_tip_overdue` replaces `table.questions_tip` the same way (its first form is for one question, which is then the overdue one). The overdue clause ("1 overdue", "dont 1 en retard") is drawn in `warning` as one run (components-questions.md 5): the implementation marks the span of that clause in the formatted string, it never builds the sentence from fragments. `report.pending`, `report.done` and the events agree with *sujet* and *epic*, both masculine. The relative phrases of an answer date reuse `due.*` (10.4.4); the empty Answer by value reuses `due.none` (« Aucune », agreeing with *date*), Clear in the popover `common.clear`, and the remaining menu items `common.edit` and `common.delete`. Agent-facing text (`Q-12 Marie · overdue 3d`, `report to Marie`) is protocol, English only, and not in this catalogue (10).

### 10.4.7 Keys of attachments

New with attachments (2026-11-21; components-attachments.md 5, ADR 0012, data.md 11 request 20). Moved into 10.4 and `catalogue.rs` when attachments were implemented (branch `gpui-engineer/attachments-ui`): the `attach.*` family after the `inbox.*` keys, `settings.attachments` after `settings.database`, `export.include_files` and `export.too_large` (the ADR 0012 amendment's copy) after `export.warning`, `import.files_missing` after `import.network_folders`, `cmd.attach` and `cmd.attachment_save` after `cmd.save_file`. Added with the implementation, for review: `attach.saved` and `attach.show_in_folder` (the Save as toast and its action, components-attachments.md 5; the French of `attach.saved` was not in the spec), and `attach.gone` (the error toast when a file an editor stored ahead of its row was purged before the record was saved, from the review of 2026-11-22). In French the word is *fichier* (masculine): "Fichier retiré". `{size}` is the size format of components-attachments.md 5 ("2.1 MB" / "2,1 Mo"); `{name}` a file name, never translated; `{folder}` the folder's path as the system gives it.

`{date}` and `{time}` in `attach.pasted_name` are `2026-11-21` and `14.05` in both languages: a file name is not localised (10.2). The units B / KB / MB and o / Ko / Mo are formats, not keys; "25 Mo" and "10 Mo" inside the French sentences take U+202F before the unit (10.1). `attach.moved_to` is plural in French because it heads a list of files. ("Open in another app" was rejected by the user, ADR 0012, 8: its three keys are gone.) Agent-facing text (`· files: A-4 plan.pdf 2.1M`) is protocol, English only, and not in this catalogue (10).

### 10.4.8 Keys of Ready for Claude Code (ADR 0013)

New with ADR 0013 (2026-11-22; components-app.md 5, screens.md 8.26). Moved into 10.4 and `catalogue.rs` when the checklist was implemented, as a `ready.*` family before the `settings.*` keys, with the two `status.*` keys beside the status bar's; `settings.check.cli` is removed (its row is now `ready.claude`). Added with the implementation, for review: `quit.installing` (the quit confirmation while an install runs) and `terminal.ended_once` (the last line of a one-off terminal session, which has no restart). `{name}` is a tool's name, never translated ("Git", "GitHub CLI", "Claude Code"); `{version}` and `{minimum}` are version numbers as the tool prints them; `{elapsed}` is `m:ss`; `{code}` the exit code in decimal. Package ids and commands are protocol, not copy, and are never translated.

`{manager}` in `ready.uninstall` is the manager's name as a proper noun ("winget", "Homebrew", "apt"), or `ready.uninstall.native` for Claude Code's native install. `ready.agreements` is shown only for winget commands, which pass `--accept-package-agreements` and `--accept-source-agreements` (winget has no stdin to ask on). Retry on a failed row is `common.retry`; everywhere else re-checking is `ready.check_again`.

### 10.4.9 Keys of dictation (ADR 0014)

New with dictation (2026-11-22; components-voice.md 5, screens.md 8.27). Moved into 10.4 and `catalogue.rs` when dictation was implemented (branch `gpui-engineer/voice-ui`): the `voice.*` family after the `attach.*` keys, `cmd.dictate`, `cmd.dictate_stop` and `cmd.voice_settings` after `cmd.folder_branch`. Added with the implementation, for review: `voice.setup.unsupported_cpu`, the first-use dialog's line on an x86-64 processor without AVX2 and FMA (ADR 0014, amendment: `hardware::Reason::UnsupportedCpu`), shown with Close only, as `voice.setup.unavailable`. In French the verb is *dicter* and the noun *dictée* (feminine: "Dictée désactivée"). "Whisper" and the model names (Tiny, Base, Small, Large turbo) are names, kept as written in both languages. `{size}`, `{done}`, `{free}` and `{memory}` use the size format of components-attachments.md 5 (1 KB = 1,024 bytes, as the file managers show sizes: Small is "181 MB", Large turbo "547 MB"; the decimal "190 MB" and "574 MB" of the earlier text are the Hugging Face page's figures); `{percent}` is a whole number with `%` (French: a narrow no-break space before `%`, 10.2); `{shortcut}` is the key as the kbd hint names it on the platform; `{model}` a model name; `{path}` a folder in `type.code`.

The macOS `Info.plist` usage strings (ADR 0014, 8) live with the bundle (`InfoPlist.strings`), not in this catalogue: `NSMicrophoneUsageDescription` "Acta turns what you say into text, on this Mac." / "Acta transforme ce que vous dites en texte, sur ce Mac."; `NSSpeechRecognitionUsageDescription` "Lets the macOS recogniser turn what you say into text, on this Mac." / "Permet à la reconnaissance de macOS de transformer ce que vous dites en texte, sur ce Mac." The delete dialog's buttons and "Show in Explorer" (Finder, file manager) reuse their existing keys.

### 10.4.10 Clarity pass and the timeline without a composer

From the clarity review of 2026-11-22 ([reviews/2026-11-22-clarity.md](reviews/2026-11-22-clarity.md)) and the user's decision of the same day that only Claude Code adds timeline entries (components-records.md 5). **Applied to 10.4 and `catalogue.rs`** (branch `gpui-engineer/polish-2`), with these differences from the lists below, all from the user's ruling of copy.md 7 (one plain line or none, never mechanics) and the QA of the same day: `composer.hint` stays (every multi-line editor's footer uses it); `settings.reading_width.hint`, `settings.resume.hint`, `settings.usage.hint` and `browser.idle.memory` are removed rather than shortened (each restated its label or explained mechanics); `settings.setup.hint` reads "Lets Claude Code read and update your projects in Acta." and is the Set up row's label, not a description under a label repeated on the button; `settings.flash_waiting.hint` keeps its first sentence; `topic.empty_project` drops "No topics yet.", which its heading already says; `prop.id` and `prop.updated` are removed (the meta line replaced both rows, on documents too); `meta.updated` ("Updated {when} by {author}", the meta line of a project, which shows no id), `meta.copy_id` ("Copy id", the id's tooltip) and `palette.epics` ("Epics", the palette's group of epics) are new; `prop.add_report_to` stayed pending with Report to and moved into 10.4 with 10.4.6. The French colon takes U+00A0 (copy.md 10.1). The rule of the pass: one idea per line; what a person needs once (how a setting works, what setup writes, memory use) goes in a tooltip or behind "Details", never on screen at rest.

**Removed** from 10.4: `composer.placeholder`, `composer.hint`, `composer.add`, `cmd.add_entry` (no hand-written entries); `settings.theme.hint` (the segment "System" says it); `usage.no_data` (the status bar shows nothing until there is a number; `usage.no_data.tip` becomes the tooltip of the Settings row "Show plan usage in the status bar" while no report has arrived); `kb.empty.epic` and `kb.empty.project` (one text for every level, `kb.empty`).

**Changed** text in 10.4:

| Key | English | French |
|---|---|---|
| `timeline.empty` | No entries yet. Claude Code logs progress here as it works. | Aucune entrée pour l'instant. Claude Code consigne ici l'avancement au fil du travail. |
| `topic.empty_project` | Capture a note, or create a topic. | Saisissez une note ou créez un sujet. |
| `inbox.empty` | Inbox is empty. Captured notes wait here until they are filed. | La boîte de réception est vide. Les notes saisies attendent ici d'être classées. |
| `inbox.filed_empty` | No filed notes yet. | Aucune note classée pour l'instant. |
| `kb.empty` | No documents or memories yet. | Aucun document ni mémoire pour l'instant. |
| `overview.idle` | {n} active topic idle for 7 days · {n} active topics idle for 7 days | {n} sujet en cours inactif depuis 7 jours · {n} sujets en cours inactifs depuis 7 jours |
| `overview.inbox` | {n} note in the inbox for {days} days · {n} notes in the inbox, oldest {days} days | {n} note dans la boîte de réception depuis {days} jours · {n} notes dans la boîte de réception, la plus ancienne depuis {days} jours |
| `overview.week.none` | Nothing done, decided or opened this week. Last activity {when}. | Rien de terminé, décidé ni ouvert cette semaine. Dernière activité {when}. |
| `statusbar.no_folder` | Claude Code: no folder | Claude Code : aucun dossier |
| `update.ready` | Restart to update | Redémarrer pour mettre à jour |
| `settings.text_size.hint` | Timelines, notes and documents. | Journaux, notes et documents. |
| `settings.reading_width.hint` | Maximum page width. | Largeur maximale des pages. |
| `settings.language.hint` | What you and Claude Code write is never translated. | Ce que vous et Claude Code écrivez n'est jamais traduit. |
| `settings.layout.desc` | Puts every view back in its default place. | Remet chaque vue à sa place par défaut. |
| `settings.setup.hint` | Installs Acta's plugin, permissions and status line in Claude Code. | Installe le plugin, les autorisations et la ligne d'état d'Acta dans Claude Code. |
| `settings.resume.hint` | Resumes the conversation last held on that page. /clear starts a new one. | Reprend la dernière conversation menée sur cette page. /clear en commence une nouvelle. |
| `settings.flash_waiting.hint` | Only while Acta is in the background. It never takes the focus. | Seulement quand Acta est en arrière-plan. Il ne prend jamais le focus. |
| `settings.usage.hint` | Your plan's 5-hour and 7-day limits, as Claude Code last reported them. | Les limites sur 5 heures et sur 7 jours de votre forfait, selon le dernier relevé de Claude Code. |
| `settings.plugin.hint` | Sessions started in Acta's terminal always load Acta's plugin. To stop that, set "acta@inline": false under enabledPlugins. | Les sessions lancées dans le terminal d'Acta chargent toujours le plugin d'Acta. Pour l'éviter, mettez "acta@inline": false sous enabledPlugins. |
| `settings.snapshots.today` | {n} snapshot, from today, {time} · {n} snapshots, newest today, {time} | {n} instantané, d'aujourd'hui à {time} · {n} instantanés, le plus récent d'aujourd'hui à {time} |
| `settings.snapshots.older` | {n} snapshot, from {datetime} · {n} snapshots, newest {datetime} | {n} instantané, du {datetime} · {n} instantanés, le plus récent du {datetime} |
| `settings.setup.body` | Acta adds its plugin, two permissions and its status line to ~/.claude/settings.json. Your own status line still shows first. | Acta ajoute son plugin, deux autorisations et sa ligne d'état à ~/.claude/settings.json. Votre propre ligne d'état reste affichée en premier. |
| `settings.remove.body` | Removes Acta's plugin, permissions and status line from Claude Code, and turns its own memory back on. Your projects stay in Acta. | Retire de Claude Code le plugin, les autorisations et la ligne d'état d'Acta, et réactive sa propre mémoire. Vos projets restent dans Acta. |
| `chat.start.title` | Chat with Claude | Discuter avec Claude |
| `chat.start.body` | claude.ai in a panel, signed in with your own account. Acta never reads the page. | claude.ai dans un panneau, avec votre propre compte. Acta ne lit jamais la page. |
| `chat.start.memory` | Uses about 400 MB of memory once opened. | Utilise environ 400 Mo de mémoire une fois ouvert. |
| `chat.signin` | Sign in by email: open the link, then type the code it shows here. Google or Apple sign-in may be refused. | Connectez-vous par e-mail : ouvrez le lien, puis saisissez ici le code affiché. La connexion Google ou Apple peut être refusée. |
| `chat.signout.body` | Signs you out of claude.ai here. Your conversations stay in your Claude account. | Vous déconnecte de claude.ai ici. Vos conversations restent dans votre compte Claude. |
| `chat.runtime.body` | WebView2 is not installed, so claude.ai cannot show here. Everything else works. | WebView2 n'est pas installé : claude.ai ne peut pas s'afficher ici. Tout le reste fonctionne. |
| `browser.address` | Address | Adresse |
| `browser.idle.body` | Claude Code opens pages here: your dev server, or sites you signed into here. It never opens claude.ai and never types passwords. | Claude Code ouvre ses pages ici : votre serveur de développement, ou les sites auxquels vous vous êtes connecté ici. Il n'ouvre jamais claude.ai et ne saisit jamais de mot de passe. |
| `browser.idle.memory` | Uses about 150 MB of memory while in use. | Utilise environ 150 Mo de mémoire pendant son utilisation. |
| `browser.runtime.body` | WebView2 is not installed, so the browser cannot start. Everything else works. | WebView2 n'est pas installé : le navigateur ne peut pas démarrer. Tout le reste fonctionne. |
| `sessions.empty` | No Claude Code session is running. | Aucune session Claude Code en cours. |
| `export.warning` | The file is not encrypted: anyone who opens it can read these projects. | Le fichier n'est pas chiffré : quiconque l'ouvre peut lire ces projets. |
| `export.suggest` | Everything is on this computer only. An export is your copy if the keychain is lost. | Tout est sur cet ordinateur uniquement. Un export est votre copie si le trousseau est perdu. |
| `import.safe` | Nothing will be deleted. Skipped records are the same as Acta's, or older. | Rien ne sera supprimé. Les éléments ignorés sont identiques à ceux d'Acta, ou plus anciens. |
| `firstrun.body` | A project is a folder you work on with Claude Code. Acta keeps its topics, decisions and notes. | Un projet est un dossier sur lequel vous travaillez avec Claude Code. Acta en garde les sujets, les décisions et les notes. |
| `folders.also_topic` | Also attached to topic {name}. | Également rattaché au sujet {name}. |
| `folders.also_epic` | Also attached to epic {name}. | Également rattaché à l'epic {name}. |
| `folders.located_for` | {folder} located for epic {name} and everything under it. | {folder} localisé pour l'epic {name} et tout ce qu'il contient. |

**New**, placed after the keys of their family:

| Key | English | French |
|---|---|---|
| `folders.also.tip` | A session started in this folder outside Acta opens on {level} and is told about both. | Une session lancée dans ce dossier hors d'Acta s'ouvre sur : {level}, et est informée des deux. |
| `settings.status.ready` | Acta is set up in Claude Code | Acta est configuré dans Claude Code |
| `settings.status.partial` | Setup incomplete: {n} of 3 step done · Setup incomplete: {n} of 3 steps done | Configuration incomplète : {n} étape sur 3 faite · Configuration incomplète : {n} étapes sur 3 faites |
| `settings.status.unknown` | Could not check the setup | Impossible de vérifier la configuration |
| `settings.setup.what` | What setup changes | Ce que change la configuration |
| `settings.setup.details` | Setup installs Acta's plugin in Claude Code, adds two permission lines (Acta and its browser) and Acta's status line to ~/.claude/settings.json (after your own status line, if you have one), and turns Claude Code's own memory off in the open project's folders. Remove undoes all of it. | La configuration installe le plugin d'Acta dans Claude Code, ajoute deux lignes d'autorisation (Acta et son navigateur) et la ligne d'état d'Acta à ~/.claude/settings.json (après votre propre ligne d'état, si vous en avez une), et désactive la mémoire propre à Claude Code dans les dossiers du projet ouvert. Retirer annule tout cela. |
| `settings.chat_data` | Chat data | Données du chat |
| `settings.restore` | Restore... | Restaurer... |
| `prop.add_due` | Due date | Échéance |
| `prop.add_report_to` | Report to | À rapporter à |
| `meta.record` | {id} · Updated {when} by {author} | {id} · Modifié {when} par {author} |

`{level}` in `folders.also.tip` is a `level.*` string ("Epic Terminal", « Epic Terminal »), which is why the French follows a colon. `prop.add_due` and `prop.add_report_to` label the hover-revealed add chips of an unset optional property (components-records.md 5, Properties block). `meta.record` is the one meta line under the properties block that replaces the Id and Updated rows; `{id}` is in `type.code`. `settings.setup.details` (the former text of `settings.setup.hint`) is the body behind "What setup changes", followed by `settings.plugin.hint`. The Settings rows Snapshots, Trash and Keyboard shortcuts take `settings.restore`, `common.open` and `common.open` as their buttons, so a label is never repeated on its own button.

### 10.4.11 Keys of contacts and the profile

New with contacts (2026-11-22; components-contacts.md 5, data.md 11 requests 22 and 23). Moved into 10.4 with the keys of 10.4.6 (branch `gpui-engineer/questions-ui`): `settings.profile*` before `settings.theme`, `cmd.contact*` and `cmd.profile_*` after the `cmd.*` keys of questions, `trash.*` after the trash keys, the `contact.*` and `contacts.*` families together after `person.*`.

**Replaced** in 10.4.6 (not to be moved into 10.4): `person.placeholder` by `contact.placeholder`; `person.open_count` by `contact.open_count`; `person.counter` (no counter: the picker stops at 80); `person.scope` by `contact.scope` and `contact.scope_projects`; `person.go` by `contact.go`; `cmd.person_go` by `cmd.contact_go`. Kept: `person.none`, `person.section.asked`, `person.section.report`, `person.empty`, `count.person_asked`, `count.person_report`.

In French, *contact* is masculine ("ajouté", "supprimé", "fusionné"); a contact's name is user text and in French always follows a colon (10.3), so sentences put it last. `{level}` is `contact.level.all` or a `level.project` / `level.epic` string; `{record}` is a record title as written; `{places}` is a list of `contact.move.place`, joined by ", " as 10.3 licenses, at most three, then `contact.move.more`.

`contact.lifted.project` replaces `contact.lifted` when the contact is lifted to a project rather than to all projects. `contact.meta_updated` replaces `contact.meta` once the contact changed after it was added. The `{field}` of `contact.move.place` is `prop.report_to` or `question.field.asked`. Agent-facing text (`ok C-12 Paul (P-1)`, `refused: no contact "Mari" here`, `User: Alexandre, Product designer`) is protocol, English only, and not in this catalogue (10).

### 10.4.12 Keys of memory quality (ADR 0015)

New with memory quality (2026-11-22; components-knowledge.md 5, Memory quality; components-records.md 5, Overview; screens.md 8.31 to 8.33). Pending: to be moved into 10.4 and `catalogue.rs` when the UI is implemented (PLAN.md, Memory quality, phase 5): `overview.to_check` in the place of `overview.memories`, `filter.to_check` and `filter.inactive` in the place of `filter.stale_memories`, `cmd.show_to_check` and `cmd.show_inactive` in the place of `cmd.show_stale_memories`, the other `cmd.kb_*` keys after `cmd.kb_to_document`, `history.*` after `history.created.memory`, the `kb.*` keys after `kb.no_results`.

**Removed** from 10.4 when they move: `overview.memories`, `filter.stale_memories`, `cmd.show_stale_memories` (the 90-day count became "to check", which counts every reason the store computes and reads the verified date, not the last update).

**Reused**, not duplicated: Keep both is `contact.keep_both`; Move to epic / project in the notice are `kb.move_epic` and `kb.move_project`; Copied is `common.copied`; the stale-save notice is `conflict.notice`, `conflict.theirs`, `conflict.mine`; "Written by" is `kb.written.*`.

| Key | English | French |
|---|---|---|
| `overview.to_check` | {n} memory to check · {n} memories to check | {n} mémoire à vérifier · {n} mémoires à vérifier |
| `filter.to_check` | To check | À vérifier |
| `filter.inactive` | Superseded and retired | Remplacés et obsolètes |
| `kb.inactive_row` | {n} superseded or retired | {n} élément remplacé ou obsolète · {n} éléments remplacés ou obsolètes |
| `kb.to_check_empty` | Nothing to check. | Rien à vérifier. |
| `kb.inactive_empty` | Nothing is superseded or retired. | Aucun élément remplacé ou obsolète. |
| `kb.reason.conflict` | May contradict {id} | Contredit peut-être {id} |
| `kb.reason.duplicate` | Possible duplicate of {id} | Doublon possible de {id} |
| `kb.reason.file_gone` | {file} was deleted or moved | {file} a été supprimé ou déplacé |
| `kb.reason.file_gone_more` | {file} and {n} more file were deleted or moved · {file} and {n} more files were deleted or moved | {file} et {n} autre fichier ont été supprimés ou déplacés · {file} et {n} autres fichiers ont été supprimés ou déplacés |
| `kb.reason.file_changed` | {file} changed since it was checked | {file} a changé depuis la dernière vérification |
| `kb.reason.file_changed_more` | {file} and {n} more file changed since they were checked · {file} and {n} more files changed since they were checked | {file} et {n} autre fichier ont changé depuis la dernière vérification · {file} et {n} autres fichiers ont changé depuis la dernière vérification |
| `kb.reason.branch_gone` | Branch {branch} no longer exists | La branche {branch} n'existe plus |
| `kb.reason.branch_merged` | Branch {branch} was merged | La branche {branch} a été fusionnée |
| `kb.reason.folder_removed` | Its folder was removed from Acta | Son dossier a été retiré d'Acta |
| `kb.reason.topic_done` | Its topic is done | Son sujet est terminé |
| `kb.reason.topic_dropped` | Its topic was dropped | Son sujet a été abandonné |
| `kb.reason.old` | Not checked for {days} days | Non vérifiée depuis {days} jours |
| `kb.verify` | Mark as still true | Marquer comme toujours valable |
| `kb.retire` | Mark as no longer true | Marquer comme obsolète |
| `kb.retired_toast.document` | Marked as no longer true. | Marqué comme obsolète. |
| `kb.retired_toast.memory` | Marked as no longer true. | Marquée comme obsolète. |
| `kb.make_live` | Make live again | Rétablir |
| `kb.notice.superseded.document` | Superseded by {title} on {date}. | Remplacé le {date} par {title}. |
| `kb.notice.superseded.memory` | Superseded by {title} on {date}. | Remplacée le {date} par {title}. |
| `kb.notice.retired.document` | Retired by {author} on {date}. | Rendu obsolète le {date} par {author}. |
| `kb.notice.retired.memory` | Retired by {author} on {date}. | Rendue obsolète le {date} par {author}. |
| `kb.notice.conflict` | May contradict {id}. | Contredit peut-être {id}. |
| `kb.notice.duplicate` | Possible duplicate of {id}. | Doublon possible de {id}. |
| `kb.this_replaces` | This replaces it | Celle-ci la remplace |
| `kb.replaced_toast` | {id} is now superseded by this. | {id} est désormais remplacée par celle-ci. |
| `kb.merge_into_id` | Merge into {id} | Fusionner dans {id} |
| `kb.merge_here` | Merge {id} into this | Fusionner {id} dans celle-ci |
| `kb.merge_into` | Merge into... | Fusionner dans... |
| `kb.merged` | Merged into {id}. | Fusionnée dans {id}. |
| `kb.prop.branch` | Branch | Branche |
| `kb.prop.sources` | Sources | Sources |
| `kb.branch.merged` | merged | fusionnée |
| `kb.branch.gone` | no longer exists | n'existe plus |
| `kb.branch.tip` | True only on this branch, in {folder}. | Valable seulement sur cette branche, dans {folder}. |
| `kb.source.entry` | {topic}, entry of {date} | {topic}, entrée du {date} |
| `kb.source.missing` | A record not in this database | Un élément absent de cette base |
| `kb.source.commit` | Commit {sha} | Commit {sha} |
| `kb.source.copy_commit` | Copy commit id | Copier l'identifiant du commit |
| `kb.source.copy_path` | {folder} · Copy path | {folder} · Copier le chemin |
| `kb.source.changed` | changed since checked | modifié depuis la vérification |
| `kb.source.gone` | deleted or moved | supprimé ou déplacé |
| `kb.prov.from` | From topic {id} | Depuis le sujet {id} |
| `kb.prov.branch` | On branch {branch} | Sur la branche {branch} |
| `kb.prov.checked.document` | Checked {when} by {author} | Vérifié {when} par {author} |
| `kb.prov.checked.memory` | Checked {when} by {author} | Vérifiée {when} par {author} |
| `kb.session.on` | A Claude Code session on {topic} | Une session Claude Code sur {topic} |
| `kb.session.any` | A Claude Code session | Une session Claude Code |
| `kb.session.branch` | Branch {branch} | Branche {branch} |
| `kb.also_open.memory` | Claude Code is working with this memory on {topic} | Claude Code travaille avec cette mémoire sur {topic} |
| `kb.also_open.document` | Claude Code is working with this document on {topic} | Claude Code travaille avec ce document sur {topic} |
| `kb.also_open.memory_any` | Claude Code is working with this memory | Claude Code travaille avec cette mémoire |
| `kb.also_open.document_any` | Claude Code is working with this document | Claude Code travaille avec ce document |
| `kb.also_open.read` | Last read {when} | Dernière lecture {when} |
| `kb.also_open.wrote` | Last changed {when} | Dernière modification {when} |
| `kb.save_merged` | Saved. Claude Code's changes to other sections are kept. | Enregistré. Les modifications de Claude Code dans les autres sections sont conservées. |
| `kb.scan_found` | Found {n} possible duplicate. · Found {n} possible duplicates. | {n} doublon possible trouvé. · {n} doublons possibles trouvés. |
| `kb.scan_show` | Show | Afficher |
| `kb.scan_none` | No possible duplicates found. | Aucun doublon possible trouvé. |
| `history.verified.document` | Marked as still true | Marqué comme toujours valable |
| `history.verified.memory` | Marked as still true | Marquée comme toujours valable |
| `history.superseded.document` | Superseded by {id} | Remplacé par {id} |
| `history.superseded.memory` | Superseded by {id} | Remplacée par {id} |
| `history.retired.document` | Retired | Rendu obsolète |
| `history.retired.memory` | Retired | Rendue obsolète |
| `history.retired_reason.document` | Retired: {reason} | Rendu obsolète : {reason} |
| `history.retired_reason.memory` | Retired: {reason} | Rendue obsolète : {reason} |
| `history.made_live.document` | Made live again | Rétabli |
| `history.made_live.memory` | Made live again | Rétablie |
| `history.merged` | Merged {id} into this | Fusion de {id} |
| `cmd.show_to_check` | Show knowledge to check | Afficher les connaissances à vérifier |
| `cmd.show_inactive` | Show superseded and retired | Afficher les éléments remplacés et obsolètes |
| `cmd.kb_verify` | Mark as still true | Marquer comme toujours valable |
| `cmd.kb_retire` | Mark as no longer true | Marquer comme obsolète |
| `cmd.kb_make_live` | Make live again | Rétablir |
| `cmd.kb_merge` | Merge into... | Fusionner dans... |
| `cmd.kb_keep_both` | Keep both {a} and {b} | Garder les deux : {a} et {b} |
| `cmd.kb_replace` | Replace {id} with this | Remplacer {id} par celle-ci |
| `cmd.kb_find_duplicates` | Find duplicate memories | Trouver les mémoires en double |

Notes. *Mémoire* is feminine and *document* masculine, hence the `.document` / `.memory` pairs wherever a participle agrees; conflicts, duplicates and merges concern memories only, so their strings are written in the feminine ("Celle-ci la remplace", "Fusionnée dans"). *Remplacés et obsolètes* and *élément* agree with a mixed set of documents and memories (masculine plural, as `import.safe` already uses *éléments*). "Retired" is the English state word and "Mark as no longer true" the act that produces it; French uses *obsolète* for both, since a literal "retirée" collides with *Retirer* (remove a folder). "Still true" is *toujours valable* rather than *vraie* so that one form fits both kinds. `{id}` is a knowledge id (`K-4`) in `type.code` as the meta line shows it, never translated; `{title}` and `{reason}` are what Claude Code or the human wrote, as written; `{file}` is a file name (the last path segment) and `{branch}` a branch as git has it, both in `type.code`; `{folder}` a working folder's name; `{topic}` a topic id (`T-12`) with its title as the tooltip; `{date}` the absolute date and `{when}` the long form of 10.2; `{days}` is always 90 or more, so the singular form is not needed. The reason lines have no final full stop (they are tooltip and notice lines, one per reason); the notice messages that are sentences do. Agent-facing text (`?old 140d`, `?dup K-4`, `ok K-4 verified (rev 3)`, `K-9 superseded by K-21 on 11-20`) is protocol, English only, and not in this catalogue (10).
