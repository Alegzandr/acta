# macOS bundle resources

`scripts/package.sh` copies each `<lang>.lproj/InfoPlist.strings` into `Acta.app/Contents/Resources/`. They translate the permission prompts declared in `Info.plist` (ADR 0014, 8): the English text there must equal `en.lproj`, which the packaging script checks. UTF-8, checked with `plutil -lint` when packaging.
