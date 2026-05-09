#!/usr/bin/env python
"""
Poster image upscaler for LaTeX projects.

Run from latex/ directory:
    python poster_upscale_used_images.py

Options:
    --dry-run              Scan only, no changes
    --model NAME           Real-ESRGAN model (default: realesrgan-x4plus)
    --no-lanczos-fallback  Skip if Real-ESRGAN not installed
    --all-figures          Also upscale images outside poster/images/ (e.g. figures/)
"""
from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple

# --------------------------------------------------------------------------- #
# Constants
# --------------------------------------------------------------------------- #

RASTER_EXTS = {".png", ".jpg", ".jpeg", ".webp"}
VECTOR_EXTS = {".pdf", ".svg", ".eps"}
ALL_IMAGE_EXTS = RASTER_EXTS | VECTOR_EXTS

INCLUDE_RE = re.compile(
    r"(?P<cmd>\\includegraphics\s*(?:\[[^\]]*\]\s*)?\{)"
    r"(?P<path>[^}]+)"
    r"(?P<end>\})",
    flags=re.MULTILINE,
)

# Stems that hint the image is a large text-heavy panel -> prefer 8x upscale
TEXT_HEAVY_8X_HINTS = {
    "abstract", "abstarct", "background", "architecture", "architec",
    "flow", "process", "story", "panel", "explanation", "method",
    "methodology", "dynamic", "batch", "costmodel", "cost_model",
    "pipeline", "system", "overview", "objective",
}


# --------------------------------------------------------------------------- #
# Data classes
# --------------------------------------------------------------------------- #

@dataclass
class ImageUse:
    raw_path: str
    resolved: Optional[Path]
    tex_files: List[Path] = field(default_factory=list)
    skip_reason: str = ""


@dataclass
class UpscaleResult:
    src: Path
    dst: Optional[Path]
    scale: Optional[str]
    status: str          # "upscaled" | "failed" | "skipped"
    note: str = ""


# --------------------------------------------------------------------------- #
# Utilities
# --------------------------------------------------------------------------- #

def run_cmd(cmd: List[str], cwd: Optional[Path] = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )


def which(name: str) -> Optional[str]:
    return shutil.which(name)


def rel_posix(path: Path, base: Path) -> str:
    try:
        return os.path.relpath(path, base).replace(os.sep, "/")
    except ValueError:
        # Different drives on Windows
        return str(path)


