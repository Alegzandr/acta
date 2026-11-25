---
title: Encrypted storage
status: accepted
owner: Alexandre
---

# Encrypted storage

SQLCipher encrypts every page[^pages], and the key never touches the disk[^key].
A second mention of pages[^pages] keeps its number.

Setext heading
--------------

Entities: &copy; &#8212; &amp; and an escape \*not emphasis\*.

[^key]: It lives in the OS keychain, read once at startup.
[^pages]: 4 KiB pages, HMAC-SHA512 per page.
    Continued on a second line.
[^unused]: Never cited, still listed.
