#!/usr/bin/env python3
"""Generate manifest.json for shader bundle.

Usage:
    python scripts/generate_manifest.py shaders/

This scans the shaders directory and generates a manifest.json
with SHA256 hashes for all files.
"""

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path


def get_file_type(path: Path) -> str:
    """Determine file type from path."""
    name = path.name.lower()
    suffix = path.suffix.lower()

    if suffix == ".glsl":
        if name.startswith("cursor_"):
            return "cursor_shader"
        return "shader"
    elif suffix in (".png", ".jpg", ".jpeg", ".webp", ".gif"):
        return "texture"
    elif suffix in (".md", ".txt", ".rst"):
        return "doc"
    else:
        return "other"


def get_category(path: Path, file_type: str) -> str | None:
    """Determine category from file path/name."""
    name = path.stem.lower()

    if file_type == "cursor_shader":
        return "cursor"
    elif file_type == "texture":
        return "texture"

    # Categorize background shaders by name patterns
    retro_keywords = ["crt", "scanline", "vhs", "retro", "8bit", "pixel"]
    space_keywords = ["star", "galaxy", "nebula", "space", "cosmic"]
    nature_keywords = ["fire", "water", "cloud", "rain", "snow", "ocean", "wave"]
    abstract_keywords = ["plasma", "fractal", "noise", "pattern", "warp"]
    matrix_keywords = ["matrix", "digital", "cyber", "code"]

    for kw in retro_keywords:
        if kw in name:
            return "retro"
    for kw in space_keywords:
        if kw in name:
            return "space"
    for kw in nature_keywords:
        if kw in name:
            return "nature"
    for kw in matrix_keywords:
        if kw in name:
            return "matrix"
    for kw in abstract_keywords:
        if kw in name:
            return "abstract"

    return "effects"


def compute_sha256(path: Path) -> str:
    """Compute SHA256 hash of file."""
    hasher = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main() -> None:
    """Generate manifest.json for shader bundle."""
    if len(sys.argv) < 2:
        print("Usage: python scripts/generate_manifest.py <shaders_dir>")
        sys.exit(1)

    shaders_dir = Path(sys.argv[1])
    if not shaders_dir.is_dir():
        print(f"Error: {shaders_dir} is not a directory")
        sys.exit(1)

    # Get version from Cargo.toml
    cargo_toml = Path("Cargo.toml")
    version = "0.0.0"
    if cargo_toml.exists():
        for line in cargo_toml.read_text(encoding="utf-8").splitlines():
            if line.startswith("version = "):
                version = line.split('"')[1]
                break

    files = []
    for path in sorted(shaders_dir.rglob("*")):
        if path.is_file() and path.name != "manifest.json" and not path.name.startswith("."):
            relative = path.relative_to(shaders_dir)
            file_type = get_file_type(path)
            category = get_category(path, file_type)

            entry = {
                "path": str(relative),
                "sha256": compute_sha256(path),
                "type": file_type,
            }
            if category:
                entry["category"] = category

            files.append(entry)

    manifest = {
        "version": version,
        "generated": datetime.now(timezone.utc).isoformat(),
        "files": files,
    }

    output_path = shaders_dir / "manifest.json"
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    print(f"Generated {output_path} with {len(files)} files")


if __name__ == "__main__":
    main()