def is_under(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except Exception:
        return False


def fmt_size(n_bytes: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n_bytes < 1024:
            return f"{n_bytes:.0f} {unit}"
        n_bytes //= 1024
    return f"{n_bytes} GB"


# --------------------------------------------------------------------------- #
# Discovery
# --------------------------------------------------------------------------- #

def find_tex_files(root: Path) -> List[Path]:
    ignored = {".git", "node_modules", "venv", ".venv", "__pycache__", "upscaled"}
    files = []
    for p in root.rglob("*.tex"):
        if any(part in ignored for part in p.parts):
            continue
        files.append(p)
    return sorted(files)


def candidate_paths(raw: str, tex_file: Path, root: Path) -> Iterable[Path]:
    raw = raw.strip().strip('"').strip("'")
    p = Path(raw)
    bases = []
    if p.is_absolute():
        bases.append(p)
    else:
        bases.append(tex_file.parent / p)
        bases.append(root / p)

    if p.suffix:
        for b in bases:
            yield b
    else:
        for b in bases:
            for ext in ALL_IMAGE_EXTS:
                yield Path(str(b) + ext)


def resolve_image(raw: str, tex_file: Path, root: Path) -> Optional[Path]:
    for c in candidate_paths(raw, tex_file, root):
        if c.exists():
            return c.resolve()
    return None


def scan_includegraphics(root: Path, tex_files: List[Path]) -> Dict[str, ImageUse]:
    found: Dict[str, ImageUse] = {}
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = tex.read_text(errors="ignore")
        for m in INCLUDE_RE.finditer(text):
            raw = m.group("path").strip()
            resolved = resolve_image(raw, tex, root)
            key = raw if resolved is None else str(resolved)
            if key not in found:
                found[key] = ImageUse(raw_path=raw, resolved=resolved)
            if tex not in found[key].tex_files:
                found[key].tex_files.append(tex)
    return found


# --------------------------------------------------------------------------- #
# Dimension & scale selection
# --------------------------------------------------------------------------- #

def get_dimensions(path: Path) -> Optional[Tuple[int, int]]:
    magick = which("magick") or which("convert")
    if not magick:
        return None
    try:
        cmd = [magick, "identify", "-format", "%w %h", str(path)]
        cp = run_cmd(cmd)
        if cp.returncode == 0:
            parts = cp.stdout.strip().split()
            if len(parts) >= 2:
                return int(parts[0]), int(parts[1])
    except Exception:
        pass
    return None


# def choose_scale(src: Path) -> str:
#     name = src.stem.lower().replace("-", "_").replace(" ", "_")
#     dims = get_dimensions(src)
#     if dims:
#         w, h = dims
#         # Large images already: 4x is plenty
#         if w >= 2000 or h >= 2000:
#             return "4x"
#         # Medium-large: use 8x for text-heavy, 4x for diagrams
#         if w >= 1200 or h >= 800 or (w * h) >= 900_000:
#             if any(hint in name for hint in TEXT_HEAVY_8X_HINTS):
#                 return "8x"
#             return "4x"
#     # Small image or could not detect: use hints
#     if any(hint in name for hint in TEXT_HEAVY_8X_HINTS):
#         return "8x"
#     return "4x"

def choose_scale(src: Path) -> str:
    # Deadline-safe mode:
    # Always use 4x only to keep final PDF below upload limit.
    # No 8x outputs are generated.
    return "4x"
# --------------------------------------------------------------------------- #
# Backup
# --------------------------------------------------------------------------- #

def copy_backup(images_dir: Path) -> Optional[Path]:
    if not images_dir.exists():
        return None
    stamp = dt.datetime.now().strftime("%Y%m%d_%H%M%S")
    backup = images_dir.parent / f"images_backup_{stamp}"

    def ignore(dirpath: str, names: List[str]) -> set:
        return {n for n in names if n == "upscaled" or n.startswith("images_backup_")}

    shutil.copytree(images_dir, backup, ignore=ignore)
    return backup


# --------------------------------------------------------------------------- #
# Upscaling
# --------------------------------------------------------------------------- #

def upscale_image(
    src: Path,
    out_dir: Path,
    scale: str,
    model: str,
    allow_lanczos_fallback: bool,
) -> UpscaleResult:
    out_dir.mkdir(parents=True, exist_ok=True)
    dst = out_dir / f"{src.stem}_{scale}.png"
    temp4 = out_dir / f".{src.stem}_tmp_4x.png"

    realesrgan = which("realesrgan-ncnn-vulkan")
    magick = which("magick") or which("convert")

    try:
        # ------------------------------------------------------------------ #
        # Real-ESRGAN path
        # ------------------------------------------------------------------ #
        if realesrgan:
            if scale == "4x":
                cp = run_cmd([realesrgan, "-i", str(src), "-o", str(dst),
                               "-s", "4", "-n", model])
                if cp.returncode != 0:
                    return UpscaleResult(src, None, scale, "failed",
                                        f"Real-ESRGAN error:\n{cp.stdout[-800:]}")
                return UpscaleResult(src, dst, scale, "upscaled",
                                    f"Real-ESRGAN {model} 4x")

            # 8x: Real-ESRGAN 4x then Lanczos 2x
            cp = run_cmd([realesrgan, "-i", str(src), "-o", str(temp4),
                           "-s", "4", "-n", model])
            if cp.returncode != 0:
                return UpscaleResult(src, None, scale, "failed",
                                    f"Real-ESRGAN 4x step error:\n{cp.stdout[-800:]}")
            if not magick:
                return UpscaleResult(src, None, scale, "failed",
                                    "ImageMagick missing; cannot complete 8x from 4x output")
            cp2 = run_cmd([magick, str(temp4), "-filter", "Lanczos",
                            "-resize", "200%", "-unsharp", "0x1.0+0.6+0.02", str(dst)])
            if cp2.returncode != 0:
                return UpscaleResult(src, None, scale, "failed",
                                    f"ImageMagick 2x step error:\n{cp2.stdout[-800:]}")
            return UpscaleResult(src, dst, scale, "upscaled",
                                f"Real-ESRGAN {model} 4x + Lanczos 2x")

        # ------------------------------------------------------------------ #
        # Lanczos fallback (ImageMagick only)
        # ------------------------------------------------------------------ #
        if not allow_lanczos_fallback:
            return UpscaleResult(src, None, scale, "skipped",
                                "Real-ESRGAN not installed; --no-lanczos-fallback set")
        if not magick:
            return UpscaleResult(src, None, scale, "failed",
                                "Neither Real-ESRGAN nor ImageMagick found. "
                                "Install ImageMagick from https://imagemagick.org/script/download.php#windows")

        percent = "800%" if scale == "8x" else "400%"
        unsharp = "0x1.0+0.6+0.02" if scale == "8x" else "0x0.8+0.45+0.02"
        cp = run_cmd([magick, str(src), "-filter", "Lanczos",
                       "-resize", percent, "-unsharp", unsharp, str(dst)])
        if cp.returncode != 0:
            return UpscaleResult(src, None, scale, "failed",
                                f"ImageMagick error:\n{cp.stdout[-800:]}")
        return UpscaleResult(src, dst, scale, "upscaled",
                            f"ImageMagick Lanczos {scale} (no Real-ESRGAN)")

    finally:
        if temp4.exists():
            try:
                temp4.unlink()
            except Exception:
                pass


# --------------------------------------------------------------------------- #
# LaTeX path updates
# --------------------------------------------------------------------------- #

def update_tex_paths(
    tex_files: List[Path],
    replacements: Dict[Path, Path],
    root: Path,
) -> List[Path]:
    updated = []
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = tex.read_text(errors="ignore")

        changed = False

        def repl(m: re.Match) -> str:
            nonlocal changed
            raw = m.group("path").strip()
            resolved = resolve_image(raw, tex, root)
            if resolved and resolved in replacements:
                new_rel = rel_posix(replacements[resolved], tex.parent)
                changed = True
                return f"{m.group('cmd')}{new_rel}{m.group('end')}"
            return m.group(0)

        new_text = INCLUDE_RE.sub(repl, text)
        if changed:
            tex.write_text(new_text, encoding="utf-8")
            updated.append(tex)
    return updated


# --------------------------------------------------------------------------- #
# LaTeX compilation
# --------------------------------------------------------------------------- #

def find_main_tex(tex_files: List[Path], root: Path) -> Optional[Path]:
    candidates = []
    for tex in tex_files:
        try:
            text = tex.read_text(encoding="utf-8", errors="ignore")
        except TypeError:
            text = tex.read_text(errors="ignore")
        if r"\documentclass" in text:
            score = 0
            if tex.parent == root:
                score += 10
            if tex.name.lower() in {"main.tex", "poster.tex"}:
                score += 20
            candidates.append((score, tex))
    if not candidates:
        return None
    return sorted(candidates, reverse=True)[0][1]


def compile_pdf(main_tex: Optional[Path]) -> Tuple[str, str]:
    if main_tex is None:
        return "skipped", "No .tex with \\documentclass found."
    latexmk = which("latexmk")
    pdflatex = which("pdflatex")
    if latexmk:
        cmd = [latexmk, "-pdf", "-interaction=nonstopmode",
               "-halt-on-error", main_tex.name]
    elif pdflatex:
        cmd = [pdflatex, "-interaction=nonstopmode",
               "-halt-on-error", main_tex.name]
    else:
        return "skipped", "Neither latexmk nor pdflatex found."
    cp = run_cmd(cmd, cwd=main_tex.parent)
    if cp.returncode == 0:
        pdf = main_tex.with_suffix(".pdf")
        msg = f"PDF written: {pdf}" if pdf.exists() else "Compile command succeeded."
        return "success", msg
    return "failed", cp.stdout[-3000:]


# --------------------------------------------------------------------------- #
# Report
# --------------------------------------------------------------------------- #

def write_report(
    root: Path,
    image_uses: Dict[str, ImageUse],
    results: List[UpscaleResult],
    skipped: List[Tuple[str, str]],
    updated_tex: List[Path],
    backup: Optional[Path],
    compile_status: Tuple[str, str],
    report_path: Path,
    dry_run: bool,
) -> None:
    lines = [
        "# Poster image upscaling report",
        "",
        f"Project root: `{root}`",
        f"Mode: {'DRY RUN (no changes made)' if dry_run else 'LIVE'}",
        f"Backup: `{backup}`" if backup else "Backup: not created",
        "",
        "---",
        "",
        "## Images found in LaTeX",
    ]

    present = [(k, u) for k, u in image_uses.items() if u.resolved]
    missing = [(k, u) for k, u in image_uses.items() if not u.resolved]

    if present:
        lines.append("")
        lines.append("### Present")
        for _, use in sorted(present, key=lambda x: x[0]):
            rp = rel_posix(use.resolved, root)
            dims = get_dimensions(use.resolved)
            dim_str = f" [{dims[0]}x{dims[1]}px]" if dims else ""
            size_str = f" {fmt_size(use.resolved.stat().st_size)}"
            texs = ", ".join(rel_posix(t, root) for t in sorted(set(use.tex_files)))
            lines.append(f"- `{use.raw_path}` -> `{rp}`{dim_str}{size_str}  |  {texs}")

    if missing:
        lines.append("")
        lines.append("### Missing (referenced in .tex but file not found)")
        for _, use in sorted(missing, key=lambda x: x[0]):
            texs = ", ".join(rel_posix(t, root) for t in sorted(set(use.tex_files)))
            lines.append(f"- `{use.raw_path}`  |  {texs}")

    lines += ["", "---", "", "## Upscaled"]
    done = [r for r in results if r.status == "upscaled" and r.dst]
    if done:
        for r in done:
            src_r = rel_posix(r.src, root)
            dst_r = rel_posix(r.dst, root)
            lines.append(f"- `{src_r}` -> `{dst_r}` ({r.scale}) — {r.note}")
    else:
        lines.append("- None")

    lines += ["", "---", "", "## Skipped / Failed"]
    skip_lines = []
    for raw, reason in skipped:
        skip_lines.append(f"- `{raw}` — {reason}")
    for r in results:
        if r.status != "upscaled":
            skip_lines.append(f"- `{rel_posix(r.src, root)}` — {r.status}: {r.note}")
    if skip_lines:
        lines += skip_lines
    else:
        lines.append("- None")

    lines += ["", "---", "", "## LaTeX files updated"]
    if updated_tex:
        for t in updated_tex:
            lines.append(f"- `{rel_posix(t, root)}`")
    else:
        lines.append("- None")

    lines += [
        "",
        "---",
        "",
        "## PDF compile",
        f"- **{compile_status[0]}**: {compile_status[1].splitlines()[0]}",
        "",
        "---",
        "",
        "## How to revert a single image",
        "If one upscaled image looks worse, open the relevant `.tex` file and",
        "change its `\\includegraphics` path back to the original.",
        "The originals are still in `poster/images/` (unchanged).",
        "The backup folder (see top of this report) also has a full copy.",
    ]

    report_path.write_text("\n".join(lines), encoding="utf-8")


# --------------------------------------------------------------------------- #
# Main
# --------------------------------------------------------------------------- #

def main() -> int:
    ap = argparse.ArgumentParser(description="Upscale poster images for LaTeX projects")
    ap.add_argument("root", nargs="?", default=".",
                    help="Project root (default: current directory)")
    ap.add_argument("--model", default="realesrgan-x4plus",
                    help="Real-ESRGAN model (default: realesrgan-x4plus)")
    ap.add_argument("--no-lanczos-fallback", action="store_true",
                    help="Skip images if Real-ESRGAN is not installed")
    ap.add_argument("--dry-run", action="store_true",
                    help="Scan and report only; make no changes")
    ap.add_argument("--all-figures", action="store_true",
                    help="Also upscale raster images outside poster/images/ (e.g. figures/)")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    images_dir = root / "poster" / "images"
    upscaled_dir = images_dir / "upscaled"
    report_path = root / "poster_image_upscale_report.md"

    print(f"Root : {root}")
    print(f"Images dir : {images_dir}  (exists: {images_dir.exists()})")
    print()

    # ------------------------------------------------------------------ #
    # Check tools
    # ------------------------------------------------------------------ #
    magick = which("magick") or which("convert")
    realesrgan = which("realesrgan-ncnn-vulkan")
    print("Tools detected:")
    print(f"  ImageMagick : {'YES -> ' + (which('magick') or which('convert')) if magick else 'NOT FOUND - install from https://imagemagick.org/script/download.php#windows'}")
    print(f"  Real-ESRGAN : {'YES -> ' + realesrgan if realesrgan else 'not installed (Lanczos fallback will be used)'}")
    print()

    if not magick and not realesrgan and not args.dry_run:
        print("ERROR: No upscaling tool found.")
        print("Install ImageMagick: https://imagemagick.org/script/download.php#windows")
        print("Then re-open your terminal and try again.")
        return 1

    # ------------------------------------------------------------------ #
    # Scan .tex files
    # ------------------------------------------------------------------ #
    tex_files = find_tex_files(root)
    print(f"Found {len(tex_files)} .tex files")
    image_uses = scan_includegraphics(root, tex_files)
    print(f"Found {len(image_uses)} unique \\includegraphics references")
    print()

    # ------------------------------------------------------------------ #
    # Classify each image
    # ------------------------------------------------------------------ #
    skipped: List[Tuple[str, str]] = []
    to_process: Dict[Path, str] = {}

    for use in image_uses.values():
        raw = use.raw_path
        resolved = use.resolved

        if resolved is None:
            skipped.append((raw, "file not found"))
            continue

        ext = resolved.suffix.lower()

        if ext in VECTOR_EXTS:
            skipped.append((raw, "vector file — skipped"))
            continue

        if ext not in RASTER_EXTS:
            skipped.append((raw, f"unsupported type '{ext}' — skipped"))
            continue

        if is_under(resolved, upscaled_dir):
            skipped.append((raw, "already in upscaled/ — skipped"))
            continue

        # Gate: must be under poster/images/ unless --all-figures
        if not is_under(resolved, images_dir) and not args.all_figures:
            skipped.append((raw, "outside poster/images/ — use --all-figures to include"))
            continue

        to_process[resolved] = choose_scale(resolved)

    print(f"Images to upscale : {len(to_process)}")
    for p, s in sorted(to_process.items()):
        dims = get_dimensions(p)
        dim_str = f" [{dims[0]}x{dims[1]}]" if dims else ""
        print(f"  {rel_posix(p, root)}{dim_str}  ->  {s}")
    print(f"Skipped           : {len(skipped)}")
    print()

    # ------------------------------------------------------------------ #
    # Execute
    # ------------------------------------------------------------------ #
    backup: Optional[Path] = None
    results: List[UpscaleResult] = []
    updated_tex: List[Path] = []
    compile_status = ("skipped", "dry run")

    if not args.dry_run:
        if to_process:
            print("Creating backup of poster/images/ ...")
            backup = copy_backup(images_dir)
            if backup:
                print(f"  Backup: {backup}")
            upscaled_dir.mkdir(parents=True, exist_ok=True)

            for src, scale in sorted(to_process.items()):
                print(f"  Upscaling {rel_posix(src, root)} ({scale}) ...", end=" ", flush=True)
                result = upscale_image(src, upscaled_dir, scale, args.model,
                                       not args.no_lanczos_fallback)
                results.append(result)
                if result.status == "upscaled":
                    size = result.dst.stat().st_size if result.dst else 0
                    print(f"OK ({fmt_size(size)}) — {result.note}")
                else:
                    print(f"FAILED — {result.note}")

            replacements = {r.src: r.dst
                            for r in results
                            if r.status == "upscaled" and r.dst}
            if replacements:
                print()
                print("Updating \\includegraphics paths in .tex files ...")
                updated_tex = update_tex_paths(tex_files, replacements, root)
                for t in updated_tex:
                    print(f"  Updated: {rel_posix(t, root)}")

        print()
        print("Attempting PDF compile ...")
        main_tex = find_main_tex(tex_files, root)
        compile_status = compile_pdf(main_tex)
        print(f"  {compile_status[0]}: {compile_status[1].splitlines()[0]}")
    else:
        print("Dry run — no files changed.")

    # ------------------------------------------------------------------ #
    # Report
    # ------------------------------------------------------------------ #
    write_report(
        root, image_uses, results, skipped, updated_tex,
        backup, compile_status, report_path, args.dry_run,
    )
    print()
    print(f"Report written: {report_path}")

    failed = [r for r in results if r.status == "failed"]
    if failed:
        print(f"WARNING: {len(failed)} image(s) failed to upscale — see report.")

    return 0 if compile_status[0] != "failed" else 2


if __name__ == "__main__":
    raise SystemExit(main())