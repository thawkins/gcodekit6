#!/usr/bin/env python3
"""Build the GitHub release notes for one version from CHANGELOG.md.

Used by .github/workflows/release.yml. A release's notes are every
CHANGELOG section whose heading names its version (`## [2.0.0-alpha.1] - ...`;
a version can have several sections, one per milestone). Versions below it
that were never published as a GitHub release are folded in too, so a release
that follows an unpublished one still carries the full set of changes. The
walk stops at the first older version that was published, or at the first
`## [Unreleased]` heading below the version (history from before releases
were cut from this CHANGELOG).

Usage:
    release_notes.py VERSION [--changelog CHANGELOG.md] [--out notes.md]
                     [--published TAG,TAG,...] [--repo owner/name]

VERSION is the version without the leading `v`. `--published` lists the tags
that already have a published release (the current one is ignored). Exits 1
when CHANGELOG.md has no section for VERSION, so a tag is never published
without notes.
"""

import argparse
import re
import sys

HEADING = re.compile(r"^## \[(?P<ver>[^\]]+)\](?P<rest>.*)$", re.M)
# GitHub rejects release bodies longer than 125000 characters.
MAX_BODY = 120_000


def split_sections(text):
    """Return the CHANGELOG as a list of (version, heading line, body)."""
    matches = list(HEADING.finditer(text))
    sections = []
    for i, m in enumerate(matches):
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        body = text[m.end():end].strip("\n")
        sections.append((m.group("ver"), m.group(0).strip(), body))
    return sections


def select(sections, version, published):
    """Pick the sections for `version` plus older unpublished versions."""
    start = next((i for i, s in enumerate(sections) if s[0] == version), None)
    if start is None:
        return []
    chosen = []
    for ver, heading, body in sections[start:]:
        if ver == version:
            chosen.append((ver, heading, body))
        elif ver == "Unreleased" or ver in published:
            break
        else:
            chosen.append((ver, heading, body))
    return chosen


def render(version, chosen, repo):
    """Render the release body in Markdown."""
    tag = f"v{version}"
    out = [f"# GCodeKit6 {version}", ""]
    if "-" in version:
        out += [
            "This is a pre-release. It has not been tested on every "
            "platform or on real machines yet.",
            "",
        ]
    folded = sorted({v for v, _, _ in chosen if v != version})
    if folded:
        out += [
            "These notes also cover "
            + ", ".join(folded)
            + ", which had no published release.",
            "",
        ]
    for _, heading, body in chosen:
        out += [heading, "", body, ""]
    out += [
        "## Downloads",
        "",
        "| Platform | File |",
        "|---|---|",
        "| Linux (Debian, Ubuntu) | `.deb` |",
        "| Linux (Fedora, openSUSE) | `.rpm` |",
        "| Linux (any distribution) | `.flatpak`, `.AppImage`, or the bare binary |",
        "| macOS (Apple Silicon) | `.dmg` |",
        "| Windows (64-bit) | `.msi` installer, or `-portable.zip` |",
        "",
    ]
    if repo:
        out += [
            f"Full history: https://github.com/{repo}/blob/{tag}/CHANGELOG.md",
            "",
        ]
    body = "\n".join(out)
    if len(body) > MAX_BODY:
        cut = body.rfind("\n## ", 0, MAX_BODY - 400)
        body = body[: cut if cut > 0 else MAX_BODY - 400]
        body += "\n\n(Notes shortened to fit GitHub's limit; see CHANGELOG.md.)\n"
    return body


def main():
    """Parse arguments, write the notes, and exit 1 when none are found."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("version")
    ap.add_argument("--changelog", default="CHANGELOG.md")
    ap.add_argument("--out", default="release_notes.md")
    ap.add_argument("--published", default="")
    ap.add_argument("--repo", default="")
    args = ap.parse_args()

    version = args.version.removeprefix("v")
    published = {
        t.strip().removeprefix("v")
        for t in args.published.split(",")
        if t.strip() and t.strip().removeprefix("v") != version
    }
    with open(args.changelog, encoding="utf-8") as f:
        sections = split_sections(f.read())
    chosen = select(sections, version, published)
    if not chosen:
        print(f"CHANGELOG.md has no '## [{version}]' section", file=sys.stderr)
        return 1
    with open(args.out, "w", encoding="utf-8") as f:
        f.write(render(version, chosen, args.repo))
    print(
        f"Release notes for {version}: {len(chosen)} section(s) -> {args.out}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
