#!/usr/bin/env python3
"""
Extract the latest release from CHANGELOG.xml and format as Markdown.
"""

import xml.etree.ElementTree as ET
import sys
from pathlib import Path


def extract_latest_release(changelog_path: Path) -> tuple[str, str]:
    """
    Parse CHANGELOG.xml and extract the latest release.

    Returns:
        (version, markdown_content)
    """
    tree = ET.parse(changelog_path)
    root = tree.getroot()

    # Get the first release element
    release = root.find("./releases/release")

    if release is None:
        print("Error: No releases found in CHANGELOG.xml", file=sys.stderr)
        sys.exit(1)

    version = release.get("version")
    date = release.get("date")

    if version is None:
        print("Error: Release missing version attribute", file=sys.stderr)
        sys.exit(1)

    if date is None:
        print("Error: Release missing data attribute", file=sys.stderr)
        sys.exit(1)

    # Build markdown content
    lines: list[str] = []

    lines.append(f"# hashcards {version}")
    lines.append("")
    lines.append(f"Date: {date}")
    lines.append("")

    # Category mapping to nice headers
    categories = {
        "added": "## Added",
        "fixed": "## Fixed",
        "changed": "## Changed",
        "removed": "## Removed",
        "deprecated": "## Deprecated",
        "security": "## Security",
        "breaking": "## Breaking Changes",
    }

    for category_tag, header in categories.items():
        category = release.find(category_tag)
        if category is not None:
            changes = category.findall("change")
            if changes:
                lines.append(header)
                lines.append("")
                for change in changes:
                    text = change.text.strip() if change.text else ""
                    author = change.get("author")
                    if author:
                        lines.append(f"- {text} (@{author})")
                    else:
                        lines.append(f"- {text}")
                lines.append("")

    # Remove trailing empty line
    if lines and lines[-1] == "":
        _ = lines.pop()

    markdown = "\n".join(lines)

    return version, markdown


if __name__ == "__main__":
    changelog_path = Path("CHANGELOG.xml")

    if not changelog_path.exists():
        print(f"Error: {changelog_path} not found", file=sys.stderr)
        sys.exit(1)

    version, markdown = extract_latest_release(changelog_path)

    # Output for GitHub Actions
    print(f"VERSION={version}")
    print("CHANGELOG<<EOF")
    print(markdown)
    print("EOF")
