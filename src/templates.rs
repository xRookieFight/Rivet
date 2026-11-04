//! Starter content for README, community health files, issue/PR templates and
//! config files. `{name}`, `{owner}` and `{description}` get filled in per repo.

pub struct Template {
    pub name: &'static str,
    pub body: &'static str,
}

pub fn fill(body: &str, owner: &str, name: &str, description: &str, branch: &str) -> String {
    body.replace("{owner}", owner)
        .replace("{name}", name)
        .replace("{description}", if description.is_empty() { "A short description of the project." } else { description })
        .replace("{branch}", branch)
        .replace("{year}", &chrono::Utc::now().format("%Y").to_string())
}

pub const README: &[Template] = &[
    Template {
        name: "Standard",
        body: r#"# {name}

{description}

## Installation

```sh
git clone https://github.com/{owner}/{name}.git
cd {name}
```

## Usage

Describe how to use the project here.

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

See [LICENSE](LICENSE).
"#,
    },
    Template {
        name: "Library",
        body: r#"# {name}

{description}

## Features

- Fast
- Small API surface
- Well tested

## Getting started

Add the dependency to your project, then:

```rust
// example usage
```

## Documentation

API docs live in the source and are published with each release.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Distributed under the terms in [LICENSE](LICENSE).
"#,
    },
    Template {
        name: "Application",
        body: r#"<div align="center">

# {name}

{description}

</div>

## Screenshots

_Add a screenshot of the app here._

## Download

Grab the latest build from the [releases page](https://github.com/{owner}/{name}/releases).

## Building from source

```sh
git clone https://github.com/{owner}/{name}.git
cd {name}
```

## Roadmap

- [ ] First milestone
- [ ] Second milestone

## License

See [LICENSE](LICENSE).
"#,
    },
    Template { name: "Minimal", body: "# {name}\n\n{description}\n" },
];

/// Badge snippets; shields.io renders them on github.com.
pub const BADGES: &[(&str, &str)] = &[
    ("License", "[![License](https://img.shields.io/github/license/{owner}/{name})](LICENSE)"),
    ("Stars", "[![Stars](https://img.shields.io/github/stars/{owner}/{name}?style=social)](https://github.com/{owner}/{name}/stargazers)"),
    (
        "Last commit",
        "[![Last commit](https://img.shields.io/github/last-commit/{owner}/{name})](https://github.com/{owner}/{name}/commits)",
    ),
    ("Open issues", "[![Issues](https://img.shields.io/github/issues/{owner}/{name})](https://github.com/{owner}/{name}/issues)"),
    ("Latest release", "[![Release](https://img.shields.io/github/v/release/{owner}/{name})](https://github.com/{owner}/{name}/releases)"),
    (
        "CI",
        "[![CI](https://github.com/{owner}/{name}/actions/workflows/ci.yml/badge.svg)](https://github.com/{owner}/{name}/actions/workflows/ci.yml)",
    ),
    ("crates.io", "[![crates.io](https://img.shields.io/crates/v/{name})](https://crates.io/crates/{name})"),
    ("npm", "[![npm](https://img.shields.io/npm/v/{name})](https://www.npmjs.com/package/{name})"),
];

pub const CONTRIBUTING: &str = r#"# Contributing to {name}

Thanks for taking the time to contribute!

## Reporting bugs

Open an issue using the bug report template. Include steps to reproduce, what you expected and what happened instead.

## Suggesting changes

Open an issue describing the change before starting large pieces of work, so we can agree on the approach.

## Pull requests

1. Fork the repository and create a branch from `{branch}`.
2. Make your change and add tests where it makes sense.
3. Make sure the project builds and the test suite passes.
4. Open a pull request and fill in the template.

## Code style

Follow the existing style of the code base. Keep commits focused on one change.
"#;

pub const CODE_OF_CONDUCT: &str = r#"# Code of Conduct

## Our pledge

We as members, contributors, and leaders pledge to make participation in our community a harassment-free experience for everyone, regardless of age, body size, visible or invisible disability, ethnicity, sex characteristics, gender identity and expression, level of experience, education, socio-economic status, nationality, personal appearance, race, religion, or sexual identity and orientation.

## Our standards

Examples of behavior that contributes to a positive environment:

* Demonstrating empathy and kindness toward other people
* Being respectful of differing opinions, viewpoints, and experiences
* Giving and gracefully accepting constructive feedback
* Accepting responsibility and apologizing to those affected by our mistakes

Examples of unacceptable behavior:

* The use of sexualized language or imagery, and sexual attention or advances of any kind
* Trolling, insulting or derogatory comments, and personal or political attacks
* Public or private harassment
* Publishing others' private information without their explicit permission

## Enforcement

Instances of abusive, harassing, or otherwise unacceptable behavior may be reported to the project maintainers. All complaints will be reviewed and investigated promptly and fairly.

## Attribution

This Code of Conduct is adapted from the [Contributor Covenant](https://www.contributor-covenant.org), version 2.1.
"#;

pub const SECURITY: &str = r#"# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| latest  | ✅        |

## Reporting a vulnerability

Please do not report security vulnerabilities through public issues.

Use [GitHub private vulnerability reporting](https://github.com/{owner}/{name}/security/advisories/new) instead. You should receive a response within a few days. If the issue is confirmed, a fix will be released as soon as possible.
"#;

pub const FUNDING: &str = r#"# These are supported funding model platforms

github: [{owner}]
# patreon:
# open_collective:
# ko_fi:
# liberapay:
# custom: ["https://example.com/donate"]
"#;

pub const EDITORCONFIG: &str = r#"root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 4

[*.{yml,yaml,json,md}]
indent_size = 2

[Makefile]
indent_style = tab
"#;

pub const DEPENDABOT: &str = r#"version: 2
updates:
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
"#;

pub const CODEOWNERS: &str = r#"# Each line is a file pattern followed by one or more owners.
# The last matching pattern takes precedence.

*       @{owner}
/docs/  @{owner}
"#;

pub const WORKFLOWS: &[Template] = &[
    Template {
        name: "Rust CI",
        body: r#"name: CI

on:
  push:
    branches: [ "{branch}" ]
  pull_request:
    branches: [ "{branch}" ]

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build
        run: cargo build --verbose
      - name: Test
        run: cargo test --verbose
"#,
    },
    Template {
        name: "Node.js CI",
        body: r#"name: Node.js CI

on:
  push:
    branches: [ "{branch}" ]
  pull_request:
    branches: [ "{branch}" ]

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        node-version: [20.x, 22.x]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm
      - run: npm ci
      - run: npm test
"#,
    },
    Template {
        name: "Python",
        body: r#"name: Python

on:
  push:
    branches: [ "{branch}" ]
  pull_request:
    branches: [ "{branch}" ]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - run: pip install -r requirements.txt
      - run: python -m pytest
"#,
    },
    Template {
        name: "Manual (workflow_dispatch)",
        body: r#"name: Manual run

on:
  workflow_dispatch:
    inputs:
      message:
        description: "Message to print"
        required: true
        default: "Hello from Rivet"

jobs:
  run:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ inputs.message }}"
"#,
    },
];

pub const ISSUE_TEMPLATES: &[Template] = &[
    Template {
        name: "Bug report (form)",
        body: r#"name: Bug report
description: Report something that isn't working
title: "[Bug]: "
labels: ["bug"]
body:
  - type: markdown
    attributes:
      value: Thanks for taking the time to fill out this bug report!
  - type: textarea
    id: what-happened
    attributes:
      label: What happened?
      description: Also tell us what you expected to happen.
    validations:
      required: true
  - type: textarea
    id: steps
    attributes:
      label: Steps to reproduce
      placeholder: |
        1. Go to '...'
        2. Click on '...'
        3. See error
    validations:
      required: true
  - type: input
    id: version
    attributes:
      label: Version
      placeholder: e.g. 1.2.0
  - type: dropdown
    id: os
    attributes:
      label: Operating system
      options:
        - Windows
        - macOS
        - Linux
  - type: textarea
    id: logs
    attributes:
      label: Relevant log output
      render: shell
"#,
    },
    Template {
        name: "Feature request (form)",
        body: r#"name: Feature request
description: Suggest an idea for this project
labels: ["enhancement"]
body:
  - type: textarea
    id: problem
    attributes:
      label: Is your feature request related to a problem?
      description: A clear description of what the problem is.
  - type: textarea
    id: solution
    attributes:
      label: Describe the solution you'd like
    validations:
      required: true
  - type: textarea
    id: alternatives
    attributes:
      label: Alternatives you've considered
"#,
    },
    Template {
        name: "Bug report (markdown)",
        body: r#"---
name: Bug report
about: Create a report to help us improve
title: ''
labels: bug
assignees: ''
---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
1. Go to '...'
2. See error

**Expected behavior**
What you expected to happen.

**Environment**
 - OS:
 - Version:
"#,
    },
    Template {
        name: "Template chooser config",
        body: r#"blank_issues_enabled: false
contact_links:
  - name: Questions
    url: https://github.com/{owner}/{name}/discussions
    about: Ask and answer questions here.
"#,
    },
];

pub const PR_TEMPLATES: &[Template] = &[
    Template {
        name: "Default",
        body: r#"## Summary

What does this change and why?

## Changes

-

## Testing

How was this tested?

## Checklist

- [ ] Tests added or updated
- [ ] Docs updated
"#,
    },
    Template {
        name: "Bug fix",
        body: r#"## Bug

Fixes #

## Root cause

## Fix

## Verification
"#,
    },
    Template {
        name: "Release",
        body: r#"## Release

Version:

### Highlights

-

### Checklist

- [ ] Changelog updated
- [ ] Version bumped
"#,
    },
];

/// Fields of a YAML issue form, enough to render a preview of it.
#[derive(Debug, Clone, Default)]
pub struct IssueForm {
    pub name: String,
    pub description: String,
    pub title: String,
    pub labels: Vec<String>,
    pub fields: Vec<FormField>,
}

#[derive(Debug, Clone, Default)]
pub struct FormField {
    pub kind: String,
    // only the tests look at ids today; github uses them for prefilled urls
    #[allow(dead_code)]
    pub id: String,
    pub label: String,
    pub description: String,
    pub placeholder: String,
    pub value: String,
    pub options: Vec<String>,
    pub required: bool,
    pub render: Option<String>,
}

/// Parses a GitHub issue form. Errors are shown to the user as-is.
pub fn parse_issue_form(src: &str) -> Result<IssueForm, String> {
    use serde_yaml_ng::Value;
    let v: Value = serde_yaml_ng::from_str(src).map_err(|e| e.to_string())?;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut form = IssueForm {
        name: s(&v, "name"),
        description: s(&v, "description"),
        title: s(&v, "title"),
        labels: match v.get("labels") {
            Some(Value::Sequence(seq)) => seq.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
            Some(Value::String(one)) => one.split(',').map(|x| x.trim().to_string()).collect(),
            _ => vec![],
        },
        fields: vec![],
    };
    if form.name.is_empty() {
        return Err("issue forms need a top level `name`".into());
    }
    let body = v.get("body").and_then(Value::as_sequence).ok_or("issue forms need a `body` list")?;
    for item in body {
        let attrs = item.get("attributes").cloned().unwrap_or(Value::Null);
        let options = match attrs.get("options") {
            Some(Value::Sequence(seq)) => seq
                .iter()
                .filter_map(|o| o.as_str().map(str::to_string).or_else(|| o.get("label").and_then(Value::as_str).map(str::to_string)))
                .collect(),
            _ => vec![],
        };
        form.fields.push(FormField {
            kind: s(item, "type"),
            id: s(item, "id"),
            label: s(&attrs, "label"),
            description: s(&attrs, "description"),
            placeholder: s(&attrs, "placeholder"),
            value: s(&attrs, "value"),
            options,
            required: item.get("validations").and_then(|x| x.get("required")).and_then(Value::as_bool).unwrap_or(false),
            render: attrs.get("render").and_then(Value::as_str).map(str::to_string),
        });
    }
    Ok(form)
}

/// Markdown issue templates carry YAML front matter; split it from the body.
pub fn split_front_matter(src: &str) -> (Option<String>, String) {
    let trimmed = src.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let fm = rest[..end].trim().to_string();
        let body = rest[end + 4..].trim_start_matches(['\r', '\n']).to_string();
        return (Some(fm), body);
    }
    (None, src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_issue_forms_parse() {
        for t in ISSUE_TEMPLATES.iter().filter(|t| t.name.contains("(form)")) {
            let f = parse_issue_form(t.body).unwrap_or_else(|e| panic!("{}: {e}", t.name));
            assert!(!f.fields.is_empty());
        }
    }

    #[test]
    fn form_reads_options_and_required() {
        let f = parse_issue_form(ISSUE_TEMPLATES[0].body).unwrap();
        let os = f.fields.iter().find(|x| x.id == "os").unwrap();
        assert_eq!(os.options, vec!["Windows", "macOS", "Linux"]);
        assert!(f.fields.iter().find(|x| x.id == "steps").unwrap().required);
        assert_eq!(f.labels, vec!["bug"]);
    }

    #[test]
    fn front_matter_split() {
        let (fm, body) = split_front_matter("---\nname: Bug\n---\n\nHello");
        assert_eq!(fm.as_deref(), Some("name: Bug"));
        assert_eq!(body, "Hello");
        let (fm, body) = split_front_matter("no front matter");
        assert!(fm.is_none());
        assert_eq!(body, "no front matter");
    }

    #[test]
    fn fill_replaces_placeholders() {
        let s = fill("{owner}/{name} on {branch}", "a", "b", "", "main");
        assert_eq!(s, "a/b on main");
    }
}
