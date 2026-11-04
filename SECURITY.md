# Security policy

## Supported versions

Only the latest release gets security fixes. Please update before reporting.

## Reporting a vulnerability

Please don't open a public issue. Report it privately through [GitHub's private vulnerability reporting](https://github.com/xRookieFight/Rivet/security/advisories/new) instead.

Include what you found, how to reproduce it, and the Rivet version and operating system. You should get an answer within a week. Once a fix is released, the advisory is published with credit to you, unless you'd rather stay anonymous.

## Scope

Rivet holds GitHub tokens with broad scopes, so these matter most:

* Tokens leaving the OS keychain (Keychain, Credential Manager, Secret Service, Android Keystore), for example ending up in logs, the preferences file or screenshots.
* Requests that carry a token to a host other than GitHub.
* Content from GitHub (Markdown, file contents, workflow logs) that can trigger actions the user didn't ask for.
* TLS verification being bypassed or weakened.

Vulnerabilities in GitHub itself belong to [GitHub's bug bounty program](https://bounty.github.com/).
