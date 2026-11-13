# Security Policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems.

Report them privately through GitHub:
**[Report a vulnerability](https://github.com/MiloshMilinkov/KeepItInCheck/security/advisories/new)**

If that isn't possible, email milos300399@gmail.com with "SECURITY" in the subject.

Please include:

- what the problem is and where (file, command or feature);
- steps to reproduce it;
- the impact you think it has.

You'll get a reply as soon as possible. Please give a reasonable time to fix the issue before disclosing it anywhere.

## Areas of particular interest

KeepItInCheck can terminate processes, so the most important class of bug is anything that lets it
close something it should not: a protected process slipping through the classifier, PID reuse,
the force-kill path reaching a windowed app, or the relaunch command starting an arbitrary path.

## Scope

Only the code in this repository and the installers built from it are in scope.
